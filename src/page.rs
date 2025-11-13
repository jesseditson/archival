use crate::{
    object::{Object, ObjectEntry},
    object_definition::ObjectDefinition,
    FieldConfig, ObjectDefinitions, ObjectMap,
};
use liquid::{model::ScalarCow, ValueView};
use liquid_core::Value;
use once_cell::sync::Lazy;
use pluralizer::pluralize;
use regex::Regex;
use std::{
    borrow::Cow,
    env,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
struct InvalidPageError;
impl Error for InvalidPageError {}
impl fmt::Display for InvalidPageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid page")
    }
}

static TEMPLATE_FILE_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+?)(\.\w+)?\.liquid").unwrap());

#[derive(Default, Debug, Clone)]
pub enum TemplateType {
    #[default]
    Default,
    Html,
    Css,
    Csv,
    Json,
    Rss,
    Unknown(String),
}

impl TemplateType {
    pub fn from_ext(ext: &str) -> Self {
        match &ext.to_lowercase()[..] {
            "html" => Self::Html,
            "css" => Self::Css,
            "csv" => Self::Csv,
            "json" => Self::Json,
            "rss" => Self::Rss,
            "" => Self::Default,
            r => Self::Unknown(r.to_string()),
        }
    }
    pub fn parse_path(name: &str) -> Option<(&str, Self)> {
        if let Some(m) = TEMPLATE_FILE_NAME_RE.captures(name) {
            let name = m.get(1);
            let type_extension = m.get(2);
            let name = name?.as_str();
            if type_extension.is_none() {
                return Some((name, Self::default()));
            }
            let type_extension = type_extension.unwrap().as_str();
            return Some((name, Self::from_ext(&type_extension[1..])));
        }
        None
    }
    pub fn extension(&self) -> &str {
        match self {
            TemplateType::Default => "html",
            TemplateType::Html => "html",
            TemplateType::Css => "css",
            TemplateType::Csv => "css",
            TemplateType::Json => "json",
            TemplateType::Rss => "rss",
            TemplateType::Unknown(r) => r,
        }
    }
}

#[derive(Debug)]
pub struct PageTemplate<'a> {
    pub definition: &'a ObjectDefinition,
    pub object: &'a Object,
    pub content: String,
    #[allow(dead_code)]
    pub file_type: TemplateType,
    pub debug_path: PathBuf,
}

#[derive(Debug)]
pub struct RenderGlobals<'a> {
    pub site_url: Cow<'a, str>,
}

impl RenderGlobals<'_> {
    fn inject(&self, object: &mut liquid::Object) {
        object.insert("site_url".into(), Value::scalar(self.site_url.to_string()));
    }
}

pub struct Page<'a> {
    name: String,
    content: Option<String>,
    template: Option<PageTemplate<'a>>,
    file_type: TemplateType,
    globals: &'a RenderGlobals<'a>,
    pub debug_path: Option<PathBuf>,
}

pub(crate) fn debug_context(object: &liquid::Object, lp: usize) -> String {
    let mut debug_str = String::default();
    fn to_str(val: &Value, lp: usize) -> String {
        let include_values = env::var("ARCHIVAL_CONTEXT_VALUES").is_ok();
        match val {
            Value::Object(o) => debug_context(o, lp + 1),
            Value::Array(a) => {
                if include_values {
                    format!(
                        "\n{}↘︎[{} items]{}\n{}⎼⎼⎼",
                        "  ".repeat(lp),
                        a.len(),
                        if a.is_empty() {
                            "".to_string()
                        } else {
                            a.iter()
                                .map(|v| to_str(v, lp + 1))
                                .collect::<Vec<String>>()
                                .join(&format!("\n{}--", "  ".repeat(lp + 2)))
                        },
                        "  ".repeat(lp),
                    )
                } else {
                    format!(
                        "\n{}↘︎[{} items]{}",
                        "  ".repeat(lp),
                        a.len(),
                        a.first().map(|v| to_str(v, lp + 1)).unwrap_or_default()
                    )
                }
            }
            Value::Nil => " (nil)".to_string(),
            Value::Scalar(s) => {
                if include_values {
                    format!(" ({}: {:?})", val.type_name(), s.as_view())
                } else {
                    format!(": ({})", val.type_name())
                }
            }
            _ => format!(": ({})", val.type_name()),
        }
    }
    for k in object.keys() {
        debug_str += &format!("\n{}⌗{}", "  ".repeat(lp), k);
        let ev = liquid_core::Value::Scalar(ScalarCow::new("empty"));
        let val = object.get(k).unwrap_or(&ev);
        debug_str += &to_str(val, lp + 1);
    }
    debug_str
}

