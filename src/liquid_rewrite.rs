//! Rewrites liquid output statements (`{{ … }}`) into archival's internal
//! [`OUTPUT_TAG`] so that liquid contained in a rendered *value* can be
//! rendered in place, against the live runtime (see `crate::tags::output`).
//!
//! It also strips Shopify-style inline comments (`{% # … %}`). `liquid-core`'s
//! grammar requires a tag to open with an `Identifier`, so a `#` tag can't be
//! registered with `ParserBuilder::tag` — it fails to lex before tag lookup
//! happens. Removing them here is what makes them a comment.
//!
//! Shopify's `{% liquid %}` and `{% echo %}` are expanded here for a related
//! reason: `WHITESPACE` below is implicit throughout the grammar, so a tag's
//! newlines are gone by the time a `ParseTag` sees its tokens — and newlines are
//! exactly what separates one statement in a `{% liquid %}` body from the next.
//! Only the source still has them.
//!
//! This is a hand-written scanner standing in for liquid's pest grammar, so it
//! depends on the following rules from `liquid-core`'s `grammar.pest`. If the
//! liquid dependency is ever bumped, re-read them:
//!
//! ```text
//! WHITESPACE = _{" " | NEWLINE }
//! NON_WHITESPACE_CONTROL_HYPHEN = _{ !"-}}" ~ !"-%}" ~ "-" }
//! TagStart = _{ (WHITESPACE* ~ "{%-") | "{%" }    ExpressionStart = _{ (WHITESPACE* ~ "{{-") | "{{" }
//! TagEnd   = _{ ("-%}" ~ WHITESPACE*) | "%}" }    ExpressionEnd   = _{ ("-}}" ~ WHITESPACE*) | "}}" }
//! TagInner = !{Identifier ~ TagToken*}            ExpressionInner = !{FilterChain}
//! StringLiteral = @{ ("'" ~ (!"'" ~ ANY)* ~ "'") | ("\"" ~ (!"\"" ~ ANY)* ~ "\"") }
//! ```
//!
//! Four consequences make the transform equivalent rather than heuristic:
//!
//! 1. `TagToken` includes `FilterChain`, so a tag's arguments are a strict
//!    superset of an expression's body: nothing that parses inside `{{ }}`
//!    fails to parse as `{% __archival_out … %}`.
//! 2. Tags and expressions share identical whitespace-control machinery, so
//!    `{{-` -> `{%-` and `-}}` -> `-%}` trims exactly the same bytes.
//! 3. A `-` immediately preceding `}}` can never belong to the expression, so
//!    "the byte before `}}` is `-`" soundly detects whitespace control.
//! 4. String literals have no escapes, so skipping a quoted run is trivial.
//!
//! `{% raw %}` writes its body out verbatim, so raw bodies are left alone.
//! `{% comment %}` discards its body, so rewriting inside one is harmless and
//! is not special-cased.
//!
//! An inline comment's body is text rather than liquid, so — matching Shopify —
//! it ends at the first `%}` with no regard for quoting, and an apostrophe in it
//! opens nothing.
//!
//! Where the scanner is unsure it emits the source verbatim, but only in cases
//! that are already parse errors (an unterminated `{{`, an unterminated string,
//! an empty `{{}}`). Falling back to verbatim for input that parses today would
//! silently stop rendering liquid in field values.

use std::borrow::Cow;

/// The internal tag every `{{ … }}` output statement is rewritten to.
pub(crate) const OUTPUT_TAG: &str = "__archival_out";

/// A rewritten template, and where it lines up with the source it came from.
pub(crate) struct Rewrite<'a> {
    pub text: Cow<'a, str>,
    #[cfg(feature = "lsp")]
    pub anchors: Anchors,
}

impl<'a> Rewrite<'a> {
    fn new(text: Cow<'a, str>, _anchors: Anchors) -> Self {
        Self {
            text,
            #[cfg(feature = "lsp")]
            anchors: _anchors,
        }
    }
}

/// Points at which the rewritten text and its source agree, recorded only for
/// the language server — nothing else maps a position back.
#[derive(Default)]
pub(crate) struct Anchors {
    /// `(rewritten_offset, source_offset)`, ascending. Between one anchor and
    /// the next the two texts advance in lockstep, so an offset resolves by
    /// linear interpolation from the anchor governing it.
    #[cfg(feature = "lsp")]
    map: Vec<(u32, u32)>,
    #[cfg(feature = "lsp")]
    source_len: u32,
}

impl Anchors {
    fn new(_source_len: usize) -> Self {
        Self {
            #[cfg(feature = "lsp")]
            map: Vec::new(),
            #[cfg(feature = "lsp")]
            source_len: _source_len as u32,
        }
    }

