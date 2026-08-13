//! A language server for archival's liquid templates, spoken over stdio by
//! `archival lsp`.

mod builtin;
mod diagnostics;
mod documents;
mod objects;
mod server;
mod toml_diag;
mod workspace;

pub(crate) use server::run_stdio;
