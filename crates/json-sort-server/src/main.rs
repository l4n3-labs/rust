//! LSP server for sorting JSON and JSONC files.
//!
//! Communicates over stdin/stdout using the JSON-RPC protocol. Provides 9 code
//! actions (refactor.rewrite) that sort the active document using different strategies
//! from the [`json_sort`] library.

mod actions;
mod backend;
mod documents;

use backend::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