    #[inline]
    fn record(&mut self, _rewritten: usize, _source: usize) {
        #[cfg(feature = "lsp")]
        self.map.push((_rewritten as u32, _source as u32));
    }
}

#[cfg(feature = "lsp")]
impl Anchors {
    /// The offset in the original source that `offset` in the rewritten text
    /// came from. The liquid parser only ever sees rewritten text, so its
    /// positions come back through here before they can be shown against a
    /// file.
    pub fn to_source(&self, offset: usize) -> usize {
        let source_len = self.source_len as usize;
        let Some(governing) = self
            .map
            .partition_point(|(at, _)| *at as usize <= offset)
            .checked_sub(1)
        else {
            return self
                .map
                .first()
                .map_or(offset, |(_, at)| *at as usize)
                .min(source_len);
        };
        let (from, to) = self.map[governing];
        // A replacement need not be the length of what it replaced, so an
        // offset inside one resolves no further than the region it belongs to.
        let limit = self
            .map
            .get(governing + 1)
            .map_or(source_len, |(_, at)| *at as usize);
        (to as usize + (offset - from as usize)).min(limit)
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Rewrites every output statement in `source` to [`OUTPUT_TAG`], expands
/// `{% liquid %}` and `{% echo %}`, and removes every inline comment,
/// preserving whitespace control and leaving `{% raw %}` bodies untouched.
/// Returns `Cow::Borrowed` when there was nothing to rewrite, which is the
/// common case.
pub(crate) fn rewrite_template(source: &str) -> Cow<'_, str> {
    rewrite_template_mapped(source).text
}

/// [`rewrite_template`], keeping the mapping back to `source`.
pub(crate) fn rewrite_template_mapped(source: &str) -> Rewrite<'_> {
    let borrowed = |anchors| Rewrite::new(Cow::Borrowed(source), anchors);
    if !source.contains("{{") && !source.contains("{%") {
        return borrowed(Anchors::new(source.len()));
    }
    let mut anchors = Anchors::new(source.len());
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + 16);
    // The end of the region already flushed into `out`. All delimiters are
    // ASCII, so every index used to slice `source` is a char boundary.
    let mut copied = 0;
    let mut i = 0;
    while i + 1 < len {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        match bytes[i + 1] {
            b'%' => {
                let name = tag_name(bytes, i + 2);
                let expandable = name == b"liquid" || name == b"echo";
                if name == b"raw" {
                    i = skip_raw_block(bytes, i + 2);
                } else if is_inline_comment(bytes, i + 2) || expandable {
                    let body_start = tag_body_start(bytes, i + 2) + name.len();
                    // A `{% liquid %}` body is line-oriented, so it ends at the
                    // first `%}` outside a string on a statement line. A comment
                    // is text and ends at the first `%}` outright.
                    let close = match name {
                        b"liquid" => find_liquid_tag_end(bytes, body_start),
                        b"echo" => scan_to(bytes, body_start, *b"%}").map(|(at, _)| at),
                        _ => find_tag_end(bytes, i + 2),
                    };
                    let Some(close_at) = close else {
                        // Unterminated tag: a parse error either way.
                        i += 2;
                        continue;
                    };
                    let dash_open = bytes.get(i + 2) == Some(&b'-');
                    let dash_close = bytes[close_at - 1] == b'-';
                    let before = &source[copied..i];
                    anchors.record(out.len(), copied);
                    out.push_str(if dash_open {
                        before.trim_end_matches(is_liquid_whitespace)
                    } else {
                        before
                    });
                    if expandable {
                        let body_end = if dash_close { close_at - 1 } else { close_at };
                        let body = &source[body_start..body_end];
                        if name == b"liquid" {
                            expand_statements(body, body_start, &mut out, &mut anchors);
                        } else {
                            let at = body_start + leading_whitespace(body);
                            expand_echo(body.trim(), at, &mut out, &mut anchors);
                        }
                    }
                    let mut next = close_at + 2;
                    if dash_close {
                        let rest = &source[next..];
                        next += rest.len() - rest.trim_start_matches(is_liquid_whitespace).len();
                    }
                    copied = next;
                    i = next;
                } else {
                    i = scan_to(bytes, i + 2, *b"%}").map_or(i + 2, |(_, end)| end);
                }
            }
            b'{' => {
                let dash_open = bytes.get(i + 2) == Some(&b'-');
                let inner_start = i + 2 + usize::from(dash_open);
                let Some((close_at, close_end)) = scan_to(bytes, inner_start, *b"}}") else {
                    // Unterminated expression or string literal: a parse error
                    // either way, so leave the remainder alone.
                    break;
                };
                let dash_close = close_at > inner_start && bytes[close_at - 1] == b'-';
                let inner_end = if dash_close { close_at - 1 } else { close_at };
                // Trimming only what the grammar counts as whitespace keeps
                // input that fails to parse today (a tab inside `{{ }}`, say)
                // failing to parse after the rewrite.
                let raw_inner = &source[inner_start..inner_end];
                let inner = raw_inner.trim_matches(is_liquid_whitespace);
                if inner.is_empty() {
                    // `{{}}` is a parse error; don't turn it into a tag whose
                    // error would be less recognizable.
                    i = close_end;
                    continue;
                }
                anchors.record(out.len(), copied);
                out.push_str(&source[copied..i]);
                out.push_str(if dash_open { "{%- " } else { "{% " });
                out.push_str(OUTPUT_TAG);
                out.push(' ');
                let inner_at = inner_start
                    + (raw_inner.len() - raw_inner.trim_start_matches(is_liquid_whitespace).len());
                anchors.record(out.len(), inner_at);
                out.push_str(inner);
                out.push_str(if dash_close { " -%}" } else { " %}" });
                copied = close_end;
                i = close_end;
            }
            _ => i += 1,
        }
    }
    if copied == 0 {
        #[cfg(feature = "lsp")]
        debug_assert!(
            anchors.is_empty(),
            "nothing was rewritten, so nothing to map"
        );
        return borrowed(anchors);
    }
    anchors.record(out.len(), copied);
    out.push_str(&source[copied..]);
    Rewrite::new(Cow::Owned(out), anchors)
}

