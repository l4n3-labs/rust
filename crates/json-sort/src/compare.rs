use std::cmp::Ordering;

use serde_json::Value;

use crate::options::{SortBy, SortDirection, SortOptions};

/// Compare two object property `(key, value)` pairs according to the sort options.
///
/// Uses the key name as a tiebreaker when the primary comparison is equal.
/// Returns [`Ordering::Equal`] for random direction (shuffling is handled separately).
pub fn compare_properties(a: (&String, &Value), b: (&String, &Value), options: &SortOptions) -> Ordering {
    if options.direction == SortDirection::Random {
        return Ordering::Equal;
    }

    let ordering = match options.sort_by {
        SortBy::Key => compare_strings(a.0, b.0, options.case_sensitive),
        SortBy::Value => {
            let cmp = compare_json_values(a.1, b.1, options.case_sensitive);
            if cmp == Ordering::Equal { compare_strings(a.0, b.0, options.case_sensitive) } else { cmp }
        }
        SortBy::KeyLength => {
            let cmp = a.0.len().cmp(&b.0.len());
            if cmp == Ordering::Equal { compare_strings(a.0, b.0, options.case_sensitive) } else { cmp }
        }
        SortBy::ValueLength => {
            let cmp = value_length(a.1).cmp(&value_length(b.1));
            if cmp == Ordering::Equal { compare_strings(a.0, b.0, options.case_sensitive) } else { cmp }
        }
        SortBy::ValueType => {
            let cmp = type_rank(a.1).cmp(&type_rank(b.1));
            if cmp == Ordering::Equal { compare_strings(a.0, b.0, options.case_sensitive) } else { cmp }
        }
    };

    match options.direction {
        SortDirection::Ascending => ordering,
        SortDirection::Descending => ordering.reverse(),
        SortDirection::Random => Ordering::Equal,
    }
}

/// Compare two bare JSON values for array element sorting.
///
/// Applies the configured direction (ascending reverses nothing, descending reverses).
pub fn compare_values(a: &Value, b: &Value, options: &SortOptions) -> Ordering {
    if options.direction == SortDirection::Random {
        return Ordering::Equal;
    }

    let ordering = compare_json_values(a, b, options.case_sensitive);

    match options.direction {
        SortDirection::Ascending => ordering,
        SortDirection::Descending => ordering.reverse(),
        SortDirection::Random => Ordering::Equal,
    }
}

/// Compare two JSON values by type rank first, then by natural ordering within the same type.
///
/// Different types are ordered: bool < null < number < string < array < object.
/// Same-type values are compared by their natural representation (numbers as f64,
/// strings lexicographically, arrays/objects by element count).
pub fn compare_json_values(a: &Value, b: &Value, case_sensitive: bool) -> Ordering {
    let rank_cmp = type_rank(a).cmp(&type_rank(b));
    if rank_cmp != Ordering::Equal {
        return rank_cmp;
    }

    match (a, b) {
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(0.0);
            let b_f = b.as_f64().unwrap_or(0.0);
            a_f.partial_cmp(&b_f).unwrap_or(Ordering::Equal)
        }
        (Value::String(a), Value::String(b)) => compare_strings(a, b, case_sensitive),
        (Value::Array(a), Value::Array(b)) => a.len().cmp(&b.len()),
        (Value::Object(a), Value::Object(b)) => a.len().cmp(&b.len()),
        _ => Ordering::Equal,
    }
}

/// Return a numeric rank for the JSON value type, used for cross-type ordering.
///
/// Ordering: Bool(0) < Null(1) < Number(2) < String(3) < Array(4) < Object(5).
pub const fn type_rank(value: &Value) -> u8 {
    match value {
        Value::Bool(_) => 0,
        Value::Null => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

// Case-insensitive mode lowercases both sides, with original case as tiebreaker.
fn compare_strings(a: &str, b: &str, case_sensitive: bool) -> Ordering {
    if case_sensitive { a.cmp(b) } else { a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)) }
}

// Strings → char count, arrays/objects → element count, everything else → 0.
fn value_length(value: &Value) -> usize {
    match value {
        Value::String(s) => s.len(),
        Value::Array(a) => a.len(),
        Value::Object(o) => o.len(),
        _ => 0,
    }
}
