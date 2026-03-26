//! JSON and JSONC sorting library with multiple sort strategies and comment preservation.
//!
//! This crate provides functions to sort JSON and JSONC (JSON with Comments) content
//! by keys, values, key length, value length, or value type — in ascending, descending,
//! or random order. JSONC features like line/block comments and trailing commas are
//! automatically detected and preserved during sorting.
//!
//! # Quick start
//!
//! ```
//! use json_sort::{sort_json, SortOptions};
//!
//! let input = r#"{"b": 2, "a": 1, "c": 3}"#;
//! let sorted = sort_json(input, &SortOptions::default()).unwrap();
//! assert!(sorted.contains(r#""a": 1"#));
//! ```

mod compare;
mod error;
mod jsonc;
mod options;

use std::ops::Range;

use rand::seq::SliceRandom;
use serde_json::Value;
use serde_json::ser::{PrettyFormatter, Serializer};

use compare::{compare_properties, compare_values};

pub use error::SortError;
pub use options::{Indent, SortBy, SortDirection, SortOptions, SortTarget};

/// Sort an entire JSON or JSONC string according to the given options.
///
/// Attempts fast-path parsing with `serde_json` first. If that fails (e.g. the input
/// contains comments or trailing commas), falls back to the JSONC parser which
/// preserves comments in the output.
///
/// # Errors
///
/// Returns [`SortError::Parse`] if the input is not valid JSON or JSONC, or
/// [`SortError::Serialize`] if the sorted result cannot be serialized.
pub fn sort_json(input: &str, options: &SortOptions) -> Result<String, SortError> {
    // Fast path: try plain JSON first
    if let Ok(mut value) = serde_json::from_str::<Value>(input) {
        sort_value(&mut value, options, 0);
        return serialize(&value, options);
    }
    // Slow path: JSONC (comments, trailing commas, etc.)
    jsonc::sort_jsonc(input, options)
}

/// Sort a byte-range slice of the JSON string, leaving the surrounding text untouched.
///
/// The `range` parameter specifies byte offsets into `input`. The slice at those offsets
/// is parsed and sorted independently, then spliced back into the original string.
///
/// # Errors
///
/// Returns [`SortError::InvalidRange`] if the range is out of bounds, or any error
/// that [`sort_json`] can return for the extracted slice.
pub fn sort_json_range(input: &str, range: Range<usize>, options: &SortOptions) -> Result<String, SortError> {
    let len = input.len();
    if range.start > len || range.end > len || range.start > range.end {
        return Err(SortError::InvalidRange { start: range.start, end: range.end, len });
    }

    let slice = &input[range.clone()];
    let sorted_slice = sort_json(slice, options)?;

    let mut result = String::with_capacity(input.len());
    result.push_str(&input[..range.start]);
    result.push_str(&sorted_slice);
    result.push_str(&input[range.end..]);
    Ok(result)
}

// Recursively sort a JSON value in-place, respecting `sort_level` depth control.
fn sort_value(value: &mut Value, options: &SortOptions, depth: i32) {
    let should_recurse = options.sort_level < 0 || depth < options.sort_level;

    match value {
        Value::Object(map) => {
            if should_sort_object(options) {
                sort_object(map, options);
            }
            if should_recurse {
                for val in map.values_mut() {
                    sort_value(val, options, depth + 1);
                }
            }
        }
        Value::Array(arr) => {
            if should_sort_array(options) {
                sort_array(arr, options);
            }
            if let SortTarget::CollectionByKey(ref key) = options.target {
                sort_collection_by_key(arr, key, options);
            }
            if should_recurse {
                for item in arr.iter_mut() {
                    sort_value(item, options, depth + 1);
                }
            }
        }
        _ => {}
    }
}

const fn should_sort_object(options: &SortOptions) -> bool {
    matches!(options.target, SortTarget::ObjectKeys | SortTarget::Both)
}

const fn should_sort_array(options: &SortOptions) -> bool {
    matches!(options.target, SortTarget::ListItems | SortTarget::Both)
}

