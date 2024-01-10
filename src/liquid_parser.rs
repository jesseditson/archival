use regex::Regex;
use std::{
    borrow::Cow,
    collections::HashMap,
    error::Error,
    fs::{self, read_dir},
    path::PathBuf,
};

use liquid_core::{
    parser,
    partials::{EagerCompiler, PartialSource},
    runtime, Language, Template,
};

use crate::tags::layout::LayoutTag;

fn liquid_extension() -> Regex {
    Regex::new(r"\.(liquid|html)").unwrap()
}

#[derive(Default, Debug, Clone)]
struct LayoutPartialSource {
    layouts: HashMap<String, String>,
}

impl LayoutPartialSource {
    pub fn new(path: Option<PathBuf>) -> Result<Self, std::io::Error> {
        let mut layouts = HashMap::new();
        if let Some(path) = path {
            let files = read_dir(path)?;
            let ext_re = liquid_extension();
            for file in files {
                if let Ok(file) = file {
                    if let Some(name) = file.file_name().to_str() {
                        if ext_re.is_match(name) {
                            let template_name = ext_re.replace(name, "").to_string();
                            let contents = fs::read_to_string(file.path())?;
                            layouts.insert(template_name, contents);
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
        if let Some(v) = self.layouts.get(name) {
            Some(v.into())
        } else {
            None
        }
    }
}

pub fn get(layout_path: Option<PathBuf>) -> Result<liquid::Parser, Box<dyn Error>> {
    let layout_partials = EagerCompiler::new(LayoutPartialSource::new(layout_path)?);
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
