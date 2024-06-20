use crate::{page::TemplateType, tags::layout::LayoutTag, FileSystemAPI};
use liquid_core::partials::{EagerCompiler, PartialSource};
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    error::Error,
    path::Path,
};
use tracing::{debug, error, warn};

pub static PARTIAL_FILE_NAME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^_(.+)\.liquid").unwrap());

#[derive(Default, Debug, Clone)]
struct ArchivalPartial {
    template_type: TemplateType,
    content: String,
}
impl ArchivalPartial {
    fn new(content: String, template_type: TemplateType) -> Self {
        Self {
            template_type,
            content,
        }
    }
}

#[derive(Default, Debug, Clone)]
struct ArchivalPartialSource {
    partials: HashMap<String, ArchivalPartial>,
    layouts: HashSet<String>,
}

impl ArchivalPartialSource {
    pub fn new(
        pages_path: Option<&Path>,
        layout_path: Option<&Path>,
        fs: &impl FileSystemAPI,
    ) -> Result<Self, Box<dyn Error>> {
        let mut partials = HashMap::new();
        let mut layouts = HashSet::new();
        // Add layouts
        if let Some(path) = layout_path {
            for file in fs.walk_dir(path, false)? {
                if let Some(name) = file.file_name().map(|f| f.to_str().unwrap()) {
                    if let Some((template_name, t)) = TemplateType::parse_path(name) {
                        debug!("adding layout {} ({})", template_name, t.extension());
                        if let Some(content) = fs.read_to_string(&path.join(&file))? {
                            layouts.insert(template_name.to_string());
                            partials.insert(
                                template_name.to_string(),
                                ArchivalPartial::new(content, t),
                            );
                        } else {
                            error!("Failed reading layout {}", file.display());
                        }
                    } else {
                        warn!(
                            "invalid layout file {}. Try adding .[ext].liquid to the filename.",
                            name
                        );
                    }
                }
            }
        }
        if let Some(path) = pages_path {
            for file in fs.walk_dir(path, false)? {
                if let Some(name) = file.file_name().map(|f| f.to_str().unwrap()) {
                    if PARTIAL_FILE_NAME_RE.is_match(name) {
                        debug!("partial at path {:?}", file);
                        let (partial_name, t) = TemplateType::parse_path(name).unwrap();
                        // Remove underscore from beginning of name
                        let partial_name = &partial_name[1..];
                        // Prepend path to this file if needed
                        let partial_name = if let Some(parent_dir) = file.parent() {
                            parent_dir.join(partial_name).to_string_lossy().to_string()
                        } else {
                            partial_name.to_string()
                        };
                        debug!("adding partial {} ({})", partial_name, t.extension());
                        if let Some(content) = fs.read_to_string(&path.join(&file))? {
                            partials
                                .insert(partial_name.to_string(), ArchivalPartial::new(content, t));
                        } else {
                            error!("Failed reading partial {}", file.display());
                        }
                    }
                }
            }
        }
        Ok(Self { partials, layouts })
    }

    fn layout_types(&self) -> HashMap<String, TemplateType> {
        let mut types = HashMap::new();
        for layout in &self.layouts {
            types.insert(
                layout.to_owned(),
                self.partials.get(layout).unwrap().template_type.clone(),
            );
        }
        types
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
        self.partials.get(name).map(|p| p.content.to_owned().into())
    }
}

pub fn get(
    pages_path: Option<&Path>,
    layout_path: Option<&Path>,
    fs: &impl FileSystemAPI,
) -> Result<liquid::Parser, Box<dyn Error>> {
    let partial_source = ArchivalPartialSource::new(pages_path, layout_path, fs)?;
    let parser =
        liquid::ParserBuilder::with_stdlib().tag(LayoutTag::new(partial_source.layout_types()));
    let partials = EagerCompiler::new(partial_source.clone());
    Ok(parser.partials(partials).build()?)
}
