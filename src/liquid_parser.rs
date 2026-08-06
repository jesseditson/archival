use crate::liquid_rewrite::rewrite_template;
use crate::tags::include::IncludeTag;
use crate::tags::output::{OutputContext, OutputTag};
use crate::tags::render::RenderTag;
use crate::{page::TemplateType, tags::layout::LayoutTag, util::path_to_slash, FileSystemAPI};
use anyhow::Result;
use liquid_core::partials::{EagerCompiler, PartialCompiler, PartialSource};
use liquid_core::runtime::PartialStore;
use liquid_core::Language;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;
use std::{borrow::Cow, collections::HashMap, path::Path};
#[cfg(feature = "verbose-logging")]
use tracing::debug;
use tracing::error;

pub static PARTIAL_FILE_NAME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^_(.+)\.liquid").unwrap());

#[derive(Default, Debug, Clone)]
pub(crate) struct ArchivalPartialSource {
    partials: HashMap<String, String>,
}

impl ArchivalPartialSource {
    pub fn new(
        pages_path: Option<&Path>,
        layout_path: Option<&Path>,
        fs: &impl FileSystemAPI,
    ) -> Result<Self> {
        let mut partials = HashMap::new();
        // Add layouts
        if let Some(path) = layout_path {
            for file in fs.walk_dir(path, false)? {
                if let Some(name) = file.file_name().map(|f| f.to_str().unwrap()) {
                    if let Some((template_name, _t)) = TemplateType::parse_path(name) {
                        #[cfg(feature = "verbose-logging")]
                        debug!("adding layout {} ({})", template_name, _t.extension());
                        if let Some(contents) = fs.read_to_string(path.join(&file))? {
                            partials.insert(template_name.to_string(), contents);
                        } else {
                            error!("Failed reading layout {}", file.display());
                        }
                    }
                }
            }
        }
        if let Some(path) = pages_path {
            for file in fs.walk_dir(path, false)? {
                if let Some(name) = file.file_name().map(|f| f.to_str().unwrap()) {
                    if PARTIAL_FILE_NAME_RE.is_match(name) {
                        #[cfg(feature = "verbose-logging")]
                        debug!("partial at path {:?}", file);
                        let (partial_name, _t) = TemplateType::parse_path(name).unwrap();
                        // Remove underscore from beginning of name
                        let partial_name = &partial_name[1..];
                        // Prepend path to this file if needed. Partials are
                        // referenced by name in templates (`{% include
                        // "dir/partial" %}`), so the name always uses `/`.
                        let partial_name = if let Some(parent_dir) = file.parent() {
                            path_to_slash(parent_dir.join(partial_name))
                        } else {
                            partial_name.to_string()
                        };
                        #[cfg(feature = "verbose-logging")]
                        debug!("adding partial {} ({})", partial_name, _t.extension());
                        if let Some(contents) = fs.read_to_string(path.join(&file))? {
                            partials.insert(partial_name.to_string(), contents);
                        } else {
                            error!("Failed reading partial {}", file.display());
                        }
                    }
                }
            }
        }
        Ok(Self { partials })
    }
}

impl ArchivalPartialSource {
    /// A stable hash of all partial names and contents. Used to decide whether
    /// a cached parser (whose compiled partials embed these sources) is still
    /// valid.
    pub fn source_hash(&self) -> u64 {
        use std::hash::Hasher;
        let mut keys: Vec<&String> = self.partials.keys().collect();
        keys.sort();
        let mut hasher = seahash::SeaHasher::new();
        for key in keys {
            hasher.write(key.as_bytes());
            hasher.write(&[0]);
            hasher.write(self.partials[key].as_bytes());
            hasher.write(&[0]);
        }
        hasher.finish()
    }
}

impl PartialSource for ArchivalPartialSource {
    fn contains(&self, name: &str) -> bool {
        self.partials.contains_key(name)
    }

    fn names(&self) -> Vec<&str> {
        let mut names = vec![];
        for k in self.partials.keys() {
            names.push(&k[..]);
        }
        names
    }

    /// Partials and layouts are rewritten on the way to the compiler so that
    /// field values they output are rendered in place (see
    /// `crate::liquid_rewrite`). Rewriting happens here rather than in `new()`
    /// because every build constructs an `ArchivalPartialSource` just to hash
    /// it, and only compiles on a cache miss.
    fn try_get<'a>(&'a self, name: &str) -> Option<Cow<'a, str>> {
        self.partials.get(name).map(|p| rewrite_template(p))
    }
}

// Builds cache parsers via Site (see partials_hash/build_with_partials); this
// one-shot constructor is kept for callers (and tests) that need a standalone
// parser.
#[allow(dead_code)]
pub fn get(
    pages_path: Option<&Path>,
    layout_path: Option<&Path>,
    fs: &impl FileSystemAPI,
) -> Result<liquid::Parser> {
    let source = ArchivalPartialSource::new(pages_path, layout_path, fs)?;
    build_with_partials(source)
}

/// Reads all partial/layout sources and returns their combined hash. Reading
/// and hashing sources is much cheaper than compiling them, so builds use this
/// to decide whether a cached parser can be reused.
pub(crate) fn partials_hash(
    pages_path: Option<&Path>,
    layout_path: Option<&Path>,
    fs: &impl FileSystemAPI,
) -> Result<(ArchivalPartialSource, u64)> {
    let source = ArchivalPartialSource::new(pages_path, layout_path, fs)?;
    let hash = source.source_hash();
    Ok((source, hash))
}

pub(crate) fn build_with_partials(source: ArchivalPartialSource) -> Result<liquid::Parser> {
    Ok(build_with_output_context(source)?.0)
}

/// Also returns the parser's [`OutputContext`], whose lifetime is tied to the
/// parser: it holds a `Weak` reference to the parser's `Language`, and its
/// cache of parsed field values is only valid for that `Language`.
pub(crate) fn build_with_output_context(
    source: ArchivalPartialSource,
) -> Result<(liquid::Parser, Arc<OutputContext>)> {
    let ctx = Arc::new(OutputContext::default());
    let partials = LanguageCapturingCompiler {
        inner: EagerCompiler::new(source),
        ctx: Arc::clone(&ctx),
    };
    let parser = liquid::ParserBuilder::with_stdlib()
        .tag(LayoutTag)
        .tag(IncludeTag)
        .tag(RenderTag)
        .tag(OutputTag::new(Arc::clone(&ctx)))
        .partials(partials);
    Ok((parser.build()?, ctx))
}

/// Parses a template, rewriting its output statements first so that liquid
/// inside the values it renders is evaluated in place. Every liquid template
/// archival parses should go through here.
pub(crate) fn parse(
    parser: &liquid::Parser,
    source: &str,
) -> std::result::Result<liquid::Template, liquid_core::Error> {
    parser.parse(&rewrite_template(source))
}

/// A pass-through partial compiler whose only job is to capture the
/// `Arc<Language>` that `ParserBuilder::build` hands to the compiler. That is
/// the only way to get hold of it, and the output tag needs it in order to
/// parse liquid found inside values.
struct LanguageCapturingCompiler<C: PartialCompiler> {
    inner: C,
    ctx: Arc<OutputContext>,
}

impl<C: PartialCompiler> PartialCompiler for LanguageCapturingCompiler<C> {
    fn compile(
        self,
        language: Arc<Language>,
    ) -> liquid_core::Result<Box<dyn PartialStore + Send + Sync>> {
        self.ctx.set_language(&language);
        self.inner.compile(language)
    }

    fn source(&self) -> &dyn PartialSource {
        self.inner.source()
    }
}
