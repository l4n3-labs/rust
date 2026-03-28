use serde::Deserialize;

/// Top-level configuration parsed from LSP `initializationOptions`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Which sort scopes to offer in the code action menu.
    pub scopes: ScopeSettings,
    /// Default action toggles applied to scopes set to `true`.
    pub actions: ActionSettings,
}

/// Per-scope configuration. Each scope accepts `true` (use global actions),
/// `false` (disabled), or an inline [`ActionSettings`] object for per-scope overrides.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ScopeSettings {
    pub deep: ScopeConfig,
    pub shallow: ScopeConfig,
    pub subtree: ScopeConfig,
}

/// A single scope's configuration: a boolean toggle or per-scope action overrides.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ScopeConfig {
    /// `true` = scope enabled (uses global `actions`), `false` = scope disabled.
    Enabled(bool),
    /// Scope enabled with per-scope action toggles (independent of global `actions`).
    Actions(ActionSettings),
}

impl Default for ScopeConfig {
    fn default() -> Self {
        Self::Enabled(true)
    }
}

impl ScopeConfig {
    /// Whether this scope should produce any code actions at all.
    pub const fn is_enabled(&self) -> bool {
        match self {
            Self::Enabled(v) => *v,
            Self::Actions(_) => true,
        }
    }

    /// Whether a specific action (by index into the `ACTIONS` array) is enabled
    /// for this scope.
    pub const fn is_action_enabled(&self, index: usize, global: &ActionSettings) -> bool {
        match self {
            Self::Enabled(false) => false,
            Self::Enabled(true) => global.is_enabled(index),
            Self::Actions(overrides) => overrides.is_enabled(index),
        }
    }
}

/// Toggle individual sort strategies. Fields are ordered to match the `ACTIONS`
/// array in `actions.rs` so they can be looked up by index.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ActionSettings {
    pub ascending: bool,
    pub descending: bool,
    pub randomize: bool,
    pub by_value: bool,
    pub by_key_length: bool,
    pub by_value_length: bool,
    pub by_value_type: bool,
    pub sort_list_items: bool,
    pub sort_all: bool,
}

impl ActionSettings {
    /// Whether the action at the given index (into the `ACTIONS` array) is enabled.
    pub const fn is_enabled(&self, index: usize) -> bool {
        match index {
            0 => self.ascending,
            1 => self.descending,
            2 => self.randomize,
            3 => self.by_value,
            4 => self.by_key_length,
            5 => self.by_value_length,
            6 => self.by_value_type,
            7 => self.sort_list_items,
            8 => self.sort_all,
            _ => true,
        }
    }
}

impl Default for ActionSettings {
    fn default() -> Self {
        Self {
            ascending: true,
            descending: true,
            randomize: true,
            by_value: true,
            by_key_length: true,
            by_value_length: true,
            by_value_type: true,
            sort_list_items: true,
            sort_all: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_json_defaults_to_all_enabled() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(s.scopes.deep.is_enabled());
        assert!(s.scopes.shallow.is_enabled());
        assert!(s.scopes.subtree.is_enabled());
        for i in 0..9 {
            assert!(s.actions.is_enabled(i), "action {i} should default to enabled");
        }
    }

    #[test]
    fn scope_false_disables_scope() {
        let json = r#"{"scopes": {"subtree": false}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.scopes.deep.is_enabled());
        assert!(s.scopes.shallow.is_enabled());
        assert!(!s.scopes.subtree.is_enabled());
    }

    #[test]
    fn scope_true_uses_global_actions() {
        let json = r#"{"scopes": {"deep": true}, "actions": {"randomize": false}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.scopes.deep.is_action_enabled(0, &s.actions)); // ascending
        assert!(!s.scopes.deep.is_action_enabled(2, &s.actions)); // randomize
    }

    #[test]
    fn scope_object_overrides_independently() {
        let json = r#"{
            "actions": {"ascending": false},
            "scopes": {"deep": {"ascending": true, "descending": false}}
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        // Per-scope: ascending enabled, descending disabled
        assert!(s.scopes.deep.is_action_enabled(0, &s.actions));
        assert!(!s.scopes.deep.is_action_enabled(1, &s.actions));
        // Global ascending=false doesn't affect the per-scope override
        // Shallow still uses global
        assert!(!s.scopes.shallow.is_action_enabled(0, &s.actions));
    }

    #[test]
    fn scope_object_defaults_unspecified_actions_to_true() {
        let json = r#"{"scopes": {"deep": {"randomize": false}}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.scopes.deep.is_action_enabled(0, &s.actions)); // ascending
        assert!(!s.scopes.deep.is_action_enabled(2, &s.actions)); // randomize
        assert!(s.scopes.deep.is_action_enabled(8, &s.actions)); // sort_all
    }

    #[test]
    fn global_action_disabled() {
        let json = r#"{"actions": {"randomize": false, "by_value": false}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.actions.is_enabled(0)); // ascending
        assert!(!s.actions.is_enabled(2)); // randomize
        assert!(!s.actions.is_enabled(3)); // by_value
        assert!(s.actions.is_enabled(8)); // sort_all
    }

    #[test]
    fn full_override_all_disabled() {
        let json = r#"{
            "scopes": {"deep": false, "shallow": false, "subtree": false},
            "actions": {
                "ascending": false, "descending": false, "randomize": false,
                "by_value": false, "by_key_length": false, "by_value_length": false,
                "by_value_type": false, "sort_list_items": false, "sort_all": false
            }
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(!s.scopes.deep.is_enabled());
        assert!(!s.scopes.shallow.is_enabled());
        assert!(!s.scopes.subtree.is_enabled());
        for i in 0..9 {
            assert!(!s.actions.is_enabled(i));
        }
    }

    #[test]
    fn unknown_action_index_defaults_to_true() {
        let s = ActionSettings::default();
        assert!(s.is_enabled(99));
    }

    #[test]
    fn scope_config_enabled_false_rejects_all_actions() {
        let config = ScopeConfig::Enabled(false);
        let global = ActionSettings::default();
        for i in 0..9 {
            assert!(!config.is_action_enabled(i, &global));
        }
    }

    #[test]
    fn from_value_matches_real_initialization_options() {
        // Reproduce the exact flow: Zed passes a serde_json::Value to initialize().
        let json_val: serde_json::Value =
            serde_json::from_str(r#"{"scopes": {"shallow": false, "subtree": false}}"#).unwrap();

        // This is what initialize() does:
        let result = serde_json::from_value::<Settings>(json_val);
        let s = result.expect("deserialization must succeed");
        assert!(s.scopes.deep.is_enabled(), "deep should default to enabled");
        assert!(!s.scopes.shallow.is_enabled(), "shallow should be disabled");
        assert!(!s.scopes.subtree.is_enabled(), "subtree should be disabled");
    }

    #[test]
    fn mixed_scope_types() {
        let json = r#"{
            "scopes": {
                "deep": true,
                "shallow": false,
                "subtree": {"ascending": true, "descending": false}
            }
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.scopes.deep.is_enabled());
        assert!(!s.scopes.shallow.is_enabled());
        assert!(s.scopes.subtree.is_enabled());
        assert!(s.scopes.subtree.is_action_enabled(0, &s.actions));
        assert!(!s.scopes.subtree.is_action_enabled(1, &s.actions));
    }
}
