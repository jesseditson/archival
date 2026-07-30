//! Rewrites liquid output statements (`{{ … }}`) into archival's internal
//! [`OUTPUT_TAG`] so that liquid contained in a rendered *value* can be
//! rendered in place, against the live runtime (see `crate::tags::output`).
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
//! Where the scanner is unsure it emits the source verbatim, but only in cases
//! that are already parse errors (an unterminated `{{`, an unterminated string,
//! an empty `{{}}`). Falling back to verbatim for input that parses today would
//! silently stop rendering liquid in field values.

use std::borrow::Cow;

/// The internal tag every `{{ … }}` output statement is rewritten to.
pub(crate) const OUTPUT_TAG: &str = "__archival_out";

/// Rewrites every output statement in `source` to [`OUTPUT_TAG`], preserving
/// whitespace control and leaving `{% raw %}` bodies untouched. Returns
/// `Cow::Borrowed` when there was nothing to rewrite, which is the common case.
pub(crate) fn rewrite_outputs(source: &str) -> Cow<'_, str> {
    if !source.contains("{{") {
        return Cow::Borrowed(source);
    }
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
                if tag_name(bytes, i + 2) == b"raw" {
                    i = skip_raw_block(bytes, i + 2);
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
                let inner = source[inner_start..inner_end].trim_matches(is_liquid_whitespace);
                if inner.is_empty() {
                    // `{{}}` is a parse error; don't turn it into a tag whose
                    // error would be less recognizable.
                    i = close_end;
                    continue;
                }
                out.push_str(&source[copied..i]);
                out.push_str(if dash_open { "{%- " } else { "{% " });
                out.push_str(OUTPUT_TAG);
                out.push(' ');
                out.push_str(inner);
                out.push_str(if dash_close { " -%}" } else { " %}" });
                copied = close_end;
                i = close_end;
            }
            _ => i += 1,
        }
    }
    if copied == 0 {
        return Cow::Borrowed(source);
    }
    out.push_str(&source[copied..]);
    Cow::Owned(out)
}

/// Scans forward from `from` for the two-byte delimiter `close`, skipping over
/// quoted string literals. Returns the index the delimiter starts at and the
/// index just past it, or `None` if the delimiter (or a closing quote) is
/// missing.
fn scan_to(bytes: &[u8], from: usize, close: [u8; 2]) -> Option<(usize, usize)> {
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
fn is_liquid_whitespace(c: char) -> bool {
    c == ' ' || c == '\n' || c == '\r'
}

/// Reads the identifier naming the tag that starts at `from` (the index just
/// past `{%`), skipping the whitespace-control hyphen and any whitespace the
/// grammar allows before it.
fn tag_name(bytes: &[u8], from: usize) -> &[u8] {
    let mut i = from;
    if bytes.get(i) == Some(&b'-') {
        i += 1;
    }
    while matches!(bytes.get(i), Some(b' ' | b'\n' | b'\r')) {
        i += 1;
    }
    let start = i;
    while matches!(bytes.get(i), Some(b) if b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-') {
        i += 1;
    }
    &bytes[start..i]
}

/// Given `from` (the index just past the `{%` of a `raw` tag), returns the
/// index just past the matching `{% endraw %}`, or the end of the source if
/// there isn't one. Everything in between is copied verbatim.
fn skip_raw_block(bytes: &[u8], from: usize) -> usize {
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
        rewrite_outputs(source).into_owned()
    }

    fn unchanged(source: &str) {
        assert!(
            matches!(rewrite_outputs(source), Cow::Borrowed(_)),
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
    fn leaves_raw_blocks_alone() {
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
    fn is_idempotent() {
        for source in CORPUS {
            let once = rewrite(source);
            assert_eq!(rewrite(&once), once, "not idempotent: {source:?}");
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
            let rewritten = rewrite_outputs(source);
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
