use std::collections::HashMap;
use std::sync::RwLock;

use tower_lsp::lsp_types::Url;

/// Thread-safe in-memory store for open document contents, keyed by URI.
pub struct DocumentStore {
    documents: RwLock<HashMap<Url, String>>,
}

impl DocumentStore {
    /// Create an empty document store.
    pub fn new() -> Self {
        Self { documents: RwLock::new(HashMap::new()) }
    }

    /// Track a newly opened document.
    pub fn open(&self, uri: Url, content: String) {
        self.documents.write().unwrap().insert(uri, content);
    }

    /// Replace the content of an already-open document.
    pub fn update(&self, uri: &Url, content: String) {
        self.documents.write().unwrap().insert(uri.clone(), content);
    }

    /// Remove a document from the store when it is closed.
    pub fn close(&self, uri: &Url) {
        self.documents.write().unwrap().remove(uri);
    }

    /// Retrieve a clone of the document content, or `None` if not tracked.
    pub fn get(&self, uri: &Url) -> Option<String> {
        self.documents.read().unwrap().get(uri).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_uri(name: &str) -> Url {
        Url::parse(&format!("file:///tmp/{name}.json")).unwrap()
    }

    #[test]
    fn open_and_get() {
        let store = DocumentStore::new();
        let uri = test_uri("a");
        store.open(uri.clone(), r#"{"a": 1}"#.to_string());
        assert_eq!(store.get(&uri), Some(r#"{"a": 1}"#.to_string()));
    }

    #[test]
    fn update_replaces_content() {
        let store = DocumentStore::new();
        let uri = test_uri("b");
        store.open(uri.clone(), "old".to_string());
        store.update(&uri, "new".to_string());
        assert_eq!(store.get(&uri), Some("new".to_string()));
    }

    #[test]
    fn close_removes_document() {
        let store = DocumentStore::new();
        let uri = test_uri("c");
        store.open(uri.clone(), "content".to_string());
        store.close(&uri);
        assert_eq!(store.get(&uri), None);
    }

    #[test]
    fn get_missing_returns_none() {
        let store = DocumentStore::new();
        let uri = test_uri("nonexistent");
        assert_eq!(store.get(&uri), None);
    }
}
