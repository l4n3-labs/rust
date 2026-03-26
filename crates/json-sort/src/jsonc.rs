use std::collections::HashMap;
use std::rc::Rc;

use jsonc_parser::ast::{Comment, CommentKind, ObjectProp, Value};
use jsonc_parser::common::Ranged;
use jsonc_parser::{CollectOptions, CommentCollectionStrategy, ParseOptions};

use crate::SortError;
use crate::compare::compare_properties;
use crate::options::{Indent, SortOptions};

/// Sort JSONC content while preserving comments.
///
/// Parses the input with `jsonc_parser` to build an AST and a separate comment map.
/// Object properties are sorted using the same comparison functions as the plain JSON
/// path, but comments that precede each property are kept attached and move with it.
pub fn sort_jsonc(input: &str, options: &SortOptions) -> Result<String, SortError> {
    let collect_options = CollectOptions { comments: CommentCollectionStrategy::Separate, tokens: false };
    let parse_options = ParseOptions::default();

    let parse_result = jsonc_parser::parse_to_ast(input, &collect_options, &parse_options)
        .map_err(|e| SortError::Parse(e.to_string()))?;

    let comments = parse_result.comments.unwrap_or_default();

    parse_result.value.map_or_else(
        || Ok(String::new()),
        |value| {
            let indent_str = match options.indent {
                Indent::Spaces(n) => " ".repeat(n as usize),
                Indent::Tabs => "\t".to_string(),
            };
            let mut output = String::new();
            emit_value(input, &value, &comments, options, &indent_str, 0, &mut output);
            Ok(output)
        },
    )
}

// Gather all comments whose start position falls within `range_start..range_end`,
// deduplicated by start position and sorted by source order.
fn collect_comments_in_range<'a>(
    comments: &HashMap<usize, Rc<Vec<Comment<'a>>>>,
    range_start: usize,
    range_end: usize,
) -> Vec<Comment<'a>> {
    let mut result: Vec<Comment<'a>> = Vec::new();
    let mut seen_starts = std::collections::HashSet::new();

    for comment_list in comments.values() {
        for comment in comment_list.as_ref() {
            let cr = comment.range();
            if cr.start >= range_start && cr.start < range_end && seen_starts.insert(cr.start) {
                result.push(comment.clone());
            }
        }
    }

    result.sort_by_key(|c| c.range().start);
    result
}

// Write a single comment to the output buffer with proper indentation.
// Line comments get their own line; block comments are emitted inline.
fn emit_comment(comment: &Comment<'_>, indent: &str, depth: usize, output: &mut String) {
    let prefix = indent.repeat(depth);
    match comment.kind() {
        CommentKind::Line => {
            output.push_str(&prefix);
            output.push_str("//");
            output.push_str(comment.text());
            output.push('\n');
        }
        CommentKind::Block => {
            output.push_str(&prefix);
            output.push_str("/*");
            output.push_str(comment.text());
            output.push_str("*/ ");
        }
    }
}

// Recursively emit a JSON value. Objects and arrays are dispatched to their
// specialized emitters; primitives are copied verbatim from the source text.
fn emit_value(
    source: &str,
    value: &Value<'_>,
    comments: &HashMap<usize, Rc<Vec<Comment<'_>>>>,
    options: &SortOptions,
    indent_str: &str,
    depth: usize,
    output: &mut String,
) {
    match value {
        Value::Object(obj) => {
            emit_object(source, obj, comments, options, indent_str, depth, output);
        }
        Value::Array(arr) => {
            emit_array(source, arr, comments, options, indent_str, depth, output);
        }
        _ => {
            // For primitives, extract source text
            let r = value.range();
            output.push_str(&source[r.start..r.end]);
        }
    }
}

