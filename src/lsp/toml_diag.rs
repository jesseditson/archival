//! Turning a toml parse or validation failure into a diagnostic that points at
//! the key it is about.

use super::documents::Document;
use lsp_types::{Diagnostic, DiagnosticSeverity};
use std::ops::Range;
use toml_edit::{Item, Table};

pub(super) fn diagnostic(
    doc: &Document,
    span: Range<usize>,
    message: String,
    severity: DiagnosticSeverity,
) -> Diagnostic {
    Diagnostic {
        range: doc.range(span),
        severity: Some(severity),
        source: Some("archival".into()),
        message,
        ..Default::default()
    }
}

/// A diagnostic for text that is not toml at all. `toml` carries a span for
/// these, so no searching is needed.
pub(super) fn syntax_error(doc: &Document, err: &toml::de::Error) -> Diagnostic {
    let span = err.span().unwrap_or(0..doc.text.len());
    diagnostic(
        doc,
        span,
        err.message().to_string(),
        DiagnosticSeverity::ERROR,
    )
}

/// An error about `source` as a whole, narrowed to a key when the message names
/// one. Archival's errors quote the offending key as `"name"`, and carry no
/// span of their own.
pub(super) fn error_at_named_key(
    doc: &Document,
    message: String,
    severity: DiagnosticSeverity,
) -> Diagnostic {
    let named = message.split('"').nth(1).unwrap_or_default().to_string();
    error_at_key(doc, &named, message, severity)
}

/// An error about `key`, falling back to the whole document when it cannot be
/// found — a key named in an error does not always appear in the source, since
/// some come from a value rather than from what it was written under.
pub(super) fn error_at_key(
    doc: &Document,
    key: &str,
    message: String,
    severity: DiagnosticSeverity,
) -> Diagnostic {
    let span = span_of_key(&doc.text, key).unwrap_or(0..doc.text.len());
    diagnostic(doc, span, message, severity)
}

fn span_of_key(source: &str, key: &str) -> Option<Range<usize>> {
    if key.is_empty() {
        return None;
    }
    let parsed = toml_edit::Document::<&str>::parse(source).ok()?;
    find_key(parsed.as_table(), key)
}

fn find_key(table: &Table, name: &str) -> Option<Range<usize>> {
    for (key_name, item) in table.iter() {
        let (key, _) = table.get_key_value(key_name)?;
        if key_name == name {
            return key.span();
        }
        let nested = child_tables(item)
            .into_iter()
            .find_map(|child| find_key(child, name));
        if nested.is_some() {
            return nested;
        }
    }
    None
}

pub(super) fn child_tables(item: &Item) -> Vec<&Table> {
    match item {
        Item::ArrayOfTables(tables) => tables.iter().collect(),
        Item::Table(table) => vec![table],
        _ => vec![],
    }
}
