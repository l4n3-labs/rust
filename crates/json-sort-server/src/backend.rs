use std::collections::HashMap;
use std::ops::Range as StdRange;
use std::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, Position, Range, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url, WorkDoneProgressOptions, WorkspaceEdit,
};
use tower_lsp::{Client, LanguageServer};

use crate::cursor;
use crate::documents::DocumentStore;
use crate::settings::Settings;

/// LSP backend that holds the client connection and open document state.
pub struct Backend {
    #[allow(dead_code)]
    client: Client,
    /// In-memory store for open document contents.
    pub documents: DocumentStore,
    /// User-configurable settings parsed from `initializationOptions`.
    settings: RwLock<Settings>,
}

impl Backend {
    /// Create a new backend with an empty document store.
    pub fn new(client: Client) -> Self {
        Self { client, documents: DocumentStore::new(), settings: RwLock::new(Settings::default()) }
    }
}

/// Look up a sort action by index, build its options, and sort the content.
///
/// When `shallow` is true, overrides `sort_level` to `0` so only the top level
/// is sorted.
///
/// Returns `None` if the index is out of bounds or sorting fails.
pub fn resolve_sort_action(content: &str, action_index: usize, shallow: bool) -> Option<String> {
    let action_def = crate::actions::ACTIONS.get(action_index)?;
    let mut options = (action_def.options)();
    if shallow {
        options.sort_level = 0;
    }
    json_sort::sort_json(content, &options).ok()
}

/// Sort only a byte-range slice of the document.
///
/// Returns the sorted slice text, or `None` if the index/range is invalid.
pub fn resolve_sort_action_range(content: &str, action_index: usize, range: StdRange<usize>) -> Option<String> {
    let action_def = crate::actions::ACTIONS.get(action_index)?;
    let options = (action_def.options)();
    let slice = content.get(range)?;
    json_sort::sort_json(slice, &options).ok()
}

/// Build the list of code action stubs for a given document and cursor position.
///
/// Extracted from the `LanguageServer::code_action` handler so it can be tested
/// without constructing a full [`Backend`] (which requires a `tower_lsp::Client`).
pub fn build_code_actions(content: &str, uri: &Url, cursor: Position, settings: &Settings) -> Vec<CodeActionOrCommand> {
    let mut actions: Vec<CodeActionOrCommand> = Vec::new();
    let enabled_actions = |scope: &crate::settings::ScopeConfig| {
        let scope = scope.clone();
        let global = settings.actions.clone();
        crate::actions::ACTIONS.iter().enumerate().filter(move |(i, _)| scope.is_action_enabled(*i, &global))
    };

    // Deep Sort — sorts entire document recursively (default).
    if settings.scopes.deep.is_enabled() {
        actions.extend(enabled_actions(&settings.scopes.deep).map(|(i, def)| {
            CodeActionOrCommand::CodeAction(CodeAction {
                title: def.title.to_string(),
                kind: Some(CodeActionKind::REFACTOR_REWRITE),
                data: Some(serde_json::json!({
                    "action_index": i,
                    "uri": uri.to_string(),
                })),
                ..Default::default()
            })
        }));
    }

    // Shallow Sort — sorts only top-level keys of the root object.
    if settings.scopes.shallow.is_enabled() {
        actions.extend(enabled_actions(&settings.scopes.shallow).map(|(i, def)| {
            CodeActionOrCommand::CodeAction(CodeAction {
                title: crate::actions::shallow_title(def),
                kind: Some(CodeActionKind::REFACTOR_REWRITE),
                data: Some(serde_json::json!({
                    "action_index": i,
                    "uri": uri.to_string(),
                    "shallow": true,
                })),
                ..Default::default()
            })
        }));
    }

    // Subtree Sort — sorts the innermost object/array under the cursor.
    if settings.scopes.subtree.is_enabled()
        && let Some(offset) = cursor::position_to_offset(content, cursor)
        && let Some(enc_range) = cursor::find_enclosing_range(content, offset)
        && !cursor::is_root_range(content, &enc_range)
    {
        actions.extend(enabled_actions(&settings.scopes.subtree).map(|(i, def)| {
            CodeActionOrCommand::CodeAction(CodeAction {
                title: crate::actions::subtree_title(def),
                kind: Some(CodeActionKind::REFACTOR_REWRITE),
                data: Some(serde_json::json!({
                    "action_index": i,
                    "uri": uri.to_string(),
                    "range_start": enc_range.start,
                    "range_end": enc_range.end,
                })),
                ..Default::default()
            })
        }));
    }

    actions
}