/// The offset [`str::trim`] starts at.
fn leading_whitespace(s: &str) -> usize {
    s.len() - s.trim_start().len()
}

/// Scans forward from `from` for the two-byte delimiter `close`, skipping over
/// quoted string literals. Returns the index the delimiter starts at and the
/// index just past it, or `None` if the delimiter (or a closing quote) is
/// missing.
pub(crate) fn scan_to(bytes: &[u8], from: usize, close: [u8; 2]) -> Option<(usize, usize)> {
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            quote @ (b'\'' | b'"') => {
                i += 1;
                loop {
                    if i >= bytes.len() {
                        return None;
                    }
                    let closed = bytes[i] == quote;
                    i += 1;
                    if closed {
                        break;
                    }
                }
            }
            b if b == close[0] && bytes.get(i + 1) == Some(&close[1]) => return Some((i, i + 2)),
            _ => i += 1,
        }
    }
    None
}

/// `WHITESPACE = _{" " | NEWLINE }` in liquid's grammar — notably not a tab.
pub(crate) fn is_liquid_whitespace(c: char) -> bool {
    c == ' ' || c == '\n' || c == '\r'
}

/// Expands a `{% liquid %}` body, where each line is one statement written
/// without its `{% %}`. `body_start` is where `body` begins in the source, so
/// that each statement can be anchored back to the line it was written on.
fn expand_statements(body: &str, body_start: usize, out: &mut String, anchors: &mut Anchors) {
    let mut at = body_start;
    for line in body.split_inclusive('\n') {
        expand_statement(line, at, out, anchors);
        at += line.len();
    }
}

/// Writes the tag one `{% liquid %}` statement stands for. Blank lines and `#`
/// comment lines produce nothing, and `echo` is an output statement.
fn expand_statement(line: &str, line_start: usize, out: &mut String, anchors: &mut Anchors) {
    let statement = line.trim();
    if statement.is_empty() || statement.starts_with('#') {
        return;
    }
    let at = line_start + leading_whitespace(line);
    match echo_argument(statement) {
        Some(expression) => expand_echo(
            expression,
            at + (statement.len() - expression.len()),
            out,
            anchors,
        ),
        None => {
            out.push_str("{% ");
            anchors.record(out.len(), at);
            out.push_str(statement);
            out.push_str(" %}");
        }
    }
}

/// The expression of an `echo` statement, or `None` if it is a different tag.
fn echo_argument(statement: &str) -> Option<&str> {
    let rest = statement.strip_prefix("echo")?;
    (rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_whitespace())).then(|| rest.trim())
}

/// `echo` is an output statement written as a tag. Without an expression it
/// outputs nothing.
fn expand_echo(expression: &str, at: usize, out: &mut String, anchors: &mut Anchors) {
    if expression.is_empty() {
        return;
    }
    out.push_str("{% ");
    out.push_str(OUTPUT_TAG);
    out.push(' ');
    anchors.record(out.len(), at);
    out.push_str(expression);
    out.push_str(" %}");
}

