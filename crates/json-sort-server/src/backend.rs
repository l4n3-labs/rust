use std::collections::HashMap;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, Position, Range, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url, WorkDoneProgressOptions, WorkspaceEdit,
};
use tower_lsp::{Client, LanguageServer};

use crate::documents::DocumentStore;

/// LSP backend that holds the client connection and open document state.
pub struct Backend {
    #[allow(dead_code)]
    client: Client,
    /// In-memory store for open document contents.
    pub documents: DocumentStore,
}

impl Backend {
    /// Create a new backend with an empty document store.
    pub fn new(client: Client) -> Self {
        Self { client, documents: DocumentStore::new() }
    }
}

/// Look up a sort action by index, build its options, and sort the content.
///
/// Returns `None` if the index is out of bounds or sorting fails.
pub fn resolve_sort_action(content: &str, action_index: usize) -> Option<String> {
    let action_def = crate::actions::ACTIONS.get(action_index)?;
    let options = (action_def.options)();
    json_sort::sort_json(content, &options).ok()
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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

    // Return all 9 sort actions as unresolved stubs; actual edits are computed lazily
    // in `code_action_resolve` when the user selects one.
    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        if self.documents.get(uri).is_none() {
            return Ok(None);
        }
        let actions = crate::actions::ACTIONS
            .iter()
            .enumerate()
            .map(|(i, def)| {
                CodeActionOrCommand::CodeAction(CodeAction {
                    title: def.title.to_string(),
                    kind: Some(CodeActionKind::REFACTOR_REWRITE),
                    data: Some(serde_json::json!({
                        "action_index": i,
                        "uri": uri.to_string(),
                    })),
                    ..Default::default()
                })
            })
            .collect();
        Ok(Some(actions))
    }

    // Resolve the chosen action: sort the document and produce a full-document TextEdit.
    async fn code_action_resolve(&self, mut action: CodeAction) -> Result<CodeAction> {
        let Some(data) = &action.data else { return Ok(action) };
        let action_index = usize::try_from(data["action_index"].as_u64().unwrap_or(0)).unwrap_or(0);
        let uri_str = data["uri"].as_str().unwrap_or_default();
        let Ok(uri) = Url::parse(uri_str) else { return Ok(action) };
        let Some(content) = self.documents.get(&uri) else { return Ok(action) };
        let Some(sorted) = resolve_sort_action(&content, action_index) else {
            return Ok(action);
        };

        let full_range =
            Range { start: Position { line: 0, character: 0 }, end: Position { line: u32::MAX, character: u32::MAX } };
        let mut changes = HashMap::new();
        changes.insert(uri, vec![TextEdit { range: full_range, new_text: sorted }]);
        action.edit = Some(WorkspaceEdit { changes: Some(changes), ..Default::default() });
        Ok(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_ascending_sort() {
        let content = r#"{"c": 3, "a": 1, "b": 2}"#;
        let result = resolve_sort_action(content, 0).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn resolve_descending_sort() {
        let content = r#"{"a": 1, "c": 3, "b": 2}"#;
        let result = resolve_sort_action(content, 1).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["c", "b", "a"]);
    }

    #[test]
    fn resolve_sort_list_items() {
        let content = r"[3, 1, 2]";
        let result = resolve_sort_action(content, 7).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn resolve_sort_both() {
        let content = r#"{"b": [3, 1], "a": 1}"#;
        let result = resolve_sort_action(content, 8).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(parsed["b"], serde_json::json!([1, 3]));
    }

    #[test]
    fn resolve_invalid_index_returns_none() {
        let result = resolve_sort_action("{}", 99);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_invalid_json_returns_none() {
        let result = resolve_sort_action("not json", 0);
        assert!(result.is_none());
    }
}
