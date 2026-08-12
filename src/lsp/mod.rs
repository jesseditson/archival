//! A language server for archival's liquid templates, spoken over stdio by
//! `archival lsp`.

mod diagnostics;
mod documents;
mod server;

pub(crate) use server::run_stdio;
