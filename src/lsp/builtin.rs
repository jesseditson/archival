//! Diagnostics for archival's own configuration files.
//!
//! These are checked by handing them to the same parsers a build uses, rather
//! than to the JSON Schemas published for editors. The schemas describe a
//! version of archival; the parsers compiled into this binary *are* the version
//! the site will be built with, and they reject things a schema cannot express
//! — an unrecognized field type, a validator that is not a regular expression, a
//! reserved name used as an object.

use super::documents::Document;
use super::toml_diag::{error_at_named_key, syntax_error};
use crate::manifest::Manifest;
use crate::object_definition::ObjectDefinition;
use lsp_types::{Diagnostic, DiagnosticSeverity};
use std::path::Path;

/// Checks `archival.toml`.
pub(super) fn validate_manifest(doc: &Document, root: &Path) -> Vec<Diagnostic> {
    if let Err(err) = toml::from_str::<toml::Table>(&doc.text) {
        return vec![syntax_error(doc, &err)];
    }
    // The upload prefix is supplied by whoever is building, not by the file.
    match Manifest::from_string(root, doc.text.clone(), Some("")) {
        Ok(_) => vec![],
        Err(err) => vec![error_at_named_key(
            doc,
            err.to_string(),
            DiagnosticSeverity::ERROR,
        )],
    }
}

/// Checks `archival_objects.toml`, or whatever `object_file` names.
///
/// Custom types come from the manifest, so a definition using one is only
/// understood when the manifest alongside it loaded.
pub(super) fn validate_definitions(doc: &Document, manifest: Option<&Manifest>) -> Vec<Diagnostic> {
    if let Err(err) = toml::from_str::<toml::Table>(&doc.text) {
        return vec![syntax_error(doc, &err)];
    }
    let editor_types = manifest.map(|m| m.editor_types.clone()).unwrap_or_default();
    match ObjectDefinition::from_source(&doc.text, &editor_types) {
        Ok(_) => vec![],
        Err(err) => vec![error_at_named_key(
            doc,
            err.to_string(),
            DiagnosticSeverity::ERROR,
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(text: &str) -> Vec<Diagnostic> {
        validate_manifest(&Document::new(text.to_string()), Path::new("/site"))
    }

    fn definitions(text: &str) -> Vec<Diagnostic> {
        validate_definitions(&Document::new(text.to_string()), None)
    }

    #[test]
    fn accepts_a_valid_manifest() {
        assert!(manifest("site_name = \"a\"\npages = \"pages\"\n").is_empty());
    }

    #[test]
    fn reports_a_manifest_field_of_the_wrong_type() {
        let found = manifest("pages = 42\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn reports_manifest_syntax_errors() {
        let found = manifest("site_name = \n");
        assert_eq!(found.len(), 1, "{found:#?}");
    }

    #[test]
    fn accepts_valid_definitions() {
        assert!(definitions("[post]\ntitle = \"string\"\nbody = \"markdown\"\n").is_empty());
    }

    /// A schema cannot tell a real type from any other string, because a custom
    /// editor type is also just a string. The parser can.
    #[test]
    fn reports_an_unrecognized_field_type() {
        let found = definitions("[post]\ntitle = \"strng\"\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert!(found[0].message.contains("strng"), "{:?}", found[0].message);
    }

    #[test]
    fn reports_a_reserved_object_name() {
        let found = definitions("[objects]\ntitle = \"string\"\n");
        assert_eq!(found.len(), 1, "{found:#?}");
    }

    /// A custom type is only recognized when the manifest defining it loaded.
    #[test]
    fn resolves_custom_types_from_the_manifest() {
        let source = "[post]\nthumb = \"hero\"\n";
        assert_eq!(definitions(source).len(), 1);

        let manifest = Manifest::from_string(
            Path::new("/site"),
            "[editor_types.hero]\ntype = \"image\"\n".to_string(),
            Some(""),
        )
        .unwrap();
        let doc = Document::new(source.to_string());
        assert!(validate_definitions(&doc, Some(&manifest)).is_empty());
    }
}