/// Resolve a code action stub into a concrete workspace edit.
///
/// Extracted from the `LanguageServer::code_action_resolve` handler for testability.
pub fn resolve_code_action(content: &str, mut action: CodeAction) -> CodeAction {
    let Some(data) = &action.data else { return action };
    let action_index = usize::try_from(data["action_index"].as_u64().unwrap_or(0)).unwrap_or(0);
    let uri_str = data["uri"].as_str().unwrap_or_default();
    let Ok(uri) = Url::parse(uri_str) else { return action };

    let shallow = data["shallow"].as_bool().unwrap_or(false);

    let Some((edit_range, sorted)) =
        (if let (Some(start), Some(end)) = (data["range_start"].as_u64(), data["range_end"].as_u64()) {
            // Subtree sort: sort only the byte range.
            #[allow(clippy::cast_possible_truncation)]
            let byte_range = (start as usize)..(end as usize);
            let start_pos = cursor::offset_to_position(content, byte_range.start);
            let end_pos = cursor::offset_to_position(content, byte_range.end);
            resolve_sort_action_range(content, action_index, byte_range)
                .map(|sorted| (Range { start: start_pos, end: end_pos }, sorted))
        } else {
            // Deep or shallow whole-file sort.
            resolve_sort_action(content, action_index, shallow).map(|sorted| {
                (
                    Range {
                        start: Position { line: 0, character: 0 },
                        end: Position { line: u32::MAX, character: u32::MAX },
                    },
                    sorted,
                )
            })
        })
    else {
        return action;
    };

    let mut changes = HashMap::new();
    changes.insert(uri, vec![TextEdit { range: edit_range, new_text: sorted }]);
    action.edit = Some(WorkspaceEdit { changes: Some(changes), ..Default::default() });
    action
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(opts) = params.initialization_options
            && let Ok(s) = serde_json::from_value::<Settings>(opts)
        {
            *self.settings.write().unwrap() = s;
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                    code_action_kinds: Some(vec![CodeActionKind::REFACTOR_REWRITE]),
                    resolve_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "json-sort-server".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.documents.open(params.text_document.uri, params.text_document.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().next() {
            self.documents.update(&params.text_document.uri, change.text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.close(&params.text_document.uri);
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let Some(content) = self.documents.get(uri) else {
            return Ok(None);
        };
        let settings = self.settings.read().unwrap().clone();
        Ok(Some(build_code_actions(&content, uri, params.range.start, &settings)))
    }

    async fn code_action_resolve(&self, action: CodeAction) -> Result<CodeAction> {
        let Some(data) = &action.data else { return Ok(action) };
        let uri_str = data["uri"].as_str().unwrap_or_default();
        let Ok(uri) = Url::parse(uri_str) else { return Ok(action) };
        let Some(content) = self.documents.get(&uri) else { return Ok(action) };
        Ok(resolve_code_action(&content, action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_deep_ascending_sort() {
        let content = r#"{"c": 3, "a": 1, "b": 2}"#;
        let result = resolve_sort_action(content, 0, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn resolve_deep_descending_sort() {
        let content = r#"{"a": 1, "c": 3, "b": 2}"#;
        let result = resolve_sort_action(content, 1, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["c", "b", "a"]);
    }

    #[test]
    fn resolve_deep_sort_list_items() {
        let content = r"[3, 1, 2]";
        let result = resolve_sort_action(content, 7, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn resolve_deep_sort_both() {
        let content = r#"{"b": [3, 1], "a": 1}"#;
        let result = resolve_sort_action(content, 8, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(parsed["b"], serde_json::json!([1, 3]));
    }

    #[test]
    fn resolve_shallow_sort_only_top_level() {
        let content = r#"{"b": {"z": 1, "a": 2}, "a": 1}"#;
        let result = resolve_sort_action(content, 0, true).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        // Top-level keys are sorted
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        // Nested keys remain unsorted (z before a)
        let inner_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(inner_keys, vec!["z", "a"]);
    }

    #[test]
    fn resolve_deep_sort_recurses_into_nested_objects() {
        let content = r#"{"b": {"z": 1, "a": 2}, "a": 1}"#;
        let result = resolve_sort_action(content, 0, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        // Nested keys are also sorted
        let inner_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(inner_keys, vec!["a", "z"]);
    }

    #[test]
    fn resolve_shallow_sort_leaves_nested_arrays_unsorted() {
        let content = r#"{"b": [3, 1, 2], "a": 1}"#;
        let result = resolve_sort_action(content, 8, true).unwrap(); // index 8 = Sort All
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        // Top-level keys sorted
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        // Nested array remains unsorted
        assert_eq!(parsed["b"], serde_json::json!([3, 1, 2]));
    }

    #[test]
    fn resolve_shallow_descending_sort() {
        let content = r#"{"a": 1, "c": 3, "b": 2}"#;
        let result = resolve_sort_action(content, 1, true).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["c", "b", "a"]);
    }

    #[test]
    fn resolve_invalid_index_returns_none() {
        let result = resolve_sort_action("{}", 99, false);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_invalid_json_returns_none() {
        let result = resolve_sort_action("not json", 0, false);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_range_sorts_inner_object() {
        let content = r#"{"z": 1, "inner": {"c": 3, "a": 1, "b": 2}, "m": 0}"#;
        // Find the byte range of the inner object
        let inner_start = content.find(r#"{"c"#).unwrap();
        let inner_end = content[inner_start..].find('}').unwrap() + inner_start + 1;
        let result = resolve_sort_action_range(content, 0, inner_start..inner_end).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn resolve_range_invalid_range_returns_none() {
        let content = r#"{"a": 1}"#;
        let result = resolve_sort_action_range(content, 0, 100..200);
        assert!(result.is_none());
    }

    // ── build_code_actions ──────────────────────────────────────────

    fn test_uri() -> Url {
        Url::parse("file:///test.json").unwrap()
    }

    fn extract_titles(actions: &[CodeActionOrCommand]) -> Vec<&str> {
        actions
            .iter()
            .filter_map(|a| match a {
                CodeActionOrCommand::CodeAction(ca) => Some(ca.title.as_str()),
                CodeActionOrCommand::Command(_) => None,
            })
            .collect()
    }

    fn extract_action(actions: &[CodeActionOrCommand], index: usize) -> &CodeAction {
        match &actions[index] {
            CodeActionOrCommand::CodeAction(ca) => ca,
            CodeActionOrCommand::Command(_) => panic!("expected CodeAction at index {index}"),
        }
    }

    #[test]
    fn code_action_cursor_at_root_returns_deep_and_shallow_only() {
        // Cursor inside the root object — no subtree actions should appear.
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());
        // 9 deep + 9 shallow = 18
        assert_eq!(actions.len(), 18);
        let titles = extract_titles(&actions);
        for title in &titles[..9] {
            assert!(title.starts_with("Deep Sort:"), "expected Deep Sort title, got: {title}");
        }
        for title in &titles[9..] {
            assert!(title.starts_with("Shallow Sort:"), "expected Shallow Sort title, got: {title}");
        }
    }

    #[test]
    fn code_action_cursor_in_nested_object_returns_all_three_groups() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());
        // 9 deep + 9 shallow + 9 subtree = 27
        assert_eq!(actions.len(), 27);

        let titles = extract_titles(&actions);
        for title in &titles[..9] {
            assert!(title.starts_with("Deep Sort:"), "expected Deep Sort, got: {title}");
        }
        for title in &titles[9..18] {
            assert!(title.starts_with("Shallow Sort:"), "expected Shallow Sort, got: {title}");
        }
        for title in &titles[18..] {
            assert!(title.starts_with("Subtree Sort:"), "expected Subtree Sort, got: {title}");
        }
    }

    #[test]
    fn code_action_subtree_stubs_carry_range_metadata() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());

        // Index 18 = first subtree action (after 9 deep + 9 shallow)
        let first_subtree = extract_action(&actions, 18);
        let data = first_subtree.data.as_ref().unwrap();
        assert!(data["range_start"].is_u64());
        assert!(data["range_end"].is_u64());

        let start = usize::try_from(data["range_start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(data["range_end"].as_u64().unwrap()).unwrap();
        assert_eq!(&content[start..end], r#"{"c": 3, "a": 1}"#);
    }

    #[test]
    fn code_action_shallow_stubs_carry_shallow_flag() {
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());

        // Index 9 = first shallow action
        let first_shallow = extract_action(&actions, 9);
        let data = first_shallow.data.as_ref().unwrap();
        assert_eq!(data["shallow"].as_bool(), Some(true));
    }

    #[test]
    fn code_action_deep_stubs_have_no_range_or_shallow() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());

        let first_deep = extract_action(&actions, 0);
        let data = first_deep.data.as_ref().unwrap();
        assert!(data.get("range_start").is_none() || data["range_start"].is_null());
        assert!(data.get("shallow").is_none() || data["shallow"].is_null());
    }

    #[test]
    fn code_action_cursor_in_nested_array_returns_subtree_actions() {
        let content = r#"{"items": [3, 1, 2]}"#;
        let cursor = Position { line: 0, character: 12 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());
        // 9 deep + 9 shallow + 9 subtree = 27
        assert_eq!(actions.len(), 27);

        // Index 18 = first subtree action
        let first_subtree = extract_action(&actions, 18);
        let data = first_subtree.data.as_ref().unwrap();
        let start = usize::try_from(data["range_start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(data["range_end"].as_u64().unwrap()).unwrap();
        assert_eq!(&content[start..end], "[3, 1, 2]");
    }

    #[test]
    fn code_action_shallow_stubs_have_no_range_metadata() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());

        // Index 9 = first shallow action
        let first_shallow = extract_action(&actions, 9);
        let data = first_shallow.data.as_ref().unwrap();
        assert!(data.get("range_start").is_none() || data["range_start"].is_null());
        assert!(data.get("range_end").is_none() || data["range_end"].is_null());
    }

    #[test]
    fn code_action_all_stubs_have_refactor_rewrite_kind() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());

        for (i, action) in actions.iter().enumerate() {
            let CodeActionOrCommand::CodeAction(ca) = action else {
                panic!("expected CodeAction at index {i}");
            };
            assert_eq!(ca.kind, Some(CodeActionKind::REFACTOR_REWRITE), "wrong kind at index {i}");
        }
    }

    #[test]
    fn code_action_primitive_content_returns_deep_and_shallow_only() {
        // A bare primitive has no enclosing brackets for subtree
        let content = "42";
        let cursor = Position { line: 0, character: 0 };
        let actions = build_code_actions(content, &test_uri(), cursor, &Settings::default());
        // 9 deep + 9 shallow, no subtree
        assert_eq!(actions.len(), 18);
    }

    #[test]
    fn code_action_all_stubs_carry_action_index_and_uri() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let uri = test_uri();
        let actions = build_code_actions(content, &uri, cursor, &Settings::default());

        for (i, action) in actions.iter().enumerate() {
            let CodeActionOrCommand::CodeAction(ca) = action else {
                panic!("expected CodeAction at index {i}");
            };
            let data = ca.data.as_ref().unwrap();
            assert!(data["action_index"].is_u64(), "missing action_index at {i}");
            assert_eq!(data["uri"].as_str().unwrap(), uri.as_str(), "wrong uri at {i}");
        }
    }

    // ── resolve_code_action ─────────────────────────────────────────

    #[test]
    fn resolve_subtree_action_produces_scoped_edit() {
        let content = r#"{"outer": {"c": 3, "a": 1, "b": 2}, "z": 0}"#;
        let inner_start = content.find(r#"{"c"#).unwrap();
        let inner_end = content[inner_start..].find('}').unwrap() + inner_start + 1;

        let stub = CodeAction {
            title: "Subtree Sort: Ascending".to_string(),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            data: Some(serde_json::json!({
                "action_index": 0,
                "uri": test_uri().to_string(),
                "range_start": inner_start,
                "range_end": inner_end,
            })),
            ..Default::default()
        };

        let resolved = resolve_code_action(content, stub);
        let edit = resolved.edit.expect("expected a workspace edit");
        let changes = edit.changes.expect("expected changes");
        let edits = &changes[&test_uri()];
        assert_eq!(edits.len(), 1);

        // The edit range should cover only the inner object, not the whole file.
        let text_edit = &edits[0];
        assert_ne!(text_edit.range.start, Position { line: 0, character: 0 });
        assert_ne!(text_edit.range.end, Position { line: u32::MAX, character: u32::MAX });

        // Start position should correspond to the inner object's byte offset.
        let expected_start = cursor::offset_to_position(content, inner_start);
        let expected_end = cursor::offset_to_position(content, inner_end);
        assert_eq!(text_edit.range.start, expected_start);
        assert_eq!(text_edit.range.end, expected_end);

        // The sorted text should have keys in ascending order.
        let parsed: serde_json::Value = serde_json::from_str(&text_edit.new_text).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn resolve_deep_action_produces_full_document_edit() {
        let content = r#"{"c": 3, "a": 1, "b": 2}"#;
        let stub = CodeAction {
            title: "Deep Sort: Ascending".to_string(),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            data: Some(serde_json::json!({
                "action_index": 0,
                "uri": test_uri().to_string(),
            })),
            ..Default::default()
        };

        let resolved = resolve_code_action(content, stub);
        let edit = resolved.edit.expect("expected a workspace edit");
        let changes = edit.changes.expect("expected changes");
        let edits = &changes[&test_uri()];
        assert_eq!(edits.len(), 1);

        let text_edit = &edits[0];
        assert_eq!(text_edit.range.start, Position { line: 0, character: 0 });
        assert_eq!(text_edit.range.end, Position { line: u32::MAX, character: u32::MAX });
    }

    #[test]
    fn resolve_shallow_action_produces_full_document_edit_with_shallow_sort() {
        let content = r#"{"b": {"z": 1, "a": 2}, "a": 1}"#;
        let stub = CodeAction {
            title: "Shallow Sort: Ascending".to_string(),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            data: Some(serde_json::json!({
                "action_index": 0,
                "uri": test_uri().to_string(),
                "shallow": true,
            })),
            ..Default::default()
        };

        let resolved = resolve_code_action(content, stub);
        let edit = resolved.edit.expect("expected a workspace edit");
        let changes = edit.changes.expect("expected changes");
        let edits = &changes[&test_uri()];
        let text_edit = &edits[0];

        // Full-document range
        assert_eq!(text_edit.range.start, Position { line: 0, character: 0 });
        assert_eq!(text_edit.range.end, Position { line: u32::MAX, character: u32::MAX });

        // Top-level keys sorted, nested keys untouched
        let parsed: serde_json::Value = serde_json::from_str(&text_edit.new_text).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        let inner_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(inner_keys, vec!["z", "a"]);
    }

    #[test]
    fn resolve_deep_action_sorts_recursively() {
        let content = r#"{"b": {"z": 1, "a": 2}, "a": 1}"#;
        let stub = CodeAction {
            title: "Deep Sort: Ascending".to_string(),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            data: Some(serde_json::json!({
                "action_index": 0,
                "uri": test_uri().to_string(),
            })),
            ..Default::default()
        };

        let resolved = resolve_code_action(content, stub);
        let edit = resolved.edit.unwrap();
        let edits = &edit.changes.unwrap()[&test_uri()];
        let parsed: serde_json::Value = serde_json::from_str(&edits[0].new_text).unwrap();
        let inner_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(inner_keys, vec!["a", "z"]);
    }

    #[test]
    fn resolve_subtree_action_sorts_recursively_within_subtree() {
        let content = r#"{"outer": {"b": {"z": 1, "a": 2}, "a": 1}, "keep": 0}"#;
        let inner_start = content.find(r#"{"b"#).unwrap();
        let inner_end = {
            // Find the matching closing brace for the inner object
            let mut depth = 0i32;
            let mut end = inner_start;
            for (i, b) in content[inner_start..].bytes().enumerate() {
                match b {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = inner_start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            end
        };

        let stub = CodeAction {
            title: "Subtree Sort: Ascending".to_string(),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            data: Some(serde_json::json!({
                "action_index": 0,
                "uri": test_uri().to_string(),
                "range_start": inner_start,
                "range_end": inner_end,
            })),
            ..Default::default()
        };

        let resolved = resolve_code_action(content, stub);
        let edit = resolved.edit.unwrap();
        let edits = &edit.changes.unwrap()[&test_uri()];
        let parsed: serde_json::Value = serde_json::from_str(&edits[0].new_text).unwrap();
        // Top-level of subtree sorted
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        // Nested within subtree also sorted
        let inner_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(inner_keys, vec!["a", "z"]);
    }

    #[test]
    fn resolve_action_invalid_uri_returns_unchanged() {
        let stub = CodeAction {
            title: "Deep Sort: Ascending".to_string(),
            data: Some(serde_json::json!({
                "action_index": 0,
                "uri": "not a valid uri",
            })),
            ..Default::default()
        };
        let resolved = resolve_code_action("{}", stub);
        assert!(resolved.edit.is_none());
    }

    #[test]
    fn resolve_action_no_data_returns_unchanged() {
        let stub = CodeAction { title: "test".to_string(), data: None, ..Default::default() };
        let resolved = resolve_code_action("{}", stub);
        assert!(resolved.edit.is_none());
    }

    #[test]
    fn resolve_action_missing_shallow_flag_defaults_to_deep() {
        let content = r#"{"b": {"z": 1, "a": 2}, "a": 1}"#;
        let stub = CodeAction {
            title: "Deep Sort: Ascending".to_string(),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            data: Some(serde_json::json!({
                "action_index": 0,
                "uri": test_uri().to_string(),
                // no "shallow" key at all
            })),
            ..Default::default()
        };

        let resolved = resolve_code_action(content, stub);
        let edit = resolved.edit.unwrap();
        let edits = &edit.changes.unwrap()[&test_uri()];
        let parsed: serde_json::Value = serde_json::from_str(&edits[0].new_text).unwrap();
        // Nested keys should be sorted (deep behavior)
        let inner_keys: Vec<&String> = parsed["b"].as_object().unwrap().keys().collect();
        assert_eq!(inner_keys, vec!["a", "z"]);
    }

    // ── settings filtering ─────────────────────────────────────────

    use crate::settings::{ActionSettings, ScopeConfig, ScopeSettings};

    #[test]
    fn settings_deep_scope_disabled_omits_deep_actions() {
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        let settings = Settings {
            scopes: ScopeSettings { deep: ScopeConfig::Enabled(false), ..Default::default() },
            ..Default::default()
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        // Only shallow (9), no deep
        assert_eq!(actions.len(), 9);
        let titles = extract_titles(&actions);
        for title in &titles {
            assert!(title.starts_with("Shallow Sort:"), "unexpected: {title}");
        }
    }

    #[test]
    fn settings_shallow_scope_disabled_omits_shallow_actions() {
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        let settings = Settings {
            scopes: ScopeSettings { shallow: ScopeConfig::Enabled(false), ..Default::default() },
            ..Default::default()
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        assert_eq!(actions.len(), 9);
        let titles = extract_titles(&actions);
        for title in &titles {
            assert!(title.starts_with("Deep Sort:"), "unexpected: {title}");
        }
    }

    #[test]
    fn settings_subtree_scope_disabled_omits_subtree_actions() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let settings = Settings {
            scopes: ScopeSettings { subtree: ScopeConfig::Enabled(false), ..Default::default() },
            ..Default::default()
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        assert_eq!(actions.len(), 18);
    }

    #[test]
    fn settings_all_scopes_disabled_returns_empty() {
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        let settings = Settings {
            scopes: ScopeSettings {
                deep: ScopeConfig::Enabled(false),
                shallow: ScopeConfig::Enabled(false),
                subtree: ScopeConfig::Enabled(false),
            },
            ..Default::default()
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        assert!(actions.is_empty());
    }

    #[test]
    fn settings_global_action_disabled_omits_from_all_scopes() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        let settings =
            Settings { actions: ActionSettings { randomize: false, ..Default::default() }, ..Default::default() };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        // 8 per scope × 3 scopes = 24
        assert_eq!(actions.len(), 24);
        let titles = extract_titles(&actions);
        for title in &titles {
            assert!(!title.contains("Randomize"), "randomize should be filtered: {title}");
        }
    }

    #[test]
    fn settings_all_global_actions_disabled_returns_empty() {
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        let settings = Settings {
            actions: ActionSettings {
                ascending: false,
                descending: false,
                randomize: false,
                by_value: false,
                by_key_length: false,
                by_value_length: false,
                by_value_type: false,
                sort_list_items: false,
                sort_all: false,
            },
            ..Default::default()
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        assert!(actions.is_empty());
    }

    #[test]
    fn settings_mixed_scopes_and_global_actions() {
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        let settings = Settings {
            scopes: ScopeSettings {
                deep: ScopeConfig::Enabled(true),
                shallow: ScopeConfig::Enabled(false),
                subtree: ScopeConfig::Enabled(true),
            },
            actions: ActionSettings {
                ascending: true,
                descending: true,
                randomize: false,
                by_value: false,
                by_key_length: false,
                by_value_length: false,
                by_value_type: false,
                sort_list_items: false,
                sort_all: false,
            },
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        // Only deep scope at root (no subtree at root), 2 enabled actions
        assert_eq!(actions.len(), 2);
        let titles = extract_titles(&actions);
        assert_eq!(titles, vec!["Deep Sort: Ascending", "Deep Sort: Descending"]);
    }

    #[test]
    fn settings_per_scope_action_overrides() {
        let content = r#"{"outer": {"c": 3, "a": 1}}"#;
        let cursor = Position { line: 0, character: 14 };
        // Global disables randomize; deep uses global; subtree overrides to enable only randomize
        let settings = Settings {
            scopes: ScopeSettings {
                deep: ScopeConfig::Enabled(true),
                shallow: ScopeConfig::Enabled(false),
                subtree: ScopeConfig::Actions(ActionSettings {
                    ascending: false,
                    descending: false,
                    randomize: true,
                    by_value: false,
                    by_key_length: false,
                    by_value_length: false,
                    by_value_type: false,
                    sort_list_items: false,
                    sort_all: false,
                }),
            },
            actions: ActionSettings { randomize: false, ..Default::default() },
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        let titles = extract_titles(&actions);
        // Deep: 8 actions (global minus randomize), Subtree: 1 action (only randomize)
        assert_eq!(actions.len(), 9);
        // Deep should not have Randomize
        assert_eq!(titles.iter().filter(|t| t.starts_with("Deep Sort:")).count(), 8);
        assert!(!titles.iter().any(|t| t.starts_with("Deep Sort:") && t.contains("Randomize")));
        // Subtree should only have Randomize
        assert_eq!(titles.iter().filter(|t| t.starts_with("Subtree Sort:")).count(), 1);
        assert!(titles.iter().any(|t| t == &"Subtree Sort: Randomize"));
    }

    #[test]
    fn settings_per_scope_object_ignores_global_actions() {
        let content = r#"{"b": 2, "a": 1}"#;
        let cursor = Position { line: 0, character: 5 };
        // Global disables ascending, but deep scope object enables it
        let settings = Settings {
            scopes: ScopeSettings {
                deep: ScopeConfig::Actions(ActionSettings { ascending: true, ..Default::default() }),
                shallow: ScopeConfig::Enabled(true),
                ..Default::default()
            },
            actions: ActionSettings { ascending: false, ..Default::default() },
        };
        let actions = build_code_actions(content, &test_uri(), cursor, &settings);
        let titles = extract_titles(&actions);
        // Deep has all 9 (ascending enabled per-scope)
        assert_eq!(titles.iter().filter(|t| t.starts_with("Deep Sort:")).count(), 9);
        // Shallow uses global: 8 (ascending disabled)
        assert_eq!(titles.iter().filter(|t| t.starts_with("Shallow Sort:")).count(), 8);
        assert!(!titles.iter().any(|t| t.starts_with("Shallow Sort:") && t.contains("Ascending")));
    }
}