impl<'a> Page<'a> {
    pub fn new_with_template(
        name: String,
        definition: &'a ObjectDefinition,
        object: &'a Object,
        content: String,
        file_type: TemplateType,
        globals: &'a RenderGlobals<'a>,
        template_debug_path: &Path,
    ) -> Page<'a> {
        Page {
            name,
            content: None,
            template: Some(PageTemplate {
                definition,
                object,
                content,
                file_type: file_type.clone(),
                debug_path: template_debug_path.to_path_buf(),
            }),
            file_type,
            globals,
            debug_path: None,
        }
    }
    pub fn new(
        name: String,
        content: String,
        file_type: TemplateType,
        globals: &'a RenderGlobals<'a>,
        debug_path: &Path,
    ) -> Page<'a> {
        Page {
            name,
            content: Some(content),
            template: None,
            file_type,
            globals,
            debug_path: Some(debug_path.to_path_buf()),
        }
    }
    pub fn render(
        &self,
        parser: &liquid::Parser,
        objects_map: &ObjectMap,
        definitions: &ObjectDefinitions,
        field_config: &FieldConfig,
    ) -> Result<String, Box<dyn Error>> {
        #[cfg(feature = "verbose-logging")]
        tracing::debug!("rendering {}", self.name);
        let mut globals = liquid::object!({ "page": self.name });
        let mut objects = liquid::object!({});
        for (name, obj_entry) in objects_map {
            let definition = definitions
                .get(name)
                .unwrap_or_else(|| panic!("missing object definition {}", name));
            let values = match obj_entry {
                ObjectEntry::List(l) => {
                    Value::array(l.iter().map(|o| o.liquid_object(definition, field_config)))
                }
                ObjectEntry::Object(o) => o.liquid_object(definition, field_config),
            };
            objects.insert(name.into(), values.clone());
            globals.insert(
                pluralize(name, if obj_entry.is_list() { 2 } else { 1 }, false).into(),
                values,
            );
            self.globals.inject(&mut globals);
        }
        globals.insert("objects".into(), objects.into());
        if let Some(template_info) = &self.template {
            let template = parser.parse(&template_info.content)?;
            let mut object_vals = match template_info
                .object
                .liquid_object(template_info.definition, field_config)
            {
                liquid::model::Value::Object(v) => Ok(v),
                _ => Err(InvalidPageError),
            }?;
            object_vals.extend(liquid::object!({
                "object_name": template_info.object.object_name,
                "order": template_info.object.order,
                "path": template_info.object.path,
            }));
            let mut context = globals.clone();
            context.extend(liquid::object!({
              template_info.definition.name.to_owned(): object_vals
            }));
            template
                .render(&context)
                .and_then(|rendered| {
                    // We now have a rendered string, but because of possible variables
                    // in markdown, we actually need to render one more time
                    parser.parse(&rendered)?.render(&context)
                })
                .map_err(|error| {
                    error
                        .trace(format!("{}", template_info.debug_path.to_string_lossy()))
                        .trace(format!("context (template):{}", debug_context(&context, 0)))
                        .into()
                })
        } else if let Some(content) = &self.content {
            let template = parser.parse(content)?;
            template
                .render(&globals)
                .and_then(|rendered| {
                    // We now have a rendered string, but because of possible variables
                    // in markdown, we actually need to render one more time
                    parser.parse(&rendered)?.render(&globals)
                })
                .map_err(|error| {
                    error
                        .trace(format!(
                            "{}",
                            self.debug_path.as_ref().unwrap().to_string_lossy()
                        ))
                        .trace(format!("context (page):{}", debug_context(&globals, 0)))
                        .into()
                })
        } else {
            panic!("Pages must have either a template or a path");
        }
    }

    pub fn extension(&self) -> &str {
        self.file_type.extension()
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        fields::{
            meta::{Meta, MetaMap},
            DateTime, FieldType, FieldValue, MetaValue, ObjectValues,
        },
        liquid_parser,
        object_definition::FieldsMap,
        MemoryFileSystem, ObjectMap,
    };

    use super::*;

    fn content_markdown() -> String {
        "## hello from markdown!

here is some unescaped html: <br/>

here is a liquid variable: {{site_url}}
        "
        .to_string()
    }

    fn get_objects_map() -> ObjectMap {
        let tour_dates_objects = vec![ObjectValues::from([
            (
                "date".to_string(),
                FieldValue::Date(DateTime::from("12/22/2022 00:00:00").unwrap()),
            ),
            (
                "ticket_link".to_string(),
                FieldValue::String("foo.com".to_string()),
            ),
        ])];
        let numbers_objects = vec![ObjectValues::from([(
            "number".to_string(),
            FieldValue::Number(2.57),
        )])];
        let artist_values = ObjectValues::from([
            (
                "name".to_string(),
                FieldValue::String("Tormenta Rey".to_string()),
            ),
            (
                "meta".to_string(),
                FieldValue::Meta(Meta(MetaMap::from([
                    ("number".to_string(), MetaValue::Number(42.26)),
                    (
                        "deep".to_string(),
                        MetaValue::Map(Meta(MetaMap::from([(
                            "deep".to_string(),
                            MetaValue::Array(vec![MetaValue::String("HELLO!".to_string())]),
                        )]))),
                    ),
                ]))),
            ),
            ("numbers".to_string(), FieldValue::Objects(numbers_objects)),
            (
                "tour_dates".to_string(),
                FieldValue::Objects(tour_dates_objects),
            ),
        ]);
        let artist = Object {
            filename: "tormenta-rey".to_string(),
            object_name: "artist".to_string(),
            path: "artist/tormenta-rey".to_string(),
            order: None,
            values: artist_values,
        };
        let links_objects = vec![ObjectValues::from([(
            "url".to_string(),
            FieldValue::String("foo.com".to_string()),
        )])];
        let c_values = ObjectValues::from([
            (
                "content".to_string(),
                FieldValue::Markdown(content_markdown()),
            ),
            ("name".to_string(), FieldValue::String("home".to_string())),
            ("links".to_string(), FieldValue::Objects(links_objects)),
        ]);

        let c = Object {
            filename: "home".to_string(),
            object_name: "c".to_string(),
            path: "home".to_string(),
            order: None,
            values: c_values,
        };

        ObjectMap::from([
            (
                "artist".to_string(),
                ObjectEntry::from_vec(vec![artist.clone(), artist]),
            ),
            ("c".to_string(), ObjectEntry::from_vec(vec![c])),
        ])
    }

    fn artist_definition() -> ObjectDefinition {
        let artist_def_fields = FieldsMap::from([("name".to_string(), FieldType::String)]);
        let tour_dates_fields = FieldsMap::from([
            ("date".to_string(), FieldType::Date),
            ("ticket_link".to_string(), FieldType::String),
        ]);
        let numbers_fields = FieldsMap::from([("number".to_string(), FieldType::Number)]);
        let artist_children = ObjectDefinitions::from([
            (
                "tour_dates".to_string(),
                ObjectDefinition {
                    name: "tour_dates".to_string(),
                    fields: tour_dates_fields,
                    template: None,
                    children: ObjectDefinitions::new(),
                },
            ),
            (
                "numbers".to_string(),
                ObjectDefinition {
                    name: "numbers".to_string(),
                    fields: numbers_fields,
                    template: None,
                    children: ObjectDefinitions::new(),
                },
            ),
        ]);
        ObjectDefinition {
            name: "artist".to_string(),
            fields: artist_def_fields,
            template: Some("artist".to_string()),
            children: artist_children,
        }
    }

    fn get_definition_map() -> ObjectDefinitions {
        ObjectDefinitions::from([
            ("artist".to_string(), artist_definition()),
            (
                "c".to_string(),
                ObjectDefinition {
                    name: "c".to_string(),
                    fields: FieldsMap::from([
                        ("name".to_string(), FieldType::String),
                        ("content".to_string(), FieldType::Markdown),
                    ]),
                    template: None,
                    children: ObjectDefinitions::from([(
                        "links".to_string(),
                        ObjectDefinition {
                            name: "links".to_string(),
                            fields: FieldsMap::from([("url".to_string(), FieldType::String)]),
                            template: None,
                            children: ObjectDefinitions::new(),
                        },
                    )]),
                },
            ),
        ])
    }

    fn page_content() -> &'static str {
        "{% assign c = objects.c | where: \"name\", \"home\" | first %}
        url: {{site_url}}
        name: {{c.name}}
        content: {{c.content}}
        page_path: {{c.path}}
        {% for link in c.links %}
          link: {{link.url}}
        {% endfor %}
        {% for artist in artists %}
          artist: {{artist.name}}
          path: {{artist.path}}
        {% endfor %}
        "
    }
    fn artist_template_content() -> &'static str {
        "name: {{artist.name}}
        metanum: {{artist.meta.number}}
        deep: {{artist.meta.deep.deep | first}}
        {% for number in artist.numbers %}
          number: {{number.number}}
        {% endfor %}
        {% for date in artist.tour_dates %}
          date: {{date.date | date: \"%b %d, %y\"}}
          link: {{date.ticket_link}}
        {% endfor %}"
    }

    #[test]
    fn regular_page() -> Result<(), Box<dyn Error>> {
        let liquid_parser = liquid_parser::get(None, None, &MemoryFileSystem::default())?;
        let globals = RenderGlobals {
            site_url: "https://foo.bar".into(),
        };
        let field_config = FieldConfig {
            uploads_url: "https://uploads.foo.bar".into(),
            upload_prefix: "something/".into(),
        };
        let objects_map = get_objects_map();
        let definition_map = get_definition_map();
        let page = Page::new(
            "home".to_string(),
            page_content().to_string(),
            TemplateType::Default,
            &globals,
            Path::new("objects/home.toml"),
        );
        let rendered = page.render(&liquid_parser, &objects_map, &definition_map, &field_config)?;
        println!("rendered: {}", rendered);
        assert!(rendered.contains("name: home"), "filtered object");
        assert!(
            rendered.contains(r##"content: <h2><a href="#hello-from-markdown" aria-hidden="true" class="anchor" id="hello-from-markdown"></a>hello from markdown!</h2>"##),
            "markdown is parsed into html correctly"
        );
        assert!(rendered.contains("link: foo.com"), "child string field");
        assert!(
            rendered.contains("artist: Tormenta Rey"),
            "item from objects"
        );
        assert!(rendered.contains("page_path: home"), "path is defined");
        assert!(
            rendered.contains("path: artist/tormenta-rey"),
            "items define paths"
        );
        assert!(
            rendered.contains("url: https://foo.bar"),
            "site_url is defined"
        );
        assert!(
            rendered.contains("here is some unescaped html: <br/>"),
            "html in markdown is not escaped"
        );
        assert!(
            rendered.contains("here is a liquid variable: https://foo.bar"),
            "liquid in markdown is parsed"
        );
        Ok(())
    }
    #[test]
    fn template_page() -> Result<(), Box<dyn Error>> {
        let globals = RenderGlobals {
            site_url: "https://foo.bar".into(),
        };
        let field_config = FieldConfig {
            uploads_url: "https://uploads.foo.bar".into(),
            upload_prefix: "something/".into(),
        };
        let definition_map = get_definition_map();
        let liquid_parser = liquid_parser::get(None, None, &MemoryFileSystem::default())?;
        let objects_map = get_objects_map();
        let object = objects_map["artist"].into_iter().next().unwrap();
        println!("OBJ: {:#?}", object);
        let artist_def = artist_definition();
        let page = Page::new_with_template(
            "tormenta-rey".to_string(),
            &artist_def,
            object,
            artist_template_content().to_string(),
            TemplateType::Default,
            &globals,
            Path::new("objects/template.toml"),
        );
        let rendered = page.render(&liquid_parser, &objects_map, &definition_map, &field_config)?;
        println!("rendered: {}", rendered);
        assert!(rendered.contains("name: Tormenta Rey"), "root field");
        assert!(
            rendered.contains("metanum: 42.26"),
            "child meta number field"
        );
        assert!(rendered.contains("number: 2.57"), "child number field");
        assert!(rendered.contains("deep: HELLO!"), "child deep field");
        assert!(rendered.contains("date: Dec 22, 22"), "child date field");
        assert!(rendered.contains("link: foo.com"), "child string field");
        Ok(())
    }
}
