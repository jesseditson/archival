//! Reproducible build-performance benchmarks.
//!
//! Run with `cargo bench --bench build`. Criterion stores results in
//! `target/criterion`, and compares against the previous run, so numbers can
//! be tracked over time and across changes.
//!
//! The synthetic site mirrors the shape of a real blog: a root `site` object
//! plus N articles with multi-kilobyte markdown bodies, a template page per
//! article, and a listing page that embeds every body (like an RSS feed).

use archival::{Archival, BuildOptions, FileSystemAPI, MemoryFileSystem};
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use std::fmt::Write;
use std::path::Path;

const ARTICLE_COUNT: usize = 30;

fn markdown_body(seed: usize) -> String {
    let mut body = String::new();
    for section in 0..8 {
        writeln!(body, "## Section {section} of article {seed}\n").unwrap();
        for para in 0..3 {
            writeln!(
                body,
                "This is paragraph {para} with a [link](https://example.com/{seed}/{section}) \
                 and some *emphasis* and `inline code`. Vestibulum id ligula porta felis \
                 euismod semper. Cras justo odio, dapibus ac facilisis in, egestas eget quam.\n"
            )
            .unwrap();
        }
        body.push_str("```rust\nfn example() -> usize {\n    42\n}\n```\n\n");
        body.push_str("- list item one\n- list item two\n- list item three\n\n");
    }
    body
}

fn site_fs() -> MemoryFileSystem {
    let mut fs = MemoryFileSystem::default();
    fs.write_str("manifest.toml", "upload_prefix = \"\"\n".to_string())
        .unwrap();
    fs.write_str(
        "objects.toml",
        r#"
[site]
name = "string"
tagline = "string"

[articles]
template = "article"
headline = "string"
slug = "string"
date = "date"
published = "boolean"
body = "markdown"
"#
        .to_string(),
    )
    .unwrap();
    fs.write_str(
        "objects/site.toml",
        "name = \"Bench Site\"\ntagline = \"A site used for benchmarking\"\n".to_string(),
    )
    .unwrap();
    for i in 0..ARTICLE_COUNT {
        fs.write_str(
            format!("objects/articles/article-{i}.toml"),
            format!(
                "headline = \"Article {i}\"\nslug = \"article-{i}\"\ndate = \"2024-01-{:02}\"\npublished = true\norder = {i}\nbody = '''\n{}'''\n",
                (i % 28) + 1,
                markdown_body(i)
            ),
        )
        .unwrap();
    }
    fs.write_str(
        "pages/article.liquid",
        r#"{% layout 'theme' %}
<article>
  <h1>{{ articles.headline }}</h1>
  {{ articles.body }}
</article>
"#
        .to_string(),
    )
    .unwrap();
    fs.write_str(
        "pages/index.liquid",
        r#"{% layout 'theme' %}
<ul>
{% for article in articles %}
  <li><a href="/{{ article.path }}.html">{{ article.headline }}</a></li>
  {{ article.body }}
{% endfor %}
</ul>
"#
        .to_string(),
    )
    .unwrap();
    fs.write_str(
        "layout/theme.liquid",
        r#"<!doctype html>
<html>
  <head><title>{{ objects.site.name }}</title></head>
  <body>
    <header>{{ objects.site.tagline }}</header>
    {{ page_content }}
  </body>
</html>
"#
        .to_string(),
    )
    .unwrap();
    fs.write_str(
        "public/styles.css",
        "body { font-family: sans-serif; }\n".to_string(),
    )
    .unwrap();
    fs
}

/// A build with cold site caches (objects are re-read and re-parsed), as run
/// by `archival build`. Note that the process-wide markdown cache is shared
/// across iterations, so this measures steady-state build cost rather than
/// first-ever-build cost.
fn bench_full_build(c: &mut Criterion) {
    let fs = site_fs();
    c.bench_function("full_build_30_articles", |b| {
        b.iter_batched(
            || Archival::new(fs.clone()).unwrap(),
            |archival| archival.build(BuildOptions::default()).unwrap(),
            BatchSize::LargeInput,
        )
    });
}

/// A rebuild after one article changed, with warm caches — this is the loop
/// the dev server runs on file change, and should stay in the low
/// milliseconds.
fn bench_rebuild(c: &mut Criterion) {
    let fs = site_fs();
    let archival = Archival::new(fs).unwrap();
    archival.build(BuildOptions::default()).unwrap();
    c.bench_function("rebuild_after_article_change", |b| {
        b.iter(|| {
            archival
                .site
                .invalidate_file(Path::new("objects/articles/article-0.toml"));
            archival.build(BuildOptions::default()).unwrap()
        })
    });
}

criterion_group!(benches, bench_full_build, bench_rebuild);
criterion_main!(benches);
