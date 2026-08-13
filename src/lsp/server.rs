//! The stdio language server: handshake, main loop, and message dispatch.

use super::documents::Documents;
use super::workspace::Workspace;
use super::{builtin, diagnostics, objects};
use crate::binary::command::ExitStatus;
use anyhow::Result;
use lsp_server::{Connection, Message, Notification};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::{
    InitializeParams, OneOf, PublishDiagnosticsParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// How long the loop waits for a message before checking whether it has been
/// asked to quit.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) fn run_stdio(quit: Arc<AtomicBool>) -> Result<ExitStatus> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;
    let _params: InitializeParams = serde_json::from_value(params)?;
    connection.initialize_finish(
        id,
        serde_json::json!({
            "capabilities": capabilities(),
            "serverInfo": { "name": "archival", "version": env!("CARGO_PKG_VERSION") },
        }),
    )?;
    let mut state = Server::default();
    let result = state.main_loop(&connection, quit);
    // The writer thread runs until every sender is gone, so the connection has
    // to go before the join.
    drop(connection);
    io_threads.join()?;
    result
}

fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        definition_provider: Some(OneOf::Left(false)),
        ..Default::default()
    }
}

#[derive(Default)]
pub(crate) struct Server {
    documents: Documents,
    workspace: Workspace,
}

impl Server {
    /// Takes a [`Connection`] rather than owning one so that tests can drive it
    /// over [`Connection::memory`].
    pub fn main_loop(
        &mut self,
        connection: &Connection,
        quit: Arc<AtomicBool>,
    ) -> Result<ExitStatus> {
        loop {
            match connection.receiver.recv_timeout(POLL_INTERVAL) {
                Ok(Message::Request(request)) => {
                    if connection.handle_shutdown(&request)? {
                        return Ok(ExitStatus::Ok);
                    }
                    debug!("unhandled request {}", request.method);
                }
                Ok(Message::Notification(notification)) => {
                    if notification.method == lsp_types::notification::Exit::METHOD {
                        return Ok(ExitStatus::Ok);
                    }
                    self.notified(connection, notification);
                }
                Ok(Message::Response(_)) => {}
                Err(err) if err.is_disconnected() => return Ok(ExitStatus::Ok),
                Err(_) => {}
            }
            if quit.load(Ordering::SeqCst) {
                return Ok(ExitStatus::Ok);
            }
        }
    }

    fn notified(&mut self, connection: &Connection, notification: Notification) {
        match &notification.method[..] {
            DidOpenTextDocument::METHOD => {
                let Some(params) = extract::<DidOpenTextDocument>(notification) else {
                    return;
                };
                let document = params.text_document;
                self.documents.open(document.uri.clone(), document.text);
                self.publish(connection, &document.uri);
            }
            DidChangeTextDocument::METHOD => {
                let Some(params) = extract::<DidChangeTextDocument>(notification) else {
                    return;
                };
                let uri = params.text_document.uri;
                self.documents.change(&uri, params.content_changes);
                self.publish(connection, &uri);
            }
            DidCloseTextDocument::METHOD => {
                let Some(params) = extract::<DidCloseTextDocument>(notification) else {
                    return;
                };
                let uri = params.text_document.uri;
                self.documents.close(&uri);
                // Diagnostics for a closed document are the client's to forget
                // only once it is told they are gone.
                self.send(connection, &uri, vec![]);
            }
            DidChangeWatchedFiles::METHOD => {
                let Some(params) = extract::<DidChangeWatchedFiles>(notification) else {
                    return;
                };
                let touched_definitions = params.changes.iter().any(|change| {
                    change
                        .uri
                        .to_file_path()
                        .is_ok_and(|path| self.workspace.is_definition_file(&path))
                });
                if touched_definitions {
                    // Definitions are read once when a site loads, so a change
                    // to them is a reload rather than an invalidation.
                    self.workspace.reload();
                    self.republish(connection);
                }
            }
            method => debug!("unhandled notification {method}"),
        }
    }

    fn republish(&mut self, connection: &Connection) {
        for uri in self.documents.uris() {
            self.publish(connection, &uri);
        }
    }

    fn publish(&mut self, connection: &Connection, uri: &Url) {
        let Some(document) = self.documents.get(uri) else {
            return;
        };
        let found = match uri.to_file_path() {
            Ok(path) if path.extension().is_some_and(|ext| ext == "toml") => {
                self.toml_errors(&path)
            }
            _ => diagnostics::parse_errors(document),
        };
        self.send(connection, uri, found);
    }

    /// Diagnostics for a toml file in a site: its manifest, its object
    /// definitions, or one object. A toml that is none of those — or that lives
    /// outside a site — has nothing to report.
    ///
    /// Every check runs against the parsers compiled into this binary, so what
    /// it reports is what the archival building the site will accept.
    fn toml_errors(&mut self, path: &Path) -> Vec<lsp_types::Diagnostic> {
        let Some(document) = Url::from_file_path(path)
            .ok()
            .and_then(|uri| self.documents.get(&uri))
        else {
            return vec![];
        };
        let Some(site) = self.workspace.site_for(path) else {
            return vec![];
        };
        if site.is_manifest(path) {
            return builtin::validate_manifest(document, &site.root);
        }
        if site.is_definitions(path) {
            return builtin::validate_definitions(document, site.manifest());
        }
        let Some(definition) = site.definition_for(path) else {
            return vec![];
        };
        let custom_types = &site
            .site
            .as_ref()
            .expect("a definition implies a loaded site")
            .manifest
            .editor_types;
        objects::validate(document, path, definition, custom_types)
    }

