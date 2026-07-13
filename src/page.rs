use crate::{
    object::{Object, ObjectEntry},
    object_definition::ObjectDefinition,
    FieldConfig, ObjectDefinitions, ObjectMap,
};
use anyhow::Result;
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

pub struct PageTemplate<'a> {
    pub definition: &'a ObjectDefinition,
    pub object: &'a Object,
    pub content: String,
    /// A pre-parsed template. When set, `content` is not parsed again; this
    /// lets builds parse each template file once and share it across every
    /// object rendered with it.
    pub parsed: Option<&'a liquid::Template>,
    #[allow(dead_code)]
    pub file_type: TemplateType,
    pub debug_path: PathBuf,
}

impl fmt::Debug for PageTemplate<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PageTemplate")
            .field("definition", &self.definition)
            .field("object", &self.object)
            .field("content", &self.content)
            .field("parsed", &self.parsed.map(|_| "<template>"))
            .field("file_type", &self.file_type)
            .field("debug_path", &self.debug_path)
            .finish()
    }
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
    pub debug_path: Option<PathBuf>,
}

/// Convert all objects into the liquid context shared by every page in a
/// build. This is by far the most expensive part of setting up a render
/// (markdown fields are converted to html here), so builds create it once and
/// share it across pages via [`LayeredContext`].
pub fn build_context(
    objects_map: &ObjectMap,
    definitions: &ObjectDefinitions,
    field_config: &FieldConfig,
    globals: &RenderGlobals,
) -> liquid::Object {
    let _span = tracing::trace_span!("build_context").entered();
    let mut context = liquid::Object::new();
    let mut objects = liquid::Object::new();
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
        context.insert(
            pluralize(name, if obj_entry.is_list() { 2 } else { 1 }, false).into(),
            values,
        );
    }
    globals.inject(&mut context);
    context.insert("objects".into(), objects.into());
    context
}

/// A render context composed of a small per-page overlay on top of the shared
/// per-build context. Lookups check the overlay first, so pages can shadow
/// shared keys without deep-cloning the (large) shared context for every page.
#[derive(Debug)]
struct LayeredContext<'a> {
    overlay: &'a liquid::Object,
    base: &'a liquid::Object,
}

impl LayeredContext<'_> {
    fn merged(&self) -> liquid::Object {
        let mut merged = self.base.clone();
        merged.extend(self.overlay.iter().map(|(k, v)| (k.clone(), v.clone())));
        merged
    }
}

impl fmt::Display for LayeredContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (k, v) in liquid::ObjectView::iter(self) {
            write!(f, "{}: {} ", k, v.render())?;
        }
        Ok(())
    }
}

impl ValueView for LayeredContext<'_> {
    fn as_debug(&self) -> &dyn fmt::Debug {
        self
    }
    fn render(&self) -> liquid::model::DisplayCow<'_> {
        liquid::model::DisplayCow::Owned(Box::new(self))
    }
    fn source(&self) -> liquid::model::DisplayCow<'_> {
        liquid::model::DisplayCow::Owned(Box::new(self))
    }
    fn type_name(&self) -> &'static str {
        "object"
    }
    fn query_state(&self, state: liquid::model::State) -> bool {
        match state {
            liquid::model::State::Truthy => true,
            liquid::model::State::DefaultValue
            | liquid::model::State::Empty
            | liquid::model::State::Blank => liquid::ObjectView::size(self) == 0,
        }
    }
    fn to_kstr(&self) -> liquid::model::KStringCow<'_> {
        liquid::model::KStringCow::from_string(self.to_string())
    }
    fn to_value(&self) -> Value {
        Value::Object(self.merged())
    }
    fn as_object(&self) -> Option<&dyn liquid::ObjectView> {
        Some(self)
    }
}

impl liquid::ObjectView for LayeredContext<'_> {
    fn as_value(&self) -> &dyn ValueView {
        self
    }
    fn size(&self) -> i64 {
        liquid::ObjectView::keys(self).count() as i64
    }
    fn keys<'k>(&'k self) -> Box<dyn Iterator<Item = liquid::model::KStringCow<'k>> + 'k> {
        Box::new(
            self.overlay
                .keys()
                .chain(
                    self.base
                        .keys()
                        .filter(|k| !self.overlay.contains_key(k.as_str())),
                )
                .map(|k| k.as_ref().into()),
        )
    }
    fn values<'k>(&'k self) -> Box<dyn Iterator<Item = &'k dyn ValueView> + 'k> {
        Box::new(liquid::ObjectView::iter(self).map(|(_, v)| v))
    }
    fn iter<'k>(
        &'k self,
    ) -> Box<dyn Iterator<Item = (liquid::model::KStringCow<'k>, &'k dyn ValueView)> + 'k> {
        Box::new(
            self.overlay
                .iter()
                .chain(
                    self.base
                        .iter()
                        .filter(|(k, _)| !self.overlay.contains_key(k.as_str())),
                )
                .map(|(k, v)| (k.as_ref().into(), v.as_view())),
        )
    }
    fn contains_key(&self, index: &str) -> bool {
        self.overlay.contains_key(index) || self.base.contains_key(index)
    }
    fn get<'s>(&'s self, index: &str) -> Option<&'s dyn ValueView> {
        self.overlay
            .get(index)
            .or_else(|| self.base.get(index))
            .map(|v| v.as_view())
    }
}

