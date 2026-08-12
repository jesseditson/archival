//! Open buffers, and the conversion between LSP positions and byte offsets.
//!
//! LSP positions are a zero-based line plus a count of UTF-16 code units into
//! that line, while everything in archival works in byte offsets.

use lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};
use std::collections::HashMap;

/// The byte offset each line of a text starts at.
pub(crate) struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i as u32 + 1),
        );
        Self { starts }
    }

    /// The byte offset `line` starts at, or `None` past the last line.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.starts.get(line).map(|start| *start as usize)
    }

    /// The line `offset` falls on, and the byte offset that line starts at.
    fn line_at(&self, offset: usize) -> (usize, usize) {
        let line = self
            .starts
            .partition_point(|start| *start as usize <= offset)
            - 1;
        (line, self.starts[line] as usize)
    }

    pub fn position(&self, text: &str, offset: usize) -> Position {
        let offset = offset.min(text.len());
        let (line, start) = self.line_at(offset);
        Position {
            line: line as u32,
            character: text[start..offset].encode_utf16().count() as u32,
        }
    }

    /// The byte offset of `position`, clamped into `text` so that a stale
    /// position from the client cannot panic a slice.
    pub fn offset(&self, text: &str, position: Position) -> usize {
        let Some(start) = self.starts.get(position.line as usize) else {
            return text.len();
        };
        let start = *start as usize;
        let line = &text[start..self.line_end(text, position.line as usize)];
        let mut units = 0;
        for (at, c) in line.char_indices() {
            if units >= position.character as usize {
                return start + at;
            }
            units += c.len_utf16();
        }
        start + line.len()
    }

    fn line_end(&self, text: &str, line: usize) -> usize {
        self.starts
            .get(line + 1)
            .map_or(text.len(), |next| *next as usize)
    }
}

pub(crate) struct Document {
    pub text: String,
    pub index: LineIndex,
}

impl Document {
    pub fn new(text: String) -> Self {
        Self {
            index: LineIndex::new(&text),
            text,
        }
    }

    pub fn range(&self, span: std::ops::Range<usize>) -> Range {
        Range {
            start: self.index.position(&self.text, span.start),
            end: self.index.position(&self.text, span.end),
        }
    }

    /// Applies one content change. A change with no range replaces the whole
    /// document.
    fn apply(&mut self, change: TextDocumentContentChangeEvent) {
        match change.range {
            Some(range) => {
                let start = self.index.offset(&self.text, range.start);
                let end = self.index.offset(&self.text, range.end).max(start);
                self.text.replace_range(start..end, &change.text);
            }
            None => self.text = change.text,
        }
        self.index = LineIndex::new(&self.text);
    }
}

#[derive(Default)]
pub(crate) struct Documents {
    open: HashMap<Url, Document>,
}

impl Documents {
    pub fn open(&mut self, uri: Url, text: String) {
        self.open.insert(uri, Document::new(text));
    }

    pub fn change(&mut self, uri: &Url, changes: Vec<TextDocumentContentChangeEvent>) {
        let Some(doc) = self.open.get_mut(uri) else {
            return;
        };
        for change in changes {
            doc.apply(change);
        }
    }

    pub fn close(&mut self, uri: &Url) {
        self.open.remove(uri);
    }

    pub fn get(&self, uri: &Url) -> Option<&Document> {
        self.open.get(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Document {
        Document::new(text.to_string())
    }

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn converts_between_offsets_and_positions() {
        let d = doc("ab\ncde\n\nf");
        for (offset, position) in [
            (0, pos(0, 0)),
            (2, pos(0, 2)),
            (3, pos(1, 0)),
            (6, pos(1, 3)),
            (7, pos(2, 0)),
            (8, pos(3, 0)),
        ] {
            assert_eq!(d.index.position(&d.text, offset), position, "at {offset}");
            assert_eq!(d.index.offset(&d.text, position), offset, "at {position:?}");
        }
    }

    /// Characters are counted in UTF-16 code units, so astral characters
    /// advance a position by two while advancing an offset by four.
    #[test]
    fn counts_characters_in_utf16() {
        let d = doc("é🎉x");
        assert_eq!(d.index.position(&d.text, 0), pos(0, 0));
        assert_eq!(d.index.position(&d.text, 2), pos(0, 1));
        assert_eq!(d.index.position(&d.text, 6), pos(0, 3));
        assert_eq!(d.index.offset(&d.text, pos(0, 3)), 6);
    }

    #[test]
    fn applies_ranged_and_whole_document_changes() {
        let mut d = doc("hello\nworld");
        d.apply(TextDocumentContentChangeEvent {
            range: Some(Range {
                start: pos(1, 0),
                end: pos(1, 5),
            }),
            range_length: None,
            text: "there".into(),
        });
        assert_eq!(d.text, "hello\nthere");
        assert_eq!(d.index.position(&d.text, 6), pos(1, 0));

        d.apply(TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "new".into(),
        });
        assert_eq!(d.text, "new");
    }

    /// A position past the end of a line, or past the end of the document,
    /// clamps rather than panicking.
    #[test]
    fn clamps_out_of_range_positions() {
        let d = doc("ab\ncd");
        assert_eq!(d.index.offset(&d.text, pos(0, 99)), 3);
        assert_eq!(d.index.offset(&d.text, pos(99, 0)), 5);
        assert_eq!(d.index.position(&d.text, 999), pos(1, 2));
    }
}
