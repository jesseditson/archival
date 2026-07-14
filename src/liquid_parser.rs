use crate::{page::TemplateType, tags::layout::LayoutTag, FileSystemAPI};
use anyhow::Result;
use liquid_core::partials::{EagerCompiler, PartialSource};
use once_cell::sync::Lazy;
use regex::Regex;
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
                        // Prepend path to this file if needed
                        let partial_name = if let Some(parent_dir) = file.parent() {
                            parent_dir.join(partial_name).to_string_lossy().to_string()
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

    fn try_get<'a>(&'a self, name: &str) -> Option<Cow<'a, str>> {
        self.partials.get(name).map(|p| p.into())
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
    let partials = EagerCompiler::new(source);
    let parser = liquid::ParserBuilder::with_stdlib()
        .tag(LayoutTag)
        .partials(partials);
    Ok(parser.build()?)
}
