# json-sort

A Rust library for sorting JSON and JSONC (JSON with Comments) content. Supports multiple sort strategies, directions, and depth control while preserving comments and trailing commas in JSONC input.

## Features

- **9 sort modes** — sort by key, value, key length, value length, or value type
- **3 directions** — ascending, descending, or random
- **Sort targets** — object keys, array elements, collections by key, or both objects and arrays
- **JSONC support** — line comments, block comments, and trailing commas are detected and preserved
- **Depth control** — sort all levels, top-level only, or up to a specific nesting depth
- **Range sorting** — sort a byte-range slice of a document, leaving the rest untouched

## Usage

```rust
use json_sort::{sort_json, SortOptions, SortBy, SortDirection, SortTarget};

// Sort with defaults (ascending, by key, object keys only)
let input = r#"{"b": 2, "a": 1, "c": 3}"#;
let sorted = sort_json(input, &SortOptions::default()).unwrap();

// Sort by value length, descending
let options = SortOptions {
    direction: SortDirection::Descending,
    sort_by: SortBy::ValueLength,
    target: SortTarget::ObjectKeys,
    ..Default::default()
};
let sorted = sort_json(input, &options).unwrap();
```

## Sort Options

| Option | Values | Default |
|---|---|---|
| `direction` | `Ascending`, `Descending`, `Random` | `Ascending` |
| `sort_by` | `Key`, `Value`, `KeyLength`, `ValueLength`, `ValueType` | `Key` |
| `target` | `ObjectKeys`, `ListItems`, `CollectionByKey(String)`, `Both` | `ObjectKeys` |
| `sort_level` | `-1` (unlimited), `0` (top-level only), `1+` (depth limit) | `-1` |
| `case_sensitive` | `true`, `false` | `false` |
| `indent` | `Spaces(n)`, `Tabs` | `Spaces(2)` |

## License

Apache-2.0