/// Rendered output can only require a second render pass if it still contains
/// liquid syntax (variables or tags embedded in markdown fields). Parsing is
/// by far the most expensive part of rendering, so pages that render to plain
/// output skip the second pass entirely.
fn may_contain_liquid(rendered: &str) -> bool {
    rendered.contains("{{") || rendered.contains("{%")
}

/// Render `template`, then re-parse and re-render the output if (and only if)
/// it may still contain liquid syntax introduced by rendered field values.
fn render_passes(
    template: &liquid::Template,
    parser: &liquid::Parser,
    context: &LayeredContext,
) -> Result<String, liquid_core::Error> {
    let first_span = tracing::trace_span!("first_render").entered();
    let rendered = template.render(context)?;
    drop(first_span);
    if !may_contain_liquid(&rendered) {
        return Ok(rendered);
    }
    let parse_span = tracing::trace_span!("second_parse").entered();
    let reparsed = parser.parse(&rendered)?;
    drop(parse_span);
    let _second_span = tracing::trace_span!("second_render").entered();
    reparsed.render(context)
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
    // Builds use `new_with_parsed_template` to share one parsed template
    // across objects; this un-parsed variant is kept for callers (and tests)
    // that render a single page.
    #[allow(dead_code)]
    pub fn new_with_template(
        name: String,
        definition: &'a ObjectDefinition,
        object: &'a Object,
        content: String,
        file_type: TemplateType,
        template_debug_path: &Path,
    ) -> Page<'a> {
        Page {
            name,
            content: None,
            template: Some(PageTemplate {
                definition,
                object,
                content,
                parsed: None,
                file_type: file_type.clone(),
                debug_path: template_debug_path.to_path_buf(),
            }),
            file_type,
            debug_path: None,
        }
    }
    pub fn new_with_parsed_template(
        name: String,
        definition: &'a ObjectDefinition,
        object: &'a Object,
        parsed: &'a liquid::Template,
        file_type: TemplateType,
        template_debug_path: &Path,
    ) -> Page<'a> {
        Page {
            name,
            content: None,
            template: Some(PageTemplate {
                definition,
                object,
                content: String::new(),
                parsed: Some(parsed),
                file_type: file_type.clone(),
                debug_path: template_debug_path.to_path_buf(),
            }),
            file_type,
            debug_path: None,
        }
    }
    pub fn new(
        name: String,
        content: String,
        file_type: TemplateType,
        debug_path: &Path,
    ) -> Page<'a> {
        Page {
            name,
            content: Some(content),
            template: None,
            file_type,
            debug_path: Some(debug_path.to_path_buf()),
        }
    }
    pub fn render(
        &self,
        parser: &liquid::Parser,
        base_context: &liquid::Object,
        field_config: &FieldConfig,
    ) -> Result<String> {
        #[cfg(feature = "verbose-logging")]
        tracing::debug!("rendering {}", self.name);
        let mut overlay = liquid::object!({ "page": self.name });
        if let Some(template_info) = &self.template {
            let parsed;
            let template = match template_info.parsed {
                Some(t) => t,
                None => {
                    let _span = tracing::trace_span!("parse_template").entered();
                    parsed = parser.parse(&template_info.content)?;
                    &parsed
                }
            };
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
                "path": template_info.object.path(),
            }));
            overlay.insert(
                template_info.definition.name.to_owned().into(),
                Value::Object(object_vals),
            );
            let context = LayeredContext {
                overlay: &overlay,
                base: base_context,
            };
            render_passes(template, parser, &context).map_err(|error| {
                error
                    .trace(format!("{}", template_info.debug_path.to_string_lossy()))
                    .trace(format!(
                        "context (template):{}",
                        debug_context(&context.merged(), 0)
                    ))
                    .into()
            })
        } else if let Some(content) = &self.content {
            let parse_span = tracing::trace_span!("parse_template").entered();
            let template = parser.parse(content)?;
            drop(parse_span);
            let context = LayeredContext {
                overlay: &overlay,
                base: base_context,
            };
            render_passes(&template, parser, &context).map_err(|error| {
                error
                    .trace(format!(
                        "{}",
                        self.debug_path.as_ref().unwrap().to_string_lossy()
                    ))
                    .trace(format!(
                        "context (page):{}",
                        debug_context(&context.merged(), 0)
                    ))
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
    fn regular_page() -> Result<()> {
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
        let base_context = build_context(&objects_map, &definition_map, &field_config, &globals);
        let page = Page::new(
            "home".to_string(),
            page_content().to_string(),
            TemplateType::Default,
            Path::new("objects/home.toml"),
        );
        let rendered = page.render(&liquid_parser, &base_context, &field_config)?;
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
        assert!(rendered.contains("page_path: c/home"), "path is defined");
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
    fn template_page() -> Result<()> {
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
        let base_context = build_context(&objects_map, &definition_map, &field_config, &globals);
        let page = Page::new_with_template(
            "tormenta-rey".to_string(),
            &artist_def,
            object,
            artist_template_content().to_string(),
            TemplateType::Default,
            Path::new("objects/template.toml"),
        );
        let rendered = page.render(&liquid_parser, &base_context, &field_config)?;
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
