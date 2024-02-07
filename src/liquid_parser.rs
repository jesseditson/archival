use crate::{tags::layout::LayoutTag, FileSystemAPI};
use liquid_core::{
    parser,
    partials::{EagerCompiler, PartialSource},
    runtime, Language, Template,
};
use regex::Regex;
use std::{borrow::Cow, collections::HashMap, error::Error, path::Path};
use tracing::error;

fn liquid_extension() -> Regex {
    Regex::new(r"\.(liquid|html)").unwrap()
}
pub fn partial_matcher() -> Regex {
    Regex::new(r"^_(.+)\.(liquid|html)").unwrap()
}

#[derive(Default, Debug, Clone)]
struct ArchivalPartialSource {
    partials: HashMap<String, String>,
}

impl ArchivalPartialSource {
    pub fn new(
        pages_path: Option<&Path>,
        layout_path: Option<&Path>,
        fs: &impl FileSystemAPI,
    ) -> Result<Self, Box<dyn Error>> {
        let mut partials = HashMap::new();
        // Add layouts
        if let Some(path) = layout_path {
            let ext_re = liquid_extension();
            for file in fs.walk_dir(path, false)? {
                if let Some(name) = file.file_name().map(|f| f.to_str().unwrap()) {
                    if ext_re.is_match(name) {
                        let template_name = ext_re.replace(name, "").to_string();
                        if let Some(contents) = fs.read_to_string(&path.join(&file))? {
                            partials.insert(template_name, contents);
                        } else {
                            error!("Failed reading layout {}", file.display());
                        }
                    }
                }
            }
        }
        if let Some(path) = pages_path {
            let partial_re = partial_matcher();
            for file in fs.walk_dir(path, false)? {
                if let Some(name) = file.file_name().map(|f| f.to_str().unwrap()) {
                    println!("NAME: {}", name);
                    if partial_re.is_match(name) {
                        let template_name = partial_re.replace(name, "$1").to_string();
                        if let Some(contents) = fs.read_to_string(&path.join(&file))? {
                            partials.insert(template_name, contents);
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

pub fn get(
    pages_path: Option<&Path>,
    layout_path: Option<&Path>,
    fs: &impl FileSystemAPI,
) -> Result<liquid::Parser, Box<dyn Error>> {
    let partials = EagerCompiler::new(ArchivalPartialSource::new(pages_path, layout_path, fs)?);
    let parser = liquid::ParserBuilder::with_stdlib()
        .tag(LayoutTag)
        .partials(partials);
    Ok(parser.build()?)
}

pub trait ToTemplate {
    fn to_template(&self, options: &Language) -> Result<Template, Box<dyn Error>>;
}
impl ToTemplate for &str {
    fn to_template(&self, options: &Language) -> Result<Template, Box<dyn Error>> {
        Ok(parser::parse(self, options).map(runtime::Template::new)?)
    }
}