/// Finds the `%}` closing a `{% liquid %}` tag that starts at `from`, returning
/// the index it starts at. A `#` line is a comment, so quotes in it open
/// nothing — an apostrophe in one must not swallow the rest of the tag.
pub(crate) fn find_liquid_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        while matches!(bytes.get(i), Some(b' ' | b'\t' | b'\r')) {
            i += 1;
        }
        let comment = bytes.get(i) == Some(&b'#');
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    i += 1;
                    break;
                }
                quote @ (b'\'' | b'"') if !comment => {
                    i += 1;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    if i >= bytes.len() {
                        return None;
                    }
                    i += 1;
                }
                b'%' if bytes.get(i + 1) == Some(&b'}') => return Some(i),
                _ => i += 1,
            }
        }
    }
    None
}

/// Finds the `%}` closing the tag that starts at `from`, ignoring quoting.
/// Returns the index the delimiter starts at.
pub(crate) fn find_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    (from..bytes.len().saturating_sub(1)).find(|&i| bytes[i] == b'%' && bytes[i + 1] == b'}')
}

/// Skips the whitespace-control hyphen and any whitespace the grammar allows
/// before a tag's first token, given `from` (the index just past `{%`).
pub(crate) fn tag_body_start(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    if bytes.get(i) == Some(&b'-') {
        i += 1;
    }
    while matches!(bytes.get(i), Some(b' ' | b'\n' | b'\r')) {
        i += 1;
    }
    i
}

/// Whether the tag starting at `from` is an inline comment (`{% # … %}`).
pub(crate) fn is_inline_comment(bytes: &[u8], from: usize) -> bool {
    bytes.get(tag_body_start(bytes, from)) == Some(&b'#')
}

/// Reads the identifier naming the tag that starts at `from` (the index just
/// past `{%`).
pub(crate) fn tag_name(bytes: &[u8], from: usize) -> &[u8] {
    let mut i = tag_body_start(bytes, from);
    let start = i;
    while matches!(bytes.get(i), Some(b) if b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-') {
        i += 1;
    }
    &bytes[start..i]
}