// Drain the map, sort entries, and re-insert in sorted order.
// Works because `serde_json::Map` with `preserve_order` maintains insertion order.
fn sort_object(map: &mut serde_json::Map<String, Value>, options: &SortOptions) {
    let mut entries: Vec<(String, Value)> = map.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    map.clear();

    if options.direction == SortDirection::Random {
        let mut rng = rand::rng();
        entries.shuffle(&mut rng);
    } else {
        entries.sort_by(|a, b| compare_properties((&a.0, &a.1), (&b.0, &b.1), options));
    }

    for (k, v) in entries {
        map.insert(k, v);
    }
}

fn sort_array(arr: &mut [Value], options: &SortOptions) {
    if options.direction == SortDirection::Random {
        let mut rng = rand::rng();
        arr.shuffle(&mut rng);
    } else {
        arr.sort_by(|a, b| compare_values(a, b, options));
    }
}

// Sort an array of objects by a shared key field. Items missing the key sort to the front.
fn sort_collection_by_key(arr: &mut [Value], key: &str, options: &SortOptions) {
    if options.direction == SortDirection::Random {
        let mut rng = rand::rng();
        arr.shuffle(&mut rng);
    } else {
        arr.sort_by(|a, b| {
            let a_val = a.get(key);
            let b_val = b.get(key);
            match (a_val, b_val) {
                (Some(av), Some(bv)) => {
                    let ordering = compare::compare_json_values(av, bv, options.case_sensitive);
                    match options.direction {
                        SortDirection::Ascending => ordering,
                        SortDirection::Descending => ordering.reverse(),
                        SortDirection::Random => std::cmp::Ordering::Equal,
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
    }
}

// Serialize a JSON value to a pretty-printed string with configurable indentation.
fn serialize(value: &Value, options: &SortOptions) -> Result<String, SortError> {
    let indent_str = match options.indent {
        Indent::Spaces(n) => " ".repeat(n as usize),
        Indent::Tabs => "\t".to_string(),
    };

    let mut buf = Vec::new();
    let formatter = PrettyFormatter::with_indent(indent_str.as_bytes());
    let mut serializer = Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(value, &mut serializer).map_err(|e| SortError::Serialize(e.to_string()))?;

    String::from_utf8(buf).map_err(|e| SortError::Serialize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn sort_simple_object_ascending() {
        let input = r#"{"c": 3, "a": 1, "b": 2}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        let expected = "{\n  \"a\": 1,\n  \"b\": 2,\n  \"c\": 3\n}";
        assert_eq!(result, expected);
    }

    #[test]
    fn sort_nested_objects_ascending() {
        let input = r#"{"b": {"z": 1, "a": 2}, "a": 1}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        let nested_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(nested_keys, vec!["a", "z"]);
    }

    #[test]
    fn sort_empty_object() {
        let input = "{}";
        let result = sort_json(input, &SortOptions::default()).unwrap();
        assert_eq!(result, "{}");
    }

    #[test]
    fn sort_single_key() {
        let input = r#"{"a": 1}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        assert_eq!(result, "{\n  \"a\": 1\n}");
    }

    // Task 4: Sort Modes Tests

    #[test]
    fn sort_descending() {
        let input = r#"{"a": 1, "c": 3, "b": 2}"#;
        let opts = SortOptions { direction: SortDirection::Descending, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["c", "b", "a"]);
    }

    #[test]
    fn sort_by_value() {
        let input = r#"{"b": 3, "a": 1, "c": 2}"#;
        let opts = SortOptions { sort_by: SortBy::Value, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "c", "b"]);
    }

    #[test]
    fn sort_by_key_length() {
        let input = r#"{"bbb": 1, "a": 2, "cc": 3}"#;
        let opts = SortOptions { sort_by: SortBy::KeyLength, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "cc", "bbb"]);
    }

    #[test]
    fn sort_by_value_length() {
        let input = r#"{"a": "zzz", "b": "x", "c": "yy"}"#;
        let opts = SortOptions { sort_by: SortBy::ValueLength, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["b", "c", "a"]);
    }

    #[test]
    fn sort_by_value_type() {
        let input = r#"{"s": "hi", "n": 1, "b": true, "null": null, "a": [1], "o": {"x": 1}}"#;
        let opts = SortOptions { sort_by: SortBy::ValueType, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["b", "null", "n", "s", "a", "o"]);
    }

    #[test]
    fn sort_randomize_changes_order() {
        let input = r#"{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7,"h":8}"#;
        let opts = SortOptions { direction: SortDirection::Random, ..Default::default() };
        let results: Vec<String> = (0..5).map(|_| sort_json(input, &opts).unwrap()).collect();
        let any_different = results.iter().any(|r| {
            let parsed: serde_json::Value = serde_json::from_str(r).unwrap();
            let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
            keys != vec!["a", "b", "c", "d", "e", "f", "g", "h"]
        });
        assert!(any_different, "randomize should produce different orderings");
    }

    // Task 5: Array Sorting Tests

    #[test]
    fn sort_array_ascending() {
        let input = r"[3, 1, 2]";
        let opts = SortOptions { target: SortTarget::ListItems, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn sort_array_strings() {
        let input = r#"["cherry", "apple", "banana"]"#;
        let opts = SortOptions { target: SortTarget::ListItems, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, serde_json::json!(["apple", "banana", "cherry"]));
    }

    #[test]
    fn sort_array_descending() {
        let input = r"[1, 3, 2]";
        let opts =
            SortOptions { target: SortTarget::ListItems, direction: SortDirection::Descending, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, serde_json::json!([3, 2, 1]));
    }

    #[test]
    fn sort_both_objects_and_arrays() {
        let input = r#"{"b": [3, 1, 2], "a": 1}"#;
        let opts = SortOptions { target: SortTarget::Both, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(parsed["b"], serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn sort_mixed_type_array() {
        let input = r#"["b", 1, true, null, "a", 2]"#;
        let opts = SortOptions { target: SortTarget::ListItems, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, serde_json::json!([true, null, 1, 2, "a", "b"]));
    }

    // Task 6: Collection Sorting Tests

    #[test]
    fn sort_collection_by_key_string() {
        let input = r#"[{"name": "charlie"}, {"name": "alice"}, {"name": "bob"}]"#;
        let opts = SortOptions { target: SortTarget::CollectionByKey("name".to_string()), ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let names: Vec<&str> = parsed.as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["alice", "bob", "charlie"]);
    }

    #[test]
    fn sort_collection_by_key_numeric() {
        let input = r#"[{"id": 3}, {"id": 1}, {"id": 2}]"#;
        let opts = SortOptions { target: SortTarget::CollectionByKey("id".to_string()), ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let ids: Vec<i64> = parsed.as_array().unwrap().iter().map(|v| v["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn sort_collection_missing_key_sorted_to_front() {
        let input = r#"[{"name": "bob"}, {"age": 30}, {"name": "alice"}]"#;
        let opts = SortOptions { target: SortTarget::CollectionByKey("name".to_string()), ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr[0]["age"], serde_json::json!(30));
        assert_eq!(arr[1]["name"], serde_json::json!("alice"));
        assert_eq!(arr[2]["name"], serde_json::json!("bob"));
    }

    // Task 7: Depth Control and Case Sensitivity Tests

    #[test]
    fn sort_level_zero_only_top_level() {
        let input = r#"{"b": {"z": 1, "a": 2}, "a": 1}"#;
        let opts = SortOptions { sort_level: 0, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        let nested_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(nested_keys, vec!["z", "a"]);
    }

    #[test]
    fn sort_level_one_sorts_two_levels() {
        let input = r#"{"b": {"z": {"y": 1, "x": 2}, "a": 1}, "a": 1}"#;
        let opts = SortOptions { sort_level: 1, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        let nested_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(nested_keys, vec!["a", "z"]);
        let deep_keys: Vec<&String> = parsed["b"]["z"].as_object().unwrap().keys().collect();
        assert_eq!(deep_keys, vec!["y", "x"]);
    }

    #[test]
    fn sort_case_insensitive() {
        let input = r#"{"Banana": 1, "apple": 2, "Cherry": 3}"#;
        let opts = SortOptions { case_sensitive: false, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["apple", "Banana", "Cherry"]);
    }

    #[test]
    fn sort_case_sensitive() {
        let input = r#"{"banana": 1, "Apple": 2, "cherry": 3}"#;
        let opts = SortOptions { case_sensitive: true, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["Apple", "banana", "cherry"]);
    }

    // Task 8: Range Sorting Tests

    #[test]
    fn sort_json_range_object_in_larger_document() {
        let prefix = "some prefix text ";
        let json_part = r#"{"c": 3, "a": 1, "b": 2}"#;
        let suffix = " some suffix text";
        let input = format!("{prefix}{json_part}{suffix}");
        let start = prefix.len();
        let end = start + json_part.len();
        let result = sort_json_range(&input, start..end, &SortOptions::default()).unwrap();
        assert!(result.starts_with(prefix));
        assert!(result.ends_with(suffix));
        let sorted_part = &result[start..result.len() - suffix.len()];
        let parsed: serde_json::Value = serde_json::from_str(sorted_part).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn sort_json_range_invalid_range() {
        let input = r#"{"a": 1}"#;
        let result = sort_json_range(input, 0..100, &SortOptions::default());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SortError::InvalidRange { .. }));
    }

    #[test]
    fn sort_json_range_empty_range() {
        let input = r"some text {} more text";
        let result = sort_json_range(input, 10..12, &SortOptions::default()).unwrap();
        assert_eq!(result, input);
    }

    // JSONC Tests

    #[test]
    fn sort_jsonc_line_comments_preserved() {
        let input = "{\n  // B's comment\n  \"b\": 2,\n  // A's comment\n  \"a\": 1\n}";
        let result = sort_json(input, &SortOptions::default()).unwrap();
        assert!(result.contains("// A's comment"));
        assert!(result.contains("// B's comment"));
        let a_pos = result.find("\"a\"").unwrap();
        let b_pos = result.find("\"b\"").unwrap();
        assert!(a_pos < b_pos, "a should come before b after sorting");
        let a_comment_pos = result.find("// A's comment").unwrap();
        assert!(a_comment_pos < a_pos, "A's comment should appear before A");
    }

    #[test]
    fn sort_jsonc_block_comments_preserved() {
        let input = "{\n  /* B */ \"b\": 2,\n  /* A */ \"a\": 1\n}";
        let result = sort_json(input, &SortOptions::default()).unwrap();
        assert!(result.contains("/* A */"));
        assert!(result.contains("/* B */"));
        let a_pos = result.find("\"a\"").unwrap();
        let b_pos = result.find("\"b\"").unwrap();
        assert!(a_pos < b_pos);
    }

    #[test]
    fn sort_jsonc_trailing_commas() {
        let input = r#"{"c": 3, "a": 1, "b": 2,}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn sort_plain_json_no_comments() {
        let input = r#"{"b": 2, "a": 1}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    // Task 10: Edge Case Tests

    #[test]
    fn sort_deeply_nested() {
        let input = r#"{"c": {"f": {"i": 1, "h": 2}, "e": 1}, "b": 1, "a": {"d": 1}}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
        let c_keys: Vec<&String> = parsed["c"].as_object().unwrap().keys().collect();
        assert_eq!(c_keys, vec!["e", "f"]);
        let f_keys: Vec<&String> = parsed["c"]["f"].as_object().unwrap().keys().collect();
        assert_eq!(f_keys, vec!["h", "i"]);
    }

    #[test]
    fn sort_unicode_keys() {
        let input = r#"{"ñ": 1, "á": 2, "z": 3}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["z", "á", "ñ"]);
    }

    #[test]
    fn sort_empty_array() {
        let input = "[]";
        let opts = SortOptions { target: SortTarget::ListItems, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        assert_eq!(result, "[]");
    }

    #[test]
    fn sort_preserves_number_precision() {
        let input = r#"{"b": 1.23456789012345, "a": 1}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        assert!(result.contains("1.23456789012345"));
    }

    #[test]
    fn sort_with_tab_indent() {
        let input = r#"{"b": 2, "a": 1}"#;
        let opts = SortOptions { indent: Indent::Tabs, ..Default::default() };
        let result = sort_json(input, &opts).unwrap();
        assert!(result.contains("\t\"a\""));
    }

    #[test]
    fn sort_invalid_json_returns_error() {
        let result = sort_json("not json at all", &SortOptions::default());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SortError::Parse(_)));
    }

    #[test]
    fn sort_null_value() {
        let result = sort_json("null", &SortOptions::default()).unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn sort_preserves_string_escapes() {
        let input = r#"{"b": "hello\nworld", "a": "tab\there"}"#;
        let result = sort_json(input, &SortOptions::default()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["a"].as_str().unwrap(), "tab\there");
        assert_eq!(parsed["b"].as_str().unwrap(), "hello\nworld");
    }
}
