use std::ops::Range;

use tower_lsp::lsp_types::Position;

/// Convert an LSP [`Position`] (0-based line, UTF-16 character offset) to a byte
/// offset into `content`. Returns `None` if the position is past the end of the
/// document.
pub fn position_to_offset(content: &str, position: Position) -> Option<usize> {
    let mut offset = 0usize;
    let target_line = position.line as usize;
    let target_char = position.character as usize;

    for (line_idx, line) in content.split('\n').enumerate() {
        if line_idx == target_line {
            let mut utf16_count = 0usize;
            for (byte_idx, ch) in line.char_indices() {
                if utf16_count == target_char {
                    return Some(offset + byte_idx);
                }
                utf16_count += ch.len_utf16();
            }
            // Cursor at end of line
            if utf16_count == target_char {
                return Some(offset + line.len());
            }
            return None;
        }
        // +1 for the '\n' separator
        offset += line.len() + 1;
    }
    None
}

/// Convert a byte offset into `content` to an LSP [`Position`].
///
/// # Panics
///
/// Panics if `offset` is greater than `content.len()`.
pub fn offset_to_position(content: &str, offset: usize) -> Position {
    assert!(offset <= content.len(), "offset {offset} exceeds content length {}", content.len());
    let before = &content[..offset];
    #[allow(clippy::cast_possible_truncation)] // line/char counts won't exceed u32 in practice
    let line = before.matches('\n').count() as u32;
    let line_start = before.rfind('\n').map_or(0, |pos| pos + 1);
    #[allow(clippy::cast_possible_truncation)]
    let character = content[line_start..offset].encode_utf16().count() as u32;
    Position { line, character }
}

/// Find the innermost `{…}` or `[…]` that encloses the given byte offset.
///
/// Correctly skips string literals, `//` line comments, and `/* */` block
/// comments so that brackets appearing inside them are ignored.
///
/// Returns `None` if the offset is not enclosed by any bracket pair.
pub fn find_enclosing_range(content: &str, offset: usize) -> Option<Range<usize>> {
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut stack: Vec<usize> = Vec::new();
    let mut result: Option<Range<usize>> = None;
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let b = bytes[i];

        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Line comment
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Block comment
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' | b'[' => stack.push(i),
            b'}' | b']' => {
                if let Some(open) = stack.pop() {
                    let close = i + 1;
                    if open < offset && offset < close && result.is_none() {
                        result = Some(open..close);
                    }
                }
            }
            _ => {}
        }

        i += 1;
    }

    result
}

