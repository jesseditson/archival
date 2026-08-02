//! Recovers the comments written above fields and objects in
//! `archival_objects.toml`.
//!
//! `toml`'s value model discards comments, so definitions are parsed a second
//! time with `toml_edit` — which keeps them as key and table "decor" — and the
//! results are joined back onto the definitions in
//! [`ObjectDefinition::from_source`](crate::ObjectDefinition::from_source).
//!
//! Only the comment block *directly above* an item is taken, and a blank line
//! breaks the association:
//!
//! ```toml
//! # Not a description of `name` - there is a blank line below.
//!
//! # The artist's display name.
//! # Shown in listings.
//! name = "string"
//! ```
//!
//! Trailing comments (`name = "string" # ...`) are deliberately ignored: they
//! land in the item's decor *suffix*, and we only ever read the prefix.

use std::collections::HashMap;
use std::sync::LazyLock;
use toml_edit::{Decor, Document, InlineTable, Item, Table, TomlError, Value};

/// Comments harvested from an `archival_objects.toml` source, shaped to mirror
/// [`ObjectDefinition`](crate::ObjectDefinition)'s split between `fields` and
/// `children` so it can be walked alongside one.
///
/// The value returned by [`extract_comments`] is the document itself: its
/// `children` are the top-level objects, and its `own`/`fields` are empty.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DefinitionComments {
    /// The comment above this definition's `[header]`.
    pub own: Option<String>,
    /// Comments above field keys, and above the first `[[object.field]]` of a
    /// oneof.
    pub fields: HashMap<String, String>,
    /// Comments for child object definitions (`[object.child]`).
    pub children: HashMap<String, DefinitionComments>,
}

impl DefinitionComments {
    pub fn is_empty(&self) -> bool {
        self.own.is_none() && self.fields.is_empty() && self.children.is_empty()
    }
    /// The comments for a child definition, or an empty set if the child had
    /// none. Lets callers recurse without an `Option` dance.
    pub fn child(&self, name: &str) -> &DefinitionComments {
        static EMPTY: LazyLock<DefinitionComments> = LazyLock::new(DefinitionComments::default);
        self.children.get(name).unwrap_or(&EMPTY)
    }
    pub fn field(&self, name: &str) -> Option<String> {
        self.fields.get(name).cloned()
    }
}

/// Parses `source` purely to recover its comments.
///
/// This is a second parse of a document that `toml` has generally already
/// accepted; callers treat a failure here as "no comments" rather than as a
/// parse error, since both crates track the same TOML spec version.
pub fn extract_comments(source: &str) -> Result<DefinitionComments, TomlError> {
    // A read-only `Document` keeps decor as spans into `source`. `DocumentMut`
    // would `despan()` the whole tree, allocating a String for every comment
    // and every run of whitespace in the file.
    let doc = Document::<&str>::parse(source)?;
    let mut root = DefinitionComments::default();
    walk(doc.as_table(), source, &mut root);
    Ok(root)
}

fn walk(table: &Table, src: &str, out: &mut DefinitionComments) {
    let names: Vec<&str> = table.iter().map(|(name, _)| name).collect();
    for name in names {
        let Some((key, item)) = table.get_key_value(name) else {
            continue;
        };
        match item {
            // `child = { ... }`. toml_edit calls this a value, but toml reads it
            // as a table, so a definition treats it as a child object exactly
            // like `[object.child]` - and the comment has to land there too.
            Item::Value(Value::InlineTable(inline)) => {
                insert_child(out, name, comment_of(key.leaf_decor(), src), |child| {
                    walk_inline(inline, src, child)
                });
            }
            // A scalar type, or an enum/oneof array.
            Item::Value(_) => {
                // For a dotted key (`a.b = "string"`) the comment is written
                // onto the last key, which is where the field ends up anyway.
                if let Some(comment) = comment_of(key.leaf_decor(), src) {
                    out.fields.insert(name.to_string(), comment);
                }
            }
            // `[[object.field]]` - a oneof. Each entry is a separate table with
            // its own header, so the field's comment is the first one's.
            Item::ArrayOfTables(tables) => {
                if let Some(comment) = tables
                    .iter()
                    .next()
                    .and_then(|t| comment_of(t.decor(), src))
                {
                    out.fields.insert(name.to_string(), comment);
                }
            }
            // `[object.child]` - a child object definition.
            Item::Table(child_table) => {
                // An implicit table never had a header of its own, so its decor
                // is empty rather than absent - don't read it.
                let own = if child_table.is_implicit() {
                    None
                } else {
                    comment_of(child_table.decor(), src)
                };
                insert_child(out, name, own, |child| walk(child_table, src, child));
            }
            Item::None => {}
        }
    }
}