    fn send(&self, connection: &Connection, uri: &Url, diagnostics: Vec<lsp_types::Diagnostic>) {
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: None,
        };
        let notification = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
        if connection.sender.send(notification.into()).is_err() {
            warn!("client disconnected while publishing diagnostics");
        }
    }
}

fn extract<N: lsp_types::notification::Notification>(
    notification: Notification,
) -> Option<N::Params> {
    match serde_json::from_value(notification.params) {
        Ok(params) => Some(params),
        Err(err) => {
            warn!("malformed {} notification: {err}", N::METHOD);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{
        DidOpenTextDocumentParams, TextDocumentIdentifier, TextDocumentItem,
        VersionedTextDocumentIdentifier,
    };

    /// Drives a server over an in-memory connection, returning every message it
    /// sent back.
    fn exchange(messages: Vec<Message>) -> Vec<Message> {
        let (connection, client) = Connection::memory();
        let worker = std::thread::spawn(move || {
            Server::default().main_loop(&connection, Arc::new(AtomicBool::new(false)))
        });
        for message in messages {
            client.sender.send(message).unwrap();
        }
        client
            .sender
            .send(Message::Notification(Notification::new(
                lsp_types::notification::Exit::METHOD.to_string(),
                serde_json::Value::Null,
            )))
            .unwrap();
        worker.join().unwrap().unwrap();
        client.receiver.into_iter().collect()
    }

    fn did_open(uri: &str, text: &str) -> Message {
        Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Url::parse(uri).unwrap(),
                    language_id: "liquid".into(),
                    version: 1,
                    text: text.into(),
                },
            },
        ))
    }

    fn diagnostics_in(message: &Message) -> PublishDiagnosticsParams {
        let Message::Notification(notification) = message else {
            panic!("expected a notification, got {message:?}");
        };
        assert_eq!(notification.method, PublishDiagnostics::METHOD);
        serde_json::from_value(notification.params.clone()).unwrap()
    }

    #[test]
    fn opening_a_document_publishes_its_diagnostics() {
        let sent = exchange(vec![did_open("file:///a.liquid", "{% if x %}unclosed")]);
        assert_eq!(sent.len(), 1);
        let published = diagnostics_in(&sent[0]);
        assert_eq!(published.uri.as_str(), "file:///a.liquid");
        assert_eq!(published.diagnostics.len(), 1);
    }

    #[test]
    fn editing_a_document_republishes() {
        let change = Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            lsp_types::DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Url::parse("file:///a.liquid").unwrap(),
                    version: 2,
                },
                content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "{{ fixed }}".into(),
                }],
            },
        ));
        let sent = exchange(vec![did_open("file:///a.liquid", "{% if x %}"), change]);
        assert_eq!(sent.len(), 2);
        assert_eq!(diagnostics_in(&sent[0]).diagnostics.len(), 1);
        assert!(diagnostics_in(&sent[1]).diagnostics.is_empty());
    }

    /// Closing a document clears what was published for it.
    #[test]
    fn closing_a_document_clears_its_diagnostics() {
        let close = Message::Notification(Notification::new(
            DidCloseTextDocument::METHOD.to_string(),
            lsp_types::DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::parse("file:///a.liquid").unwrap(),
                },
            },
        ));
        let sent = exchange(vec![did_open("file:///a.liquid", "{% if x %}"), close]);
        assert_eq!(sent.len(), 2);
        assert!(diagnostics_in(&sent[1]).diagnostics.is_empty());
    }

    /// A `.toml` under the objects dir is checked against its definition, which
    /// means resolving its site from the path alone.
    #[test]
    fn validates_object_files_against_their_definition() {
        let object = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/website/objects/section/one.toml");
        let uri = Url::from_file_path(&object).unwrap();
        let sent = exchange(vec![Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "toml".into(),
                    version: 1,
                    text: "name = \"a\"\nbody = \"b\"\nnope = 1\n".into(),
                },
            },
        ))]);
        let published = diagnostics_in(&sent[0]);
        assert_eq!(published.diagnostics.len(), 1, "{published:#?}");
        assert!(published.diagnostics[0]
            .message
            .starts_with("`nope` is not defined on `section`"));
        assert_eq!(published.diagnostics[0].range.start.line, 2);
    }

    /// A toml outside any objects dir is not an object, so it has nothing to be
    /// checked against.
    #[test]
    fn leaves_other_toml_files_alone() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/website/archival_objects.toml");
        let sent = exchange(vec![Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Url::from_file_path(&manifest).unwrap(),
                    language_id: "toml".into(),
                    version: 1,
                    text: "[whatever]\nname = \"string\"\n".into(),
                },
            },
        ))]);
        assert!(diagnostics_in(&sent[0]).diagnostics.is_empty());
    }

    #[test]
    fn a_malformed_notification_does_not_stop_the_server() {
        let garbage = Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            serde_json::json!({ "nonsense": true }),
        ));
        let sent = exchange(vec![garbage, did_open("file:///a.liquid", "{{ ok }}")]);
        assert_eq!(sent.len(), 1);
        assert!(diagnostics_in(&sent[0]).diagnostics.is_empty());
    }
}
