use std::{collections::HashMap, error::Error, fmt};

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

pub struct PageTemplate<'a> {
    pub definition: &'a ObjectDefinition,
    pub object: &'a Object,
    pub content: String,
}

pub struct Page<'a> {
    name: String,
    content: Option<String>,
    template: Option<PageTemplate<'a>>,
}

impl<'a> Page<'a> {
    pub fn new_with_template(
        name: String,
        definition: &'a ObjectDefinition,
        object: &'a Object,
        content: String,
    ) -> Page<'a> {
        Page {
            name,
            content: None,
            template: Some(PageTemplate {
                definition,
                object,
                content,
            }),
        }
    }
    pub fn new(name: String, content: String) -> Page<'a> {
        Page {
            name,
            content: Some(content),
            template: None,
        }
    }
    pub fn render(
        &self,
        parser: &liquid::Parser,
        objects_map: &HashMap<String, Vec<Object>>,
    ) -> Result<String, Box<dyn Error>> {
        let globals = liquid::object!({ "objects": objects_map, "page": self.name });
        if let Some(template_info) = &self.template {
            let template = parser.parse(&template_info.content)?;
            // TODO: parse markdown and other parsed types
            let mut context = liquid::object!({
              "object_name": template_info.object.name,
              "order": template_info.object.order
            });
            context.extend(globals);
            return Ok(template.render(&context)?);
        } else if let Some(content) = &self.content {
            let template = parser.parse(&content)?;
            return Ok(template.render(&globals)?);
        }
        panic!("Pages must have either a template or a path");
    }
}

#[cfg(test)]
mod tests {
    // use crate::liquid_parser;

    // use super::*;

    // struct TestData {
    //     page_content: String,
    //     template_content: String,
    //     objects_map: HashMap<String, Vec<Object>>,
    // }
    // fn test_data() -> TestData {
    //     let mut objects_map = HashMap::new();
    //     objects_map.insert(
    //         "test",
    //         vec![Object {
    //             name: "test1".to_string(),
    //             object_name: "test".to_string(),
    //             order: 0,
    //             values: HashMap::from([
    //                 ("title", "foo"),
    //                 ("child", HashMap::from([("foo", "bar")])),
    //             ]),
    //         }],
    //     );
    //     TestData {
    //         page_content: "".to_string(),
    //         template_content: "".to_string(),
    //         objects_map,
    //     }
    // }

    // #[test]
    // fn it_renders_regular_pages() {
    //     let liquid_parser = liquid_parser::get();
    //     let t = test_data();
    //     let page = Page::new("test".to_string(), t.page_content);
    //     let rendered = page.render(&liquid_parser, &objects_map)?;
    // }
}