/// Check whether a byte range covers the entire JSON document (the outermost
/// bracket pair with only whitespace around it).
pub fn is_root_range(content: &str, range: &Range<usize>) -> bool {
    let before = content[..range.start].trim();
    let after = content[range.end..].trim();
    before.is_empty() && after.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── position_to_offset ──────────────────────────────────────────

    #[test]
    fn position_to_offset_first_line() {
        let content = r#"{"a": 1}"#;
        assert_eq!(position_to_offset(content, Position { line: 0, character: 0 }), Some(0));
        assert_eq!(position_to_offset(content, Position { line: 0, character: 5 }), Some(5));
    }

    #[test]
    fn position_to_offset_multi_line() {
        let content = "{\n  \"a\": 1\n}";
        // Line 1, char 2 → byte offset for the `"` in `"a"`
        assert_eq!(position_to_offset(content, Position { line: 1, character: 2 }), Some(4));
        // Line 2, char 0 → the closing `}`
        assert_eq!(position_to_offset(content, Position { line: 2, character: 0 }), Some(11));
    }

    #[test]
    fn position_to_offset_past_end_returns_none() {
        let content = "{}";
        assert_eq!(position_to_offset(content, Position { line: 5, character: 0 }), None);
    }

    // ── offset_to_position ──────────────────────────────────────────

    #[test]
    fn offset_to_position_first_line() {
        let content = r#"{"a": 1}"#;
        assert_eq!(offset_to_position(content, 0), Position { line: 0, character: 0 });
        assert_eq!(offset_to_position(content, 5), Position { line: 0, character: 5 });
    }

    #[test]
    fn offset_to_position_multi_line() {
        let content = "{\n  \"a\": 1\n}";
        assert_eq!(offset_to_position(content, 4), Position { line: 1, character: 2 });
        assert_eq!(offset_to_position(content, 11), Position { line: 2, character: 0 });
    }

    #[test]
    fn round_trip_position_offset() {
        let content = "{\n  \"hello\": [\n    1,\n    2\n  ]\n}";
        for offset in 0..content.len() {
            // Only test at char boundaries
            if content.is_char_boundary(offset) {
                let pos = offset_to_position(content, offset);
                let back = position_to_offset(content, pos);
                assert_eq!(back, Some(offset), "round-trip failed at offset {offset}, pos {pos:?}");
            }
        }
    }

    // ── find_enclosing_range ────────────────────────────────────────

    #[test]
    fn simple_object() {
        let content = r#"{"a": 1, "b": 2}"#;
        // Cursor inside the object
        let range = find_enclosing_range(content, 5).unwrap();
        assert_eq!(range, 0..16);
    }

    #[test]
    fn simple_array() {
        let content = "[1, 2, 3]";
        let range = find_enclosing_range(content, 3).unwrap();
        assert_eq!(range, 0..9);
    }

    #[test]
    fn nested_returns_innermost() {
        let content = r#"{"a": {"c": 3, "b": 2}, "d": 4}"#;
        // Cursor inside the inner object {"c": 3, "b": 2}
        let range = find_enclosing_range(content, 10).unwrap();
        assert_eq!(&content[range], r#"{"c": 3, "b": 2}"#);
    }

    #[test]
    fn cursor_between_inner_objects_returns_outer() {
        let content = r#"{"a": {}, "b": {}}"#;
        // Cursor on the comma between the two inner objects
        let comma_pos = content.find(',').unwrap();
        let range = find_enclosing_range(content, comma_pos).unwrap();
        assert_eq!(range, 0..content.len());
    }

    #[test]
    fn brackets_inside_strings_are_ignored() {
        let content = r#"{"key": "val { } [ ]"}"#;
        // Cursor inside the string value — enclosing range is the outer object
        let range = find_enclosing_range(content, 12).unwrap();
        assert_eq!(range, 0..content.len());
    }

    #[test]
    fn jsonc_line_comment_with_brackets() {
        let content = "{\n  // ignore { this [\n  \"a\": 1\n}";
        let range = find_enclosing_range(content, 25).unwrap();
        assert_eq!(range, 0..content.len());
    }

    #[test]
    fn jsonc_block_comment_with_brackets() {
        let content = "{\n  /* { [ */ \"a\": 1\n}";
        let range = find_enclosing_range(content, 16).unwrap();
        assert_eq!(range, 0..content.len());
    }

    #[test]
    fn cursor_outside_brackets_returns_none() {
        let content = "  {} ";
        // Cursor on the leading whitespace
        assert!(find_enclosing_range(content, 0).is_none());
        // Cursor on the trailing whitespace
        assert!(find_enclosing_range(content, 4).is_none());
    }

    #[test]
    fn cursor_on_opening_bracket_returns_parent() {
        let content = r#"{"a": {"b": 1}}"#;
        // Cursor exactly on the inner `{` at index 6
        let inner_open = content[1..].find('{').unwrap() + 1;
        assert_eq!(inner_open, 6);
        // open < offset requires strict less-than, so cursor ON `{` matches the outer object
        let range = find_enclosing_range(content, inner_open).unwrap();
        assert_eq!(range, 0..content.len());
    }

    // ── is_root_range ───────────────────────────────────────────────

    #[test]
    fn root_range_detected() {
        let content = "  {\n  \"a\": 1\n}  \n";
        let range = find_enclosing_range(content, 6).unwrap();
        assert!(is_root_range(content, &range));
    }

    #[test]
    fn non_root_range() {
        let content = r#"{"a": {"b": 1}}"#;
        let inner = find_enclosing_range(content, 10).unwrap();
        assert!(!is_root_range(content, &inner));
    }
}
