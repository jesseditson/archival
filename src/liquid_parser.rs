use regex::Regex;
use std::{borrow::Cow, collections::HashMap, error::Error, path::Path};

use liquid_core::{
    parser,
    partials::{EagerCompiler, PartialSource},
    runtime, Language, Template,
};

use crate::{tags::layout::LayoutTag, FileSystemAPI};

fn liquid_extension() -> Regex {
    Regex::new(r"\.(liquid|html)").unwrap()
}

#[derive(Default, Debug, Clone)]
struct LayoutPartialSource {
    layouts: HashMap<String, String>,
}

impl LayoutPartialSource {
    pub fn new(path: Option<&Path>, fs: &impl FileSystemAPI) -> Result<Self, Box<dyn Error>> {
        let mut layouts = HashMap::new();
        if let Some(path) = path {
            let files = fs.read_dir(path)?;
            let ext_re = liquid_extension();
            for file in files {
                if let Some(name) = file.file_name().map(|f| f.to_str().unwrap()) {
                    if ext_re.is_match(name) {
                        let template_name = ext_re.replace(name, "").to_string();
                        if let Some(contents) = fs.read_to_string(&file)? {
                            layouts.insert(template_name, contents);
                        } else {
                            println!("Failed reading {}", file.display());
                        }
                    }
                }
            }
        }
        Ok(Self { layouts })
    }
}

impl PartialSource for LayoutPartialSource {
    fn contains(&self, name: &str) -> bool {
        self.layouts.contains_key(name)
    }

    fn names(&self) -> Vec<&str> {
        let mut names = vec![];
        for k in self.layouts.keys() {
            names.push(&k[..]);
        }
        names
    }

    fn try_get<'a>(&'a self, name: &str) -> Option<Cow<'a, str>> {
        self.layouts.get(name).map(|layout| layout.into())
    }
}

pub fn get(
    layout_path: Option<&Path>,
    fs: &impl FileSystemAPI,
) -> Result<liquid::Parser, Box<dyn Error>> {
    let layout_partials = EagerCompiler::new(LayoutPartialSource::new(layout_path, fs)?);
    let parser = liquid::ParserBuilder::with_stdlib()
        .tag(LayoutTag)
        .partials(layout_partials);
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
