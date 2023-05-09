use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use liquid;

use crate::objects::{Object, ObjectDefinition};

#[derive(Debug, Clone)]
struct InvalidPageError;
impl Error for InvalidPageError {}
impl fmt::Display for InvalidPageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid page")
    }
}

pub struct Page {
    pub definition: ObjectDefinition,
    pub object_path: PathBuf,
    pub template_path: PathBuf,
    pub object: Option<Object>,
    pub template: Option<String>,
}

impl Page {
    pub fn new(
        definition: &ObjectDefinition,
        object_path: &Path,
        template_path: &Path,
    ) -> Result<Page, Box<dyn Error>> {
        let page = Page {
            definition: definition.clone(),
            object_path: object_path.to_path_buf(),
            template_path: template_path.to_path_buf(),
            object: None,
            template: None,
        };
        page.load()?;
        Ok(page)
    }
    fn load(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    pub fn render(&self, parser: liquid::Parser) -> Result<String, Box<dyn Error>> {
        let template_str = match &self.template {
            Some(t) => Ok(t),
            None => Err(InvalidPageError),
        }?;
        let template = parser.parse(&template_str)?;
        let object = match &self.object {
            Some(t) => Ok(t),
            None => Err(InvalidPageError),
        }?;
        // TODO: parse markdown and other parsed types
        let mut globals = liquid::object!({
          "object_name": object.name,
          "order": object.order
        });
        globals.extend(liquid::object!({}));
        Ok(template.render(&globals)?)
    }
}