/// Walks the inside of an inline table, whose members are `Value`s rather than
/// `Item`s. TOML 1.1 allows these to span lines, so they can carry comments.
fn walk_inline(table: &InlineTable, src: &str, out: &mut DefinitionComments) {
    let names: Vec<&str> = table.iter().map(|(name, _)| name).collect();
    for name in names {
        let Some((key, value)) = table.get_key_value(name) else {
            continue;
        };
        match value {
            Item::Value(Value::InlineTable(inner)) => {
                insert_child(out, name, comment_of(key.leaf_decor(), src), |child| {
                    walk_inline(inner, src, child)
                });
            }
            _ => {
                if let Some(comment) = comment_of(key.leaf_decor(), src) {
                    out.fields.insert(name.to_string(), comment);
                }
            }
        }
    }
}

/// Records a child definition, dropping it if neither it nor anything beneath it
/// was described.
fn insert_child(
    out: &mut DefinitionComments,
    name: &str,
    own: Option<String>,
    walk_into: impl FnOnce(&mut DefinitionComments),
) {
    let mut child = DefinitionComments {
        own,
        ..Default::default()
    };
    walk_into(&mut child);
    if !child.is_empty() {
        out.children.insert(name.to_string(), child);
    }
}

fn comment_of(decor: &Decor, src: &str) -> Option<String> {
    let prefix = decor.prefix().map_or("", |raw| {
        // Spanned when parsed via `Document`, already resolved via `DocumentMut`.
        raw.as_str()
            .or_else(|| raw.span().and_then(|span| src.get(span)))
            .unwrap_or("")
    });
    trailing_comment_block(prefix)
}

