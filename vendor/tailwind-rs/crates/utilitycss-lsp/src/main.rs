//! Stdio entry point for the utilitycss language server.

#![forbid(unsafe_code)]

use tokio::io::{stdin, stdout};
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let (service, socket) = LspService::new(utilitycss_lsp::Backend::new);
    Server::new(stdin(), stdout(), socket).serve(service).await;
}
