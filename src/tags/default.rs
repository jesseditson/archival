use liquid_core::error::ResultLiquidReplaceExt;
use liquid_core::Expression;
use liquid_core::Language;
use liquid_core::Renderable;
use liquid_core::Result;
use liquid_core::Runtime;
use liquid_core::ValueView;
use liquid_core::{ParseTag, TagReflection, TagTokenIter};
use std::io::Write;

#[derive(Copy, Clone, Debug, Default)]
pub struct DefaultTag;

impl DefaultTag {}

impl TagReflection for DefaultTag {
    fn tag(&self) -> &'static str {
        "default"
    }

    fn description(&self) -> &'static str {
        "Renders a value or a default value if the value is not defined."
    }
}

impl ParseTag for DefaultTag {
    fn parse(
        &self,
        mut arguments: TagTokenIter<'_>,
        _options: &Language,
    ) -> Result<Box<dyn Renderable>> {
        let value = arguments.expect_next("Identifier or literal expected.")?;
        let default = arguments.expect_next("Identifier or literal expected.")?;

        let value = value.expect_value().into_result()?;
        let default = default.expect_value().into_result()?;

        arguments.expect_nothing()?;

        Ok(Box::new(Default { value, default }))
    }

    fn reflection(&self) -> &dyn TagReflection {
        self
    }
}

#[derive(Debug)]
struct Default {
    value: Expression,
    default: Expression,
}

impl Renderable for Default {
    fn render_to(&self, writer: &mut dyn Write, runtime: &dyn Runtime) -> Result<()> {
        let value = if let Ok(val) = self.value.evaluate(runtime) {
            val
        } else {
            self.default.evaluate(runtime)?
        };
        write!(writer, "{}", value.render()).replace("Failed to render")?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;

    use liquid::Object;
    use liquid_core::runtime::RuntimeBuilder;
    use liquid_core::Value;
    use liquid_core::{parser, runtime, Language, Template};
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
            .register("default".to_string(), DefaultTag.into());
        options
            .blocks
            .register("comment".to_string(), stdlib::CommentBlock.into());
        options
            .blocks
            .register("if".to_string(), stdlib::IfBlock.into());
        options
    }

    #[test]
    fn default_root() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template = "{% default undefined_var defined_var %}".to_template(&options)?;
        let runtime = RuntimeBuilder::new().build();
        runtime.set_global("defined_var".into(), Value::scalar("HELLO"));
        let output = template.render(&runtime)?;
        assert_eq!(output, "HELLO");
        Ok(())
    }
    #[test]
    fn string_val() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template = "{% default undefined_var \"A String\" %}".to_template(&options)?;
        let runtime = RuntimeBuilder::new().build();
        let output = template.render(&runtime)?;
        assert_eq!(output, "A String");
        Ok(())
    }
    #[test]
    fn nested_undefined_var() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template = "{% default object.undefined 28 %}".to_template(&options)?;
        let runtime = RuntimeBuilder::new().build();
        runtime.set_global("object".into(), Value::Object(Object::new()));
        let output = template.render(&runtime)?;
        assert_eq!(output, "28");
        Ok(())
    }
    #[test]
    fn default_also_undefined_errors() -> Result<(), Box<dyn Error>> {
        let options = options();
        let template = "{% default object.undefined also_not_defined %}".to_template(&options)?;
        let runtime = RuntimeBuilder::new().build();
        runtime.set_global("defined_var".into(), Value::scalar("HELLO"));
        let output = template.render(&runtime);
        assert!(output.is_err());
        Ok(())
    }
}
