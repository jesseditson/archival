//! Diagnostics for an object file, checked against the definition it is an
//! instance of.

use super::documents::Document;
use super::toml_diag::{child_tables, diagnostic, error_at_key, error_at_named_key, syntax_error};
use crate::fields::FieldType;
use crate::manifest::EditorTypes;
use crate::object::Object;
use crate::object_definition::ObjectDefinition;
use crate::reserved_fields::is_reserved_field;
use lsp_types::{Diagnostic, DiagnosticSeverity};
use std::path::Path;
use toml_edit::Table;

/// Checks `doc` against `definition`, reporting whatever would stop archival
/// reading it.
pub(crate) fn validate(
    doc: &Document,
    path: &Path,
    definition: &ObjectDefinition,
    custom_types: &EditorTypes,
) -> Vec<Diagnostic> {
    let table: toml::Table = match toml::from_str(&doc.text) {
        Ok(table) => table,
        Err(err) => return vec![syntax_error(doc, &err)],
    };

    let mut found = unknown_fields(doc, definition, custom_types);
    found.extend(bad_order(doc, &table));
    // Archival stops at the first bad value, so this adds at most one more.
    if let Err(err) = Object::from_table(definition, path, &table, custom_types, false) {
        found.push(error_at_named_key(
            doc,
            err.to_string(),
            DiagnosticSeverity::ERROR,
        ));
    }
    found
}

/// `order` sorts objects within their directory, and archival ignores it when
/// it is not a number, leaving the object in filename order instead.
fn bad_order(doc: &Document, table: &toml::Table) -> Option<Diagnostic> {
    let value = table.get(crate::reserved_fields::ORDER)?;
    if value.is_integer() || value.is_float() {
        return None;
    }
    Some(error_at_key(
        doc,
        crate::reserved_fields::ORDER,
        format!("`order` must be a number, not {}.", value.type_str()),
        DiagnosticSeverity::WARNING,
    ))
}

/// Keys that are neither a field nor a child of the definition. Archival only
/// logs these, so they would otherwise be silent: the value is dropped and the
/// template renders nothing.
fn unknown_fields(
    doc: &Document,
    definition: &ObjectDefinition,
    custom_types: &EditorTypes,
) -> Vec<Diagnostic> {
    let Ok(parsed) = toml_edit::Document::<&str>::parse(&doc.text) else {
        return vec![];
    };
    let mut found = vec![];
    walk(doc, parsed.as_table(), definition, custom_types, &mut found);
    found
}

fn walk(
    doc: &Document,
    table: &Table,
    definition: &ObjectDefinition,
    custom_types: &EditorTypes,
    found: &mut Vec<Diagnostic>,
) {
    for (name, item) in table.iter() {
        let Some((key, _)) = table.get_key_value(name) else {
            continue;
        };
        let resolved = custom_types
            .get(name)
            .map(|alias| &alias.alias_of[..])
            .unwrap_or(name);
        if let Some(child) = definition.children.get(resolved) {
            // Children are arrays of tables; a mismatch there is reported by
            // archival's own parse rather than here.
            for entry in child_tables(item) {
                walk(doc, entry, child, custom_types, found);
            }
            continue;
        }
        if definition.field_type(resolved).is_some() || is_reserved_field(resolved) {
            continue;
        }
        let Some(span) = key.span() else { continue };
        found.push(diagnostic(
            doc,
            span,
            format!(
                "`{name}` is not defined on `{}`. Known fields: {}.",
                definition.name,
                known_fields(definition)
            ),
            DiagnosticSeverity::WARNING,
        ));
    }
}

fn known_fields(definition: &ObjectDefinition) -> String {
    let mut names: Vec<&str> = definition
        .fields
        .iter()
        .filter(|(_, field)| !matches!(field.r#type, FieldType::Secret))
        .map(|(name, _)| &name[..])
        .chain(definition.children.keys().map(|name| &name[..]))
        .collect();
    if names.is_empty() {
        names.push("(none)");
    }
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_definition::ObjectDefinition;
    use ordermap::OrderMap;

    fn definitions(source: &str) -> crate::object_definition::ObjectDefinitions {
        ObjectDefinition::from_source(source, &OrderMap::new()).unwrap()
    }

    fn check(defs: &str, object: &str) -> Vec<Diagnostic> {
        let definitions = definitions(defs);
        let definition = definitions.values().next().unwrap();
        let doc = Document::new(object.to_string());
        validate(
            &doc,
            Path::new("objects/post/one.toml"),
            definition,
            &OrderMap::new(),
        )
    }

    const POST: &str = r#"
[post]
title = "string"
count = "number"
published = "boolean"
kind = ["a", "b"]

[post.tags]
label = "string"
"#;

    #[test]
    fn a_valid_object_has_no_diagnostics() {
        let found = check(
            POST,
            "title = \"hi\"\ncount = 2\npublished = true\nkind = \"a\"\n\n[[tags]]\nlabel = \"x\"\n",
        );
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn reports_unknown_fields() {
        let found = check(POST, "title = \"hi\"\ntitel = \"typo\"\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].severity, Some(DiagnosticSeverity::WARNING));
        assert!(found[0]
            .message
            .starts_with("`titel` is not defined on `post`"));
        // Points at the key itself, on its own line.
        assert_eq!(found[0].range.start.line, 1);
        assert_eq!(found[0].range.start.character, 0);
    }

    #[test]
    fn reports_unknown_fields_in_children() {
        let found = check(POST, "[[tags]]\nlabel = \"x\"\nnope = 1\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert!(found[0]
            .message
            .starts_with("`nope` is not defined on `tags`"));
        assert_eq!(found[0].range.start.line, 2);
    }

    #[test]
    fn reports_type_mismatches() {
        let found = check(POST, "count = \"not a number\"\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(found[0].range.start.line, 0);
    }

    #[test]
    fn reports_values_outside_an_enum() {
        let found = check(POST, "kind = \"c\"\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn reports_a_toml_syntax_error() {
        let found = check(POST, "title = \n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    /// `order` is set by the editor rather than declared, so it is not an
    /// unknown field.
    #[test]
    fn allows_reserved_keys() {
        assert!(check(POST, "order = 3\ntitle = \"hi\"\n").is_empty());
        assert!(check(POST, "order = 1.5\n").is_empty());
    }

    #[test]
    fn reports_an_order_that_is_not_a_number() {
        let found = check(POST, "title = \"hi\"\norder = \"3\"\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].message, "`order` must be a number, not string.");
        assert_eq!(found[0].range.start.line, 1);
    }
}
