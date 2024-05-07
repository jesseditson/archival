use liquid_core::error::ResultLiquidExt;
use liquid_core::model::KString;
use liquid_core::Expression;
use liquid_core::Language;
use liquid_core::Renderable;
use liquid_core::ValueView;
use liquid_core::{runtime::StackFrame, Runtime};
use liquid_core::{Error, Result};
use liquid_core::{ParseTag, TagReflection, TagTokenIter};
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::Write;

#[derive(Copy, Clone, Debug, Default)]
pub struct LayoutTag;

impl LayoutTag {}

impl TagReflection for LayoutTag {
    fn tag(&self) -> &'static str {
        "layout"
    }

    fn description(&self) -> &'static str {
        "Renders a layout with the current template as the content"
    }
}

impl ParseTag for LayoutTag {
    fn parse(
        &self,
        mut arguments: TagTokenIter<'_>,
        _options: &Language,
    ) -> Result<Box<dyn Renderable>> {
        let partial = arguments.expect_next("Identifier or literal expected.")?;

        let partial = partial.expect_value().into_result()?;

        let mut vars: Vec<(KString, Expression)> = Vec::new();
        while let Ok(next) = arguments.expect_next("") {
            let id = next.expect_identifier().into_result()?.to_string();

            arguments
                .expect_next("\":\" expected.")?
                .expect_str(":")
                .into_result_custom_msg("expected \":\" to be used for the assignment")?;

            vars.push((
                id.into(),
                arguments
                    .expect_next("expected value")?
                    .expect_value()
                    .into_result()?,
            ));

            if let Ok(comma) = arguments.expect_next("") {
                // stop looking for variables if there is no comma
                // currently allows for one trailing comma
                if comma.expect_str(",").into_result().is_err() {
                    break;
                }
            }
        }

        arguments.expect_nothing()?;

        Ok(Box::new(Layout { partial, vars }))
    }

    fn reflection(&self) -> &dyn TagReflection {
        self
    }
}

#[derive(Debug)]
struct Layout {
    partial: Expression,
    vars: Vec<(KString, Expression)>,
}

static CONTENT_SENTINEL: &str = "___LAYOUT_CONTENT___";
static SNIP_SENTINEL: &str = "___LAYOUT_SNIP___";

impl Renderable for Layout {
    fn render_to(&self, writer: &mut dyn Write, runtime: &dyn Runtime) -> Result<()> {
        runtime.set_global(
            "page_content".into(),
            liquid_core::Value::Scalar(CONTENT_SENTINEL.into()),
        );
        let value = self.partial.evaluate(runtime)?;
        if !value.is_scalar() {
            return Error::with_msg("Can only use `layout` with strings")
                .context("partial", format!("{}", value.source()))
                .into_err();
        }
        let name = value.to_kstr().into_owned();

        {
            // if there our additional variables creates a layout object to access all the variables
            // from e.g. { layout "image.html" "path" => "foo.png" }
            // then in image.html you could have <img src="{{layout.path}}" />
            let mut pass_through = std::collections::HashMap::new();
            if !self.vars.is_empty() {
                for (id, val) in &self.vars {
                    let value = val
                        .try_evaluate(runtime)
                        .ok_or_else(|| Error::with_msg("failed to evaluate value"))?;

                    pass_through.insert(id.as_ref(), value);
                }
            }

            let scope = StackFrame::new(runtime, &pass_through);
            let partial = scope
                .partials()
                .get(&name)
                .trace_with(|| format!("{{% layout {} %}}", self.partial).into())?;

            partial
                .render_to(writer, &scope)
                .trace_with(|| format!("{{% layout {} %}}", self.partial).into())
                .context_key_with(|| self.partial.to_string().into())
                .value_with(|| name.to_string().into())?;

            writer.write_all(SNIP_SENTINEL.as_bytes()).unwrap();
        }

        Ok(())
    }
}

static SNIP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!("{}(?<content>[\\s\\S]*)", SNIP_SENTINEL)).unwrap());
pub fn post_process(mut rendered: String) -> String {
    if let Some(captured) = SNIP_RE.captures(&rendered) {
        rendered = rendered.replace(CONTENT_SENTINEL, &captured["content"]);
    }
    SNIP_RE.replace(&rendered, "").into()
}

#[cfg(test)]
mod test {
    use std::borrow;
    use std::error::Error;

    use liquid_core::partials::PartialCompiler;
    use liquid_core::runtime::RuntimeBuilder;
    use liquid_core::Value;
    use liquid_core::{parser, partials, runtime, Language, Template};
    use liquid_lib::stdlib;