/// Given `from` (the index just past the `{%` of a `raw` tag), returns the
/// index just past the matching `{% endraw %}`, or the end of the source if
/// there isn't one. Everything in between is copied verbatim.
pub(crate) fn skip_raw_block(bytes: &[u8], from: usize) -> usize {
    let mut i = scan_to(bytes, from, *b"%}").map_or(from, |(_, end)| end);
    while i + 1 < bytes.len() {
        if bytes[i] != b'{' || bytes[i + 1] != b'%' {
            i += 1;
            continue;
        }
        let name = tag_name(bytes, i + 2);
        match scan_to(bytes, i + 2, *b"%}") {
            Some((_, end)) => {
                i = end;
                if name == b"endraw" {
                    return i;
                }
            }
            None => i += 2,
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite(source: &str) -> String {
        rewrite_template(source).into_owned()
    }

    fn unchanged(source: &str) {
        assert!(
            matches!(rewrite_template(source), Cow::Borrowed(_)),
            "expected {source:?} to be left alone, got {:?}",
            rewrite(source)
        );
    }

    #[test]
    fn rewrites_output_statements() {
        assert_eq!(rewrite("{{x}}"), "{% __archival_out x %}");
        assert_eq!(rewrite("{{ x }}"), "{% __archival_out x %}");
        assert_eq!(rewrite("a{{ x }}b"), "a{% __archival_out x %}b");
        assert_eq!(
            rewrite("{{a}}{{b}}{{c}}"),
            "{% __archival_out a %}{% __archival_out b %}{% __archival_out c %}"
        );
        assert_eq!(
            rewrite("{{ x | append: 'y' }}"),
            "{% __archival_out x | append: 'y' %}"
        );
        assert_eq!(rewrite("{{ x }}}"), "{% __archival_out x %}}");
    }

    #[test]
    fn preserves_whitespace_control() {
        assert_eq!(rewrite("{{- x -}}"), "{%- __archival_out x -%}");
        assert_eq!(rewrite("{{-x-}}"), "{%- __archival_out x -%}");
        assert_eq!(rewrite("{{- x }}"), "{%- __archival_out x %}");
        assert_eq!(rewrite("{{ x -}}"), "{% __archival_out x -%}");
        assert_eq!(rewrite("a\n  {{- x }}b"), "a\n  {%- __archival_out x %}b");
    }

    #[test]
    fn respects_string_literals() {
        assert_eq!(
            rewrite(r#"{{ x | append: "}}" }}"#),
            r#"{% __archival_out x | append: "}}" %}"#
        );
        assert_eq!(
            rewrite("{{ x | append: '{%' }}"),
            "{% __archival_out x | append: '{%' %}"
        );
        assert_eq!(
            rewrite(r#"{% assign a = "{{" %}{{ a }}"#),
            r#"{% assign a = "{{" %}{% __archival_out a %}"#
        );
        assert_eq!(
            rewrite(r#"{% assign a = "%}" %}{{ a }}"#),
            r#"{% assign a = "%}" %}{% __archival_out a %}"#
        );
    }

    #[test]
    fn strips_inline_comments() {
        assert_eq!(rewrite("{% # comment %}"), "");
        assert_eq!(rewrite("{%# comment %}"), "");
        assert_eq!(rewrite("{% #comment %}"), "");
        assert_eq!(rewrite("{% # %}"), "");
        assert_eq!(rewrite("a{% # c %}b"), "ab");
        assert_eq!(rewrite("{% # a %}{% # b %}"), "");
        assert_eq!(rewrite("{%\n  # multi\n  # line\n%}"), "");
        assert_eq!(
            rewrite("{% # prettier-ignore %}{{ x }}"),
            "{% __archival_out x %}"
        );
        // A comment body is text, so quotes in it open nothing.
        assert_eq!(rewrite("a{% # it's fine %}b"), "ab");
        assert_eq!(rewrite(r#"a{% # "unclosed %}b"#), "ab");
        assert_eq!(rewrite("a{% # {{ x }} %}b"), "ab");
    }

    #[test]
    fn inline_comments_preserve_whitespace_control() {
        assert_eq!(rewrite("a\n  {%- # c %}\nb"), "a\nb");
        assert_eq!(rewrite("a\n{% # c -%}\n  b"), "a\nb");
        assert_eq!(rewrite("a {%- # c -%} b"), "ab");
        assert_eq!(rewrite("a {%-# c-%} b"), "ab");
        assert_eq!(rewrite("a {% # c %} b"), "a  b");
    }

    #[test]
    fn leaves_raw_blocks_alone() {
        unchanged("{% raw %}{% # c %}{% endraw %}");
        unchanged("{% raw %}{{ x }}{% endraw %}");
        unchanged("{%- raw -%}{{ x }}{%- endraw -%}");
        unchanged("{%-raw%}{{x}}{%-endraw%}");
        // Unclosed raw is a parse error with or without the rewrite.
        unchanged("{% raw %}{{ x }}");
        assert_eq!(
            rewrite("{% raw %}{{ x }}{% endraw %}{{ y }}"),
            "{% raw %}{{ x }}{% endraw %}{% __archival_out y %}"
        );
        assert_eq!(
            rewrite("{% raw %}{{ unclosed{% endraw %}{{ y }}"),
            "{% raw %}{{ unclosed{% endraw %}{% __archival_out y %}"
        );
    }

    #[test]
    fn rewrites_inside_comments() {
        // Harmless: `{% comment %}` discards its body rather than emitting it.
        assert_eq!(
            rewrite("{% comment %}{% # c %}{% endcomment %}ok"),
            "{% comment %}{% endcomment %}ok"
        );
        assert_eq!(
            rewrite("{% comment %}{{ x }}{% endcomment %}"),
            "{% comment %}{% __archival_out x %}{% endcomment %}"
        );
    }

    #[test]
    fn leaves_malformed_input_alone() {
        unchanged("hello");
        unchanged("{{ x");
        unchanged(r#"{{ "abc }}"#);
        unchanged("{{}}");
        unchanged("{{ }}");
        unchanged("{{-}}");
        unchanged("{% assign x = 1 %}");
        unchanged("{% # c");
    }

    #[test]
    fn rewrites_inside_markdown_code_fences() {
        // Code fences aren't liquid-aware; this matches the pre-existing
        // second-pass behavior.
        assert_eq!(
            rewrite("```\n{{ x }}\n```"),
            "```\n{% __archival_out x %}\n```"
        );
    }

    #[test]
    fn expands_the_liquid_tag() {
        assert_eq!(rewrite("{% liquid assign a = 1 %}"), "{% assign a = 1 %}");
        assert_eq!(
            rewrite("{% liquid\n  assign a = 1\n  echo a\n%}"),
            "{% assign a = 1 %}{% __archival_out a %}"
        );
        assert_eq!(
            rewrite("{% liquid\n\n  # note\n  echo 'x'\n%}"),
            "{% __archival_out 'x' %}"
        );
        // Tabs are not whitespace to liquid's grammar, so indentation has to be
        // trimmed rather than carried into the emitted tag.
        assert_eq!(rewrite("{% liquid\n\techo a\n%}"), "{% __archival_out a %}");
        assert_eq!(
            rewrite("{% echo a | upcase %}"),
            "{% __archival_out a | upcase %}"
        );
        assert_eq!(rewrite("{% liquid %}"), "");
        assert_eq!(rewrite("{% echo %}"), "");
    }

    #[test]
    fn liquid_tag_preserves_whitespace_control() {
        assert_eq!(
            rewrite("a {%- liquid echo b -%} c"),
            "a{% __archival_out b %}c"
        );
        assert_eq!(
            rewrite("a {%- liquid\n  echo b\n-%} c"),
            "a{% __archival_out b %}c"
        );
    }

    /// An apostrophe in a comment line must not be read as opening a string
    /// literal and swallowing the rest of the tag.
    #[test]
    fn liquid_tag_comments_are_text() {
        assert_eq!(
            rewrite("{% liquid # it's a note\n  echo a\n%}"),
            "{% __archival_out a %}"
        );
        assert_eq!(
            rewrite("{% liquid\n  # don't\n  echo a\n%}"),
            "{% __archival_out a %}"
        );
    }

    #[test]
    fn liquid_tag_respects_string_literals() {
        assert_eq!(
            rewrite("{% liquid assign a = '%}' %}"),
            "{% assign a = '%}' %}"
        );
    }

    #[test]
    fn is_idempotent() {
        let sources = CORPUS
            .iter()
            .copied()
            .chain(COMMENT_CORPUS.iter().map(|(source, _)| *source))
            .chain(LIQUID_TAG_CORPUS.iter().map(|(source, _)| *source));
        for source in sources {
            let once = rewrite(source);
            assert_eq!(rewrite(&once), once, "not idempotent: {source:?}");
        }
    }

    /// The source `needle` maps back to, as a suffix of `source`.
    #[cfg(feature = "lsp")]
    fn map_back<'a>(source: &'a str, needle: &str) -> &'a str {
        let rewritten = rewrite_template_mapped(source);
        let at = rewritten
            .text
            .find(needle)
            .unwrap_or_else(|| panic!("{needle:?} is not in {:?}", rewritten.text));
        &source[rewritten.anchors.to_source(at)..]
    }

    #[cfg(feature = "lsp")]
    fn line_of(source: &str, needle: &str) -> usize {
        let rewritten = rewrite_template_mapped(source);
        let at = rewritten
            .anchors
            .to_source(rewritten.text.find(needle).unwrap());
        source[..at].matches('\n').count() + 1
    }

    #[cfg(feature = "lsp")]
    #[test]
    fn maps_rewritten_offsets_back_to_source() {
        assert!(map_back("a {{ x }} b", "x %}").starts_with("x }} b"));
        assert!(map_back("{{- x -}}", "x -%}").starts_with("x -}}"));
        assert!(map_back("{{a}}{{b}}{{c}}", "c %}").starts_with("c}}"));
        assert!(map_back("{{ x }}tail", "tail").starts_with("tail"));
        // Nothing rewritten: the mapping is the identity.
        assert!(map_back("plain text", "text").starts_with("text"));
    }

    /// Expansion collapses a body onto one line, so every statement in it and
    /// everything below it resolves by its own offset.
    #[cfg(feature = "lsp")]
    #[test]
    fn maps_liquid_statements_back_to_their_own_line() {
        let source = "{% liquid\n  assign one = 1\n  echo two\n%}\n{{ three }}\n{{ four }}";
        assert_eq!(line_of(source, "assign one = 1"), 2);
        assert_eq!(line_of(source, "two %}"), 3);
        assert_eq!(line_of(source, "three %}"), 5);
        assert_eq!(line_of(source, "four %}"), 6);
    }

    #[cfg(feature = "lsp")]
    #[test]
    fn maps_around_removed_and_expanded_regions() {
        // An inline comment leaves no output; what follows still maps.
        assert_eq!(line_of("{% # note %}\n{{ x }}", "x %}"), 2);
        assert_eq!(line_of("{% echo a %}\n{{ b }}", "b %}"), 2);
        assert!(map_back("{% echo a | upcase %}", "a | upcase").starts_with("a | upcase %}"));
    }

    /// Every offset lands on a char boundary inside the source, and the mapping
    /// is monotonic, so a mapped offset is always safe to slice at.
    #[cfg(feature = "lsp")]
    #[test]
    fn maps_every_offset_into_the_source() {
        let sources = CORPUS
            .iter()
            .copied()
            .chain(COMMENT_CORPUS.iter().map(|(source, _)| *source))
            .chain(LIQUID_TAG_CORPUS.iter().map(|(source, _)| *source))
            .chain(["héllo {{ wörld }} 🎉", "{% liquid\n echo é\n%}ü"]);
        for source in sources {
            let rewritten = rewrite_template_mapped(source);
            let mut last = 0;
            for offset in 0..=rewritten.text.len() {
                if !rewritten.text.is_char_boundary(offset) {
                    continue;
                }
                let at = rewritten.anchors.to_source(offset);
                assert!(
                    at <= source.len() && source.is_char_boundary(at),
                    "{source:?}: offset {offset} mapped to {at}, not a boundary"
                );
                assert!(at >= last, "{source:?}: mapping went backwards at {offset}");
                last = at;
            }
        }
    }

    /// Inline comments can't live in [`CORPUS`]: they fail to parse without the
    /// rewrite, which is the whole point of stripping them. Each entry pairs a
    /// template with what it must render to.
    const COMMENT_CORPUS: &[(&str, &str)] = &[
        ("{% # c %}", ""),
        ("{%# c %}", ""),
        ("{% # %}", ""),
        ("a{% # c %}b", "ab"),
        ("a {%- # c -%} b", "ab"),
        ("{% # it's a comment %}", ""),
        (r#"{% # a "quoted" comment %}"#, ""),
        ("{% # {{ name }} %}", ""),
        ("{% # if name %}", ""),
        ("{%\n  # multi\n  # line\n%}", ""),
        ("{% # prettier-ignore %}{{ name }}", "Archival"),
        ("{% if name %}{% # c %}{{ name }}{% endif %}", "Archival"),
        ("{% for i in list %}{% # c %}{{ i }}{% endfor %}", "onetwo"),
    ];

    /// Like [`COMMENT_CORPUS`], these can't live in [`CORPUS`]: liquid has no
    /// `liquid` or `echo` tag, so none of them parse without the expansion.
    const LIQUID_TAG_CORPUS: &[(&str, &str)] = &[
        ("{% liquid echo name %}", "Archival"),
        ("{% liquid\n  echo name\n%}", "Archival"),
        ("{% liquid\n  assign a = name\n  echo a\n%}", "Archival"),
        ("{% liquid\n  echo name | upcase\n%}", "ARCHIVAL"),
        ("{% echo name %}", "Archival"),
        ("{% echo name | upcase %}", "ARCHIVAL"),
        ("{% echo %}", ""),
        ("{% liquid %}", ""),
        ("{% liquid\n  # only a comment\n%}", ""),
        // Blocks work because each line becomes an ordinary tag.
        (
            "{% liquid\n  if name\n    echo name\n  else\n    echo 'none'\n  endif\n%}",
            "Archival",
        ),
        (
            "{% liquid\n  for i in list\n    echo i\n  endfor\n%}",
            "onetwo",
        ),
        (
            "{% liquid\n  case name\n  when 'Archival'\n    echo 'yes'\n  else\n    echo 'no'\n  endcase\n%}",
            "yes",
        ),
        (
            "{% liquid\n  capture c\n  endcapture\n  assign a = 'x'\n  echo a\n%}",
            "x",
        ),
        // A block may open inside the tag and close outside it, and vice versa.
        ("{% liquid if name %}{{ name }}{% liquid endif %}", "Archival"),
        ("{% if name %}{% liquid echo name %}{% endif %}", "Archival"),
        ("a{% liquid echo name %}b", "aArchivalb"),
        ("{% liquid assign a = '%}' \n echo a %}", "%}"),
        ("{% liquid # it's a note\n echo name %}", "Archival"),
    ];

    #[test]
    fn liquid_tags_render_like_their_expansion() {
        let parser = crate::liquid_parser::build_with_partials(Default::default()).unwrap();
        let globals = liquid::object!({
            "name": "Archival",
            "list": ["one", "two"],
        });
        for (source, expected) in LIQUID_TAG_CORPUS {
            assert!(
                parser.parse(source).is_err(),
                "{source:?} parses as stock liquid; it no longer exercises the expansion"
            );
            let rendered = crate::liquid_parser::parse(&parser, source)
                .unwrap_or_else(|e| panic!("failed parsing {source:?}: {e}"))
                .render(&globals)
                .unwrap_or_else(|e| panic!("failed rendering {source:?}: {e}"));
            assert_eq!(&rendered, expected, "output mismatch for {source:?}");
        }
    }

    #[test]
    fn inline_comments_render_as_nothing() {
        let parser = crate::liquid_parser::build_with_partials(Default::default()).unwrap();
        let globals = liquid::object!({
            "name": "Archival",
            "list": ["one", "two"],
        });
        for (source, expected) in COMMENT_CORPUS {
            assert!(
                parser.parse(source).is_err(),
                "{source:?} parses as stock liquid; it no longer exercises the strip"
            );
            let rendered = crate::liquid_parser::parse(&parser, source)
                .unwrap_or_else(|e| panic!("failed parsing {source:?}: {e}"))
                .render(&globals)
                .unwrap_or_else(|e| panic!("failed rendering {source:?}: {e}"));
            assert_eq!(&rendered, expected, "output mismatch for {source:?}");
        }
    }

    /// Templates covering every shape the scanner has to reason about. Values
    /// deliberately contain no liquid, so rewriting must not change rendering.
    const CORPUS: &[&str] = &[
        "",
        "hello",
        "{{ name }}",
        "{{name}}",
        "{{ name | upcase }}",
        "{{ name | append: '!' }}",
        r#"{{ name | append: "}}" }}"#,
        "{{ name | append: '{%' }}",
        "{{ nested.a }}",
        "{{ list[0] }}",
        "{{ 'literal' }}",
        "{{ 42 }}",
        "{{ missing }}",
        "{{- name -}}",
        "  {{- name -}}  ",
        "a\n{{- name }}\nb",
        "{{ name }}{{ name }}",
        "x{{ name }}y{{ nested.a }}z",
        "{% assign a = name %}{{ a }}",
        "{% assign a = 'x' %}{{ a }}{{ name }}",
        r#"{% assign a = "{{" %}{{ a }}"#,
        r#"{% assign a = "%}" %}{{ a }}"#,
        "{% if name %}{{ name }}{% endif %}",
        "{% unless name %}no{% else %}{{ name }}{% endunless %}",
        "{% for i in list %}{{ i }}{{ forloop.index }}{% endfor %}",
        "{% for i in list %}{%- if i %}{{- i -}}{% endif -%}{% endfor %}",
        "{% capture c %}{{ name }}{% endcapture %}{{ c }}",
        "{% case name %}{% when 'a' %}{{ name }}{% else %}x{% endcase %}",
        "{% comment %}{{ name }}{% endcomment %}ok",
        "{% raw %}{{ name }}{% endraw %}",
        "{% raw %}{{ name }}{% endraw %}{{ name }}",
        "{%- raw -%}{{ name }}{%- endraw -%}",
        "{% raw %}{% if %}{% endraw %}{{ name }}",
        "{{ name }}}",
        "}}{{ name }}",
        "{{{ name }}}",
        "{{}}",
        "{{ x",
        r#"{{ "abc }}"#,
        "{% tablerow i in list %}{{ i }}{% endtablerow %}",
        "<script>var a = 1;</script>{{ name }}",
        "```\n{{ name }}\n```",
    ];

    /// The scanner is a second lexer for liquid's grammar; this is the guard
    /// against it diverging. For every template, rewriting must not change
    /// whether it parses, nor what it renders.
    #[test]
    fn rewriting_preserves_parse_and_render() {
        let parser = crate::liquid_parser::build_with_partials(Default::default()).unwrap();
        let globals = liquid::object!({
            "name": "Archival",
            "nested": { "a": "A" },
            "list": ["one", "two"],
        });
        for source in CORPUS {
            let rewritten = rewrite_template(source);
            let before = parser.parse(source);
            let after = parser.parse(&rewritten);
            assert_eq!(
                before.is_err(),
                after.is_err(),
                "parse mismatch for {source:?} (rewritten: {rewritten:?}): {:?} vs {:?}",
                before.err().map(|e| e.to_string()),
                after.err().map(|e| e.to_string()),
            );
            let (Ok(before), Ok(after)) = (before, after) else {
                continue;
            };
            let before = before.render(&globals);
            // Templates whose stock output itself contains liquid are exactly
            // the ones this change is about: the rewritten version renders that
            // liquid rather than emitting it. Only their parseability is
            // comparable.
            if before
                .as_ref()
                .is_ok_and(|r| r.contains("{{") || r.contains("{%"))
            {
                continue;
            }
            let after = after.render(&globals);
            assert_eq!(
                before.is_err(),
                after.is_err(),
                "render mismatch for {source:?}: {:?} vs {:?}",
                before.as_ref().err().map(|e| e.to_string()),
                after.as_ref().err().map(|e| e.to_string()),
            );
            if let (Ok(before), Ok(after)) = (before, after) {
                assert_eq!(before, after, "output mismatch for {source:?}");
            }
        }
    }
}
