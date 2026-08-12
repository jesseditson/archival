//! Diagnostics for a liquid template.

use super::documents::{Document, LineIndex};
use crate::liquid_parser::{self, ArchivalPartialSource};
use crate::liquid_rewrite::rewrite_template_mapped;
use lsp_types::{Diagnostic, DiagnosticSeverity};
use once_cell::sync::Lazy;
use regex::Regex;

/// `liquid_core::Error` carries no structured position, but every parse failure
/// is a stringified pest error, which quotes one as `--> line:column`.
static ERROR_POSITION: Lazy<Regex> = Lazy::new(|| Regex::new(r"--> (\d+):(\d+)").unwrap());

/// Parses `doc` and reports whatever the liquid parser rejects.
///
/// Partials are left out of the parser: resolving one happens at render time,
/// so an unknown partial is not a syntax error and must not be reported here.
pub(crate) fn parse_errors(doc: &Document) -> Vec<Diagnostic> {
    let Ok(parser) = liquid_parser::build_with_partials(ArchivalPartialSource::default()) else {
        return vec![];
    };
    let rewritten = rewrite_template_mapped(&doc.text);
    let Err(err) = parser.parse(&rewritten.text) else {
        return vec![];
    };
    let raw = err.to_string();
    // Without a position the whole document is the best available range.
    let span = match error_offset(&raw, &rewritten.text) {
        Some(at) => {
            let at = rewritten.anchors.to_source(at);
            at..at
        }
        None => 0..doc.text.len(),
    };
    vec![Diagnostic {
        range: doc.range(span),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("archival".into()),
        message: summarize(&raw),
        ..Default::default()
    }]
}

/// The human-readable part of a parse failure.
///
/// A pest error also carries a quoted excerpt and a `line:column`, both of
/// which describe the rewritten text rather than the file, and would contradict
/// the range this diagnostic is reported at.
fn summarize(message: &str) -> String {
    message
        .lines()
        .filter_map(|line| line.trim().strip_prefix("= "))
        .next_back()
        .unwrap_or(message)
        .to_string()
}

/// The byte offset in `rewritten` that a parse error points at. Pest counts
/// lines and columns from one, and columns in characters.
fn error_offset(message: &str, rewritten: &str) -> Option<usize> {
    let found = ERROR_POSITION.captures(message)?;
    let line: usize = found.get(1)?.as_str().parse().ok()?;
    let column: usize = found.get(2)?.as_str().parse().ok()?;
    let line_start = LineIndex::new(rewritten).line_start(line.checked_sub(1)?)?;
    let rest = rewritten.get(line_start..)?;
    Some(
        line_start
            + rest
                .char_indices()
                .nth(column.checked_sub(1)?)
                .map_or(rest.len(), |(at, _)| at),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnose(text: &str) -> Vec<Diagnostic> {
        parse_errors(&Document::new(text.to_string()))
    }

    #[test]
    fn a_valid_template_has_no_diagnostics() {
        for template in [
            "{{ x }}{% if y %}a{% endif %}",
            "{% include 'nonexistent' %}",
            "{% layout 'theme' title: 'x' %}",
            "{% liquid\n  assign a = 1\n  echo a\n%}",
            "{% # a comment %}",
            "plain text",
        ] {
            assert!(diagnose(template).is_empty(), "{template:?} was rejected");
        }
    }

    #[test]
    fn reports_a_syntax_error() {
        let found = diagnose("{% if x %}unclosed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    /// The excerpt a pest error draws quotes the rewritten text, so only its
    /// description survives.
    #[test]
    fn reports_only_the_description() {
        let found = diagnose("{% for %}");
        assert_eq!(found[0].message, "Identifier expected.");
        assert_eq!(summarize("no pest error here"), "no pest error here");
    }

    /// A `{% liquid %}` body collapses to one line when rewritten, so a later
    /// error resolves to the line it was written on rather than to the
    /// rewritten one.
    #[test]
    fn positions_survive_the_rewrite() {
        let found = diagnose("{% liquid\n  assign a = 1\n  assign b = 2\n%}\n{% for %}");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].range.start.line, 4);
    }
}