// Emit a sorted object. For each property, collect its leading comments, sort the
// (property, comments) pairs, then write them out with correct indentation.
fn emit_object(
    source: &str,
    obj: &jsonc_parser::ast::Object<'_>,
    comments: &HashMap<usize, Rc<Vec<Comment<'_>>>>,
    options: &SortOptions,
    indent_str: &str,
    depth: usize,
    output: &mut String,
) {
    if obj.properties.is_empty() {
        output.push_str("{}");
        return;
    }

    // Build sortable entries: (property, leading_comments)
    let container_start = obj.range().start + 1; // after '{'

    let mut entries: Vec<(&ObjectProp<'_>, Vec<Comment<'_>>)> = Vec::new();

    for (i, prop) in obj.properties.iter().enumerate() {
        let region_start = if i == 0 { container_start } else { obj.properties[i - 1].range().end };
        let region_end = prop.range().start;
        let leading = collect_comments_in_range(comments, region_start, region_end);
        entries.push((prop, leading));
    }

    // Sort entries by property name using the same comparison as the main sort
    entries.sort_by(|a, b| {
        let a_name = a.0.name.as_str().to_string();
        let b_name = b.0.name.as_str().to_string();
        let a_serde = ast_value_to_serde(&a.0.value);
        let b_serde = ast_value_to_serde(&b.0.value);
        compare_properties((&a_name, &a_serde), (&b_name, &b_serde), options)
    });

    output.push_str("{\n");
    let child_depth = depth + 1;

    for (i, (prop, leading_comments)) in entries.iter().enumerate() {
        // Emit leading comments
        for comment in leading_comments {
            emit_comment(comment, indent_str, child_depth, output);
        }

        let prefix = indent_str.repeat(child_depth);

        // Check if the block comment was just emitted (inline style)
        let ends_with_block =
            !leading_comments.is_empty() && leading_comments.last().is_some_and(|c| c.kind() == CommentKind::Block);

        if !ends_with_block {
            output.push_str(&prefix);
        }

        // Emit property name
        output.push('"');
        output.push_str(prop.name.as_str());
        output.push_str("\": ");

        // Emit property value (recursively for nested objects/arrays)
        emit_value(source, &prop.value, comments, options, indent_str, child_depth, output);

        if i < entries.len() - 1 {
            output.push(',');
        }
        output.push('\n');
    }

    let closing_prefix = indent_str.repeat(depth);
    output.push_str(&closing_prefix);
    output.push('}');
}

// Emit an array. Elements are written in their original order (array sorting
// is handled at the serde_json level in the fast path).
fn emit_array(
    source: &str,
    arr: &jsonc_parser::ast::Array<'_>,
    comments: &HashMap<usize, Rc<Vec<Comment<'_>>>>,
    options: &SortOptions,
    indent_str: &str,
    depth: usize,
    output: &mut String,
) {
    if arr.elements.is_empty() {
        output.push_str("[]");
        return;
    }

    output.push_str("[\n");
    let child_depth = depth + 1;

    for (i, elem) in arr.elements.iter().enumerate() {
        let prefix = indent_str.repeat(child_depth);
        output.push_str(&prefix);

        emit_value(source, elem, comments, options, indent_str, child_depth, output);

        if i < arr.elements.len() - 1 {
            output.push(',');
        }
        output.push('\n');
    }

    let closing_prefix = indent_str.repeat(depth);
    output.push_str(&closing_prefix);
    output.push(']');
}

// Convert a `jsonc_parser` AST value to a `serde_json::Value` for comparison.
fn ast_value_to_serde(value: &Value<'_>) -> serde_json::Value {
    match value {
        Value::StringLit(s) => serde_json::Value::String(s.value.to_string()),
        Value::NumberLit(n) => n
            .value
            .parse::<serde_json::Number>()
            .map_or_else(|_| serde_json::Value::String(n.value.to_string()), serde_json::Value::Number),
        Value::BooleanLit(b) => serde_json::Value::Bool(b.value),
        Value::NullKeyword(_) => serde_json::Value::Null,
        Value::Object(obj) => {
            let mut map = serde_json::Map::new();
            for prop in &obj.properties {
                let key = prop.name.as_str().to_string();
                let val = ast_value_to_serde(&prop.value);
                map.insert(key, val);
            }
            serde_json::Value::Object(map)
        }
        Value::Array(arr) => {
            let items: Vec<serde_json::Value> = arr.elements.iter().map(|e| ast_value_to_serde(e)).collect();
            serde_json::Value::Array(items)
        }
    }
}
