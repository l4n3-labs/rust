/// Direction in which to sort entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Sort in ascending order (A→Z, 0→9).
    Ascending,
    /// Sort in descending order (Z→A, 9→0).
    Descending,
    /// Shuffle entries into a random order.
    Random,
}

/// Property used to compare entries during sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Compare by the property key name (alphabetical).
    Key,
    /// Compare by the JSON value (stringified).
    Value,
    /// Compare by the string length of the key name.
    KeyLength,
    /// Compare by the length/size of the value (string length, array/object element count).
    ValueLength,
    /// Group entries by JSON value type (bool, null, number, string, array, object).
    ValueType,
}

/// Which parts of the JSON structure to sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortTarget {
    /// Sort the properties of JSON objects.
    ObjectKeys,
    /// Sort the elements of JSON arrays.
    ListItems,
    /// Sort an array of objects by the value at the given key.
    CollectionByKey(String),
    /// Sort both object properties and array elements.
    Both,
}

/// Output indentation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    /// Indent with `n` spaces per nesting level.
    Spaces(u8),
    /// Indent with one tab per nesting level.
    Tabs,
}

/// Configuration for a sort operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortOptions {
    /// Direction of the sort (ascending, descending, or random).
    pub direction: SortDirection,
    /// Which property to compare entries by.
    pub sort_by: SortBy,
    /// Which parts of the JSON structure to sort.
    pub target: SortTarget,
    /// Maximum nesting depth to sort. `-1` means unlimited, `0` sorts only the top level.
    pub sort_level: i32,
    /// Whether string comparisons are case-sensitive.
    pub case_sensitive: bool,
    /// Indentation style for the output.
    pub indent: Indent,
}

/// Defaults: ascending, by key, object keys only, unlimited depth, case-insensitive, 2 spaces.
impl Default for SortOptions {
    fn default() -> Self {
        Self {
            direction: SortDirection::Ascending,
            sort_by: SortBy::Key,
            target: SortTarget::ObjectKeys,
            sort_level: -1,
            case_sensitive: false,
            indent: Indent::Spaces(2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sort_options() {
        let opts = SortOptions::default();
        assert_eq!(opts.direction, SortDirection::Ascending);
        assert_eq!(opts.sort_by, SortBy::Key);
        assert_eq!(opts.target, SortTarget::ObjectKeys);
        assert_eq!(opts.sort_level, -1);
        assert!(!opts.case_sensitive);
        assert_eq!(opts.indent, Indent::Spaces(2));
    }
}
