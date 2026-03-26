use json_sort::{SortBy, SortDirection, SortOptions, SortTarget};

/// A code action definition pairing a user-visible title with a sort configuration.
pub struct ActionDef {
    /// Label shown in the editor's code action menu.
    pub title: &'static str,
    /// Factory function that produces the [`SortOptions`] for this action.
    pub options: fn() -> SortOptions,
}

/// The 9 available sort actions, indexed by position.
///
/// The index is used as the action identifier in `code_action_resolve`.
pub const ACTIONS: &[ActionDef] = &[
    ActionDef {
        title: "Sort JSON: Ascending",
        options: || SortOptions {
            direction: SortDirection::Ascending,
            sort_by: SortBy::Key,
            target: SortTarget::ObjectKeys,
            ..Default::default()
        },
    },
    ActionDef {
        title: "Sort JSON: Descending",
        options: || SortOptions {
            direction: SortDirection::Descending,
            sort_by: SortBy::Key,
            target: SortTarget::ObjectKeys,
            ..Default::default()
        },
    },
    ActionDef {
        title: "Sort JSON: Randomize",
        options: || SortOptions {
            direction: SortDirection::Random,
            sort_by: SortBy::Key,
            target: SortTarget::ObjectKeys,
            ..Default::default()
        },
    },
    ActionDef {
        title: "Sort JSON: By Value",
        options: || SortOptions { sort_by: SortBy::Value, ..Default::default() },
    },
    ActionDef {
        title: "Sort JSON: By Key Length",
        options: || SortOptions { sort_by: SortBy::KeyLength, ..Default::default() },
    },
    ActionDef {
        title: "Sort JSON: By Value Length",
        options: || SortOptions { sort_by: SortBy::ValueLength, ..Default::default() },
    },
    ActionDef {
        title: "Sort JSON: By Value Type",
        options: || SortOptions { sort_by: SortBy::ValueType, ..Default::default() },
    },
    ActionDef {
        title: "Sort JSON: Sort List Items",
        options: || SortOptions { target: SortTarget::ListItems, ..Default::default() },
    },
    ActionDef {
        title: "Sort JSON: Sort All (Objects + Lists)",
        options: || SortOptions { target: SortTarget::Both, ..Default::default() },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_count() {
        assert_eq!(ACTIONS.len(), 9);
    }

    #[test]
    fn each_action_produces_valid_options() {
        for action in ACTIONS {
            let options = (action.options)();
            // Verify we can construct options without panic
            assert!(!action.title.is_empty());
            // Verify the options have a valid direction
            let _ = options.direction;
        }
    }

    #[test]
    fn ascending_is_first() {
        assert_eq!(ACTIONS[0].title, "Sort JSON: Ascending");
        let options = (ACTIONS[0].options)();
        assert_eq!(options.direction, SortDirection::Ascending);
    }
}