/// Takes the last contiguous run of `#` lines from a decor prefix.
///
/// A prefix holds everything since the previous item, so `# a\n\n# b\n` means
/// only `b` describes what follows.
fn trailing_comment_block(prefix: &str) -> Option<String> {
    // Anything after the final newline is the item's own indentation, on the
    // same line as the key or header. It is not a blank line, so drop it before
    // scanning or it would terminate the run immediately.
    let body = &prefix[..=prefix.rfind('\n')?];
    let mut lines: Vec<&str> = Vec::new();
    for line in body.lines().rev() {
        // A blank line, or anything that isn't a comment, ends the block.
        let Some(comment) = line.trim().strip_prefix('#') else {
            break;
        };
        lines.push(comment.trim());
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &str) -> DefinitionComments {
        extract_comments(src).expect("failed to parse")
    }

    #[test]
    fn takes_the_block_directly_above_a_field() {
        let comments = extract(
            r#"
[artists]
# The artist's display name.
# Shown in listings.
name = "string"
"#,
        );
        assert_eq!(
            comments.child("artists").field("name"),
            Some("The artist's display name.\nShown in listings.".to_string())
        );
    }

    #[test]
    fn a_blank_line_breaks_the_association() {
        let comments = extract(
            r#"
[artists]
# A note about the file, not about name.

name = "string"
bio = "markdown"
"#,
        );
        assert_eq!(comments.child("artists").field("name"), None);
        assert_eq!(comments.child("artists").field("bio"), None);
    }

    #[test]
    fn only_the_last_block_is_taken() {
        let comments = extract(
            r#"
[artists]
# An orphaned note.

# The real description.
name = "string"
"#,
        );
        assert_eq!(
            comments.child("artists").field("name"),
            Some("The real description.".to_string())
        );
    }

    #[test]
    fn trailing_comments_are_ignored() {
        let comments = extract(
            r#"
[artists]
name = "string" # not a description
bio = "markdown"
"#,
        );
        assert_eq!(comments.child("artists").field("name"), None);
        // ...and it must not leak onto the following field either.
        assert_eq!(comments.child("artists").field("bio"), None);
    }

    #[test]
    fn hashes_inside_string_values_are_not_comments() {
        let comments = extract(
            r#"
[artists]
template = "artists/# not a comment"
name = "string"
"#,
        );
        assert_eq!(comments.child("artists").field("name"), None);
    }

    #[test]
    fn describes_top_level_objects() {
        let comments = extract(
            r#"
# A musical act on the roster.
[artists]
name = "string"
"#,
        );
        assert_eq!(
            comments.child("artists").own,
            Some("A musical act on the roster.".to_string())
        );
    }

    #[test]
    fn describes_child_objects() {
        let comments = extract(
            r#"
[artists]
name = "string"
# Upcoming shows.
[artists.tour_dates]
# When the show starts.
date = "date"
"#,
        );
        let tour_dates = comments.child("artists").child("tour_dates");
        assert_eq!(tour_dates.own, Some("Upcoming shows.".to_string()));
        assert_eq!(
            tour_dates.field("date"),
            Some("When the show starts.".to_string())
        );
    }

    #[test]
    fn describes_oneofs_from_the_first_entry() {
        let comments = extract(
            r#"
[artists]
# Media attached to this artist.
[[artists.media]]
name = "video"
type = "video"
# Ignored - the field is described once.
[[artists.media]]
name = "photo"
type = "image"
"#,
        );
        assert_eq!(
            comments.child("artists").field("media"),
            Some("Media attached to this artist.".to_string())
        );
    }

    #[test]
    fn describes_enums() {
        let comments = extract(
            r#"
[artists]
# How the artist is filed.
genre = ["emo", "metal"]
"#,
        );
        assert_eq!(
            comments.child("artists").field("genre"),
            Some("How the artist is filed.".to_string())
        );
    }

    #[test]
    fn handles_tables_defined_out_of_order() {
        let comments = extract(
            r#"
# Shows.
[artists.tour_dates]
date = "date"

# The artist.
[artists]
name = "string"
"#,
        );
        let artists = comments.child("artists");
        assert_eq!(artists.own, Some("The artist.".to_string()));
        assert_eq!(artists.child("tour_dates").own, Some("Shows.".to_string()));
    }

    #[test]
    fn an_implicit_parent_has_no_description() {
        // `artists` never gets a header of its own here.
        let comments = extract(
            r#"
# Shows.
[artists.tour_dates]
date = "date"
"#,
        );
        assert_eq!(comments.child("artists").own, None);
        assert_eq!(
            comments.child("artists").child("tour_dates").own,
            Some("Shows.".to_string())
        );
    }

    #[test]
    fn treats_an_inline_table_as_a_child() {
        // toml_edit calls this a value; toml calls it a table. Definitions
        // follow toml, so the comment has to land in `children`.
        let comments = extract(
            r#"
[artists]
# Upcoming shows.
tour_dates = { date = "date" }
"#,
        );
        let artists = comments.child("artists");
        assert_eq!(artists.field("tour_dates"), None);
        assert_eq!(
            artists.child("tour_dates").own,
            Some("Upcoming shows.".to_string())
        );
    }

    #[test]
    fn an_array_of_inline_tables_stays_a_field() {
        // The inline spelling of a oneof, which is a field rather than a child.
        let comments = extract(
            r#"
[artists]
# Media attached to this artist.
media = [{ name = "video", type = "video" }]
"#,
        );
        let artists = comments.child("artists");
        assert_eq!(
            artists.field("media"),
            Some("Media attached to this artist.".to_string())
        );
        assert_eq!(artists.child("media"), &DefinitionComments::default());
    }

    #[test]
    fn handles_quoted_keys() {
        let comments = extract(
            r#"
[artists]
# A field with an awkward name.
"first name" = "string"
"#,
        );
        // Keys are decoded, matching how `toml` reads them.
        assert_eq!(
            comments.child("artists").field("first name"),
            Some("A field with an awkward name.".to_string())
        );
    }

    #[test]
    fn handles_dotted_keys() {
        let comments = extract(
            r#"
[artists]
# The display name.
name.first = "string"
"#,
        );
        // The comment is written onto the last key, which is where the field
        // itself ends up.
        assert_eq!(
            comments.child("artists").child("name").field("first"),
            Some("The display name.".to_string())
        );
    }

    #[test]
    fn handles_crlf_line_endings() {
        let comments = extract("[artists]\r\n# The name.\r\nname = \"string\"\r\n");
        assert_eq!(
            comments.child("artists").field("name"),
            Some("The name.".to_string())
        );
    }

    #[test]
    fn strips_hashes_and_surrounding_whitespace() {
        let comments = extract(
            r#"
[artists]
   #    Indented, padded.
   #Tight.
name = "string"
"#,
        );
        assert_eq!(
            comments.child("artists").field("name"),
            Some("Indented, padded.\nTight.".to_string())
        );
    }

    #[test]
    fn an_uncommented_document_yields_nothing() {
        let comments = extract(
            r#"
[artists]
name = "string"
[artists.tour_dates]
date = "date"
"#,
        );
        assert!(comments.is_empty());
    }

    #[test]
    fn a_leading_file_comment_describes_the_first_object() {
        // There is nowhere else for document-leading trivia to go, so this is
        // the documented convention: separate a file header with a blank line.
        let described = extract("# The site's objects.\n[artists]\nname = \"string\"\n");
        assert_eq!(
            described.child("artists").own,
            Some("The site's objects.".to_string())
        );

        let separated = extract("# The site's objects.\n\n[artists]\nname = \"string\"\n");
        assert_eq!(separated.child("artists").own, None);
    }
}
