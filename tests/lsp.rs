//! The language server speaks JSON-RPC over stdout, so nothing else may be
//! written there. Only a real subprocess can prove that; the in-process tests
//! in `src/lsp/server.rs` never touch stdout.

#![cfg(feature = "lsp")]

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

fn frame(message: Value) -> String {
    let body = message.to_string();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

/// Reads every JSON-RPC message off `stdout`, failing if anything that is not a
/// framed message appears between them.
fn messages(mut stdout: &str) -> Vec<Value> {
    let mut found = vec![];
    while !stdout.is_empty() {
        let (header, rest) = stdout
            .split_once("\r\n\r\n")
            .unwrap_or_else(|| panic!("expected a message header, got {stdout:?}"));
        let length: usize = header
            .strip_prefix("Content-Length: ")
            .unwrap_or_else(|| panic!("expected a Content-Length, got {header:?}"))
            .trim()
            .parse()
            .expect("Content-Length was not a number");
        found.push(serde_json::from_str(&rest[..length]).expect("message was not valid json"));
        stdout = &rest[length..];
    }
    found
}

#[test]
fn logging_does_not_corrupt_the_protocol() {
    let session = [
        frame(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                     "params": {"capabilities": {}, "processId": null}})),
        frame(json!({"jsonrpc": "2.0", "method": "initialized", "params": {}})),
        frame(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen",
                     "params": {"textDocument": {"uri": "file:///a.liquid", "languageId": "liquid",
                                                 "version": 1, "text": "{% for %}"}}})),
        frame(json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null})),
        frame(json!({"jsonrpc": "2.0", "method": "exit", "params": null})),
    ]
    .concat();

    let mut server = Command::new(env!("CARGO_BIN_EXE_archival"))
        .arg("lsp")
        .env("RUST_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn archival lsp");
    server
        .stdin
        .take()
        .unwrap()
        .write_all(session.as_bytes())
        .unwrap();

    let mut stdout = String::new();
    server
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    let mut stderr = String::new();
    server
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(server.wait().unwrap().success(), "server exited badly");

    // Trace logging was on, so there is something to have misdirected.
    assert!(!stderr.is_empty(), "expected logs on stderr");

    let found = messages(&stdout);
    let diagnostics: Vec<&Value> = found
        .iter()
        .filter(|m| m["method"] == "textDocument/publishDiagnostics")
        .collect();
    assert_eq!(diagnostics.len(), 1, "got {found:#?}");
    assert_eq!(
        diagnostics[0]["params"]["diagnostics"][0]["message"],
        json!("Identifier expected.")
    );
}