    use super::*;

    pub trait ToTemplate {
        fn to_template(&self, options: &Language) -> Result<Template, Box<dyn Error>>;
    }
    impl ToTemplate for &str {
        fn to_template(&self, options: &Language) -> Result<Template, Box<dyn Error>> {
            Ok(parser::parse(self, options).map(runtime::Template::new)?)
        }
    }

    fn options() -> Language {
        let mut options = Language::default();
        options
            .tags
            .register("layout".to_string(), LayoutTag.into());
        options
            .blocks
            .register("comment".to_string(), stdlib::CommentBlock.into());
        options
            .blocks
            .register("if".to_string(), stdlib::IfBlock.into());
        options
    }

    #[derive(Default, Debug, Clone, Copy)]
    struct TestSource;

    impl partials::PartialSource for TestSource {
        fn contains(&self, _name: &str) -> bool {
            true
        }

        fn names(&self) -> Vec<&str> {
            vec![]
        }

        fn try_get<'a>(&'a self, name: &str) -> Option<borrow::Cow<'a, str>> {
            match name {
                "example.liquid" => Some(r#"{{'whooo'}}{%comment%}What happens{%endcomment%} {%if num < numTwo%}wat{%else%}wot{%endif%} {%if num > numTwo%}wat{%else%}wot{%endif%}{{ page_content }}"#.into()),
                "example_var.liquid" => Some(r#"{{example_var}}{{ page_content }}"#.into()),
                "example_multi_var.liquid" => Some(r#"{{example_var}} {{example}}{{ page_content }}"#.into()),
                _ => None
            }
        }
    }

    #[test]
    fn layout_tag_quotes() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template = "{% layout 'example.liquid' %}\ntest test".to_template(&options)?;

        let partials = partials::OnDemandCompiler::<TestSource>::empty()
            .compile(::std::sync::Arc::new(options))
            .unwrap();
        let runtime = RuntimeBuilder::new()
            .set_partials(partials.as_ref())
            .build();
        runtime.set_global("num".into(), Value::scalar(5f64));
        runtime.set_global("numTwo".into(), Value::scalar(10f64));
        let output = post_process(template.render(&runtime).unwrap());
        assert_eq!(output, "whooo wat wot\ntest test");
        Ok(())
    }

    #[test]
    fn layout_variable() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template =
            "{% layout 'example_var.liquid' example_var:\"hello\" %}".to_template(&options)?;

        let partials = partials::OnDemandCompiler::<TestSource>::empty()
            .compile(::std::sync::Arc::new(options))
            .unwrap();
        let runtime = RuntimeBuilder::new()
            .set_partials(partials.as_ref())
            .build();
        let output = post_process(template.render(&runtime).unwrap());
        assert_eq!(output, "hello");
        Ok(())
    }

    #[test]
    fn layout_multiple_variables() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template =
            "{% layout 'example_multi_var.liquid' example_var:\"hello\", example:\"world\" %}"
                .to_template(&options)?;

        let partials = partials::OnDemandCompiler::<TestSource>::empty()
            .compile(::std::sync::Arc::new(options))
            .unwrap();
        let runtime = RuntimeBuilder::new()
            .set_partials(partials.as_ref())
            .build();
        let output = post_process(template.render(&runtime).unwrap());
        assert_eq!(output, "hello world");
        Ok(())
    }

    #[test]
    fn layout_multiple_variables_trailing_comma() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template =
            "{% layout 'example_multi_var.liquid' example_var:\"hello\", example:\"dogs\", %}"
                .to_template(&options)?;

        let partials = partials::OnDemandCompiler::<TestSource>::empty()
            .compile(::std::sync::Arc::new(options))
            .unwrap();
        let runtime = RuntimeBuilder::new()
            .set_partials(partials.as_ref())
            .build();
        let output = post_process(template.render(&runtime).unwrap());
        assert_eq!(output, "hello dogs");
        Ok(())
    }

    #[test]
    fn no_file() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template = "{% layout 'file_does_not_exist.liquid' %}".to_template(&options)?;

        let partials = partials::OnDemandCompiler::<TestSource>::empty()
            .compile(::std::sync::Arc::new(options))
            .unwrap();
        let runtime = RuntimeBuilder::new()
            .set_partials(partials.as_ref())
            .build();
        runtime.set_global("num".into(), Value::scalar(5f64));
        runtime.set_global("numTwo".into(), Value::scalar(10f64));
        let output = template.render(&runtime);
        assert!(output.is_err());
        Ok(())
    }
}
