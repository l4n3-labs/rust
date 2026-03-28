//! LSP server for sorting JSON and JSONC files.
//!
//! Communicates over stdin/stdout using the JSON-RPC protocol. Provides up to 27
//! code actions (refactor.rewrite) that sort the active document using different
//! strategies from the [`json_sort`] library, organised into three scopes:
//!
//! - **Deep Sort** (9 actions) — sorts the entire document recursively.
//! - **Shallow Sort** (9 actions) — sorts only the top-level keys of the root object.
//! - **Subtree Sort** (9 actions, contextual) — sorts the object or array under the
//!   cursor and all of its descendants. Only offered when the cursor is inside a
//!   nested (non-root) container.

mod actions;
mod backend;
mod cursor;
mod documents;
mod settings;

use backend::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
