use crate::object::to_liquid::object_to_liquid;
use crate::util::integer_decode;
use crate::{FieldConfig, ObjectDefinition};

use super::file::File;
use super::meta::Meta;
use super::DateTime;
use super::{FieldType, InvalidFieldError};
use comrak::{markdown_to_html, ComrakOptions};
use liquid::{model, ValueView};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::hash::Hash;
use std::{
    error::Error,
    fmt::{self, Debug},
};
use thiserror::Error;
use toml::Value;
use tracing::instrument;

#[derive(Debug, Error)]
pub enum FieldValueError {
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

pub type ObjectValues = BTreeMap<String, FieldValue>;

#[cfg(feature = "typescript")]
mod typedefs {
    use typescript_type_def::{
        type_expr::{Ident, NativeTypeInfo, TypeExpr, TypeInfo},
        TypeDef,
    };
    pub struct ObjectValuesTypeDef;
    impl TypeDef for ObjectValuesTypeDef {
        const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
            r#ref: TypeExpr::ident(Ident("Record<string, FieldValue>[]")),
        });
    }
}

macro_rules! compare_values {
    ($left:ident, $right:ident, $($t:path),*) => {
        match $left {
            $($t(lv) => {
                if let $t(rv) = $right {
                    lv.partial_cmp(rv)
                } else {
                    None
                }
            })*
            _ => None
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum FieldValue {
    String(String),
    Enum(String),
    Markdown(String),
    Number(f64),
    Date(DateTime),
    // Workaround for circular type: https://github.com/dbeckwith/rust-typescript-type-def/issues/18#issuecomment-2078469020
    Objects(
        #[cfg_attr(
            feature = "typescript",
            type_def(type_of = "typedefs::ObjectValuesTypeDef")
        )]
        Vec<ObjectValues>,
    ),
    Boolean(bool),
    File(File),
    Meta(Meta),
    Null,
}
fn err(f_type: &FieldType, value: String) -> FieldValueError {
    FieldValueError::InvalidValue(f_type.to_string(), value.to_owned())
}

impl Hash for FieldValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            FieldValue::Number(n) => integer_decode(*n).hash(state),
            v => v.hash(state),
        }
    }
}

pub static MARKDOWN_OPTIONS: Lazy<ComrakOptions> = Lazy::new(|| {
    let mut options = ComrakOptions::default();
    options.extension.autolink = true;
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.superscript = true;
    options.extension.description_lists = true;
    // NOTE: it's unclear how much nannying we need to do here, as users are
    // only able to update their own markdown and by definition they have access
    // to the html if they have access to the repo... however if someone is
    // tricked into pasting things into markdown they could potentially open
    // some issues?
    options.extension.tagfilter = false;
    options.extension.header_ids = Some("".to_string());
    options.extension.footnotes = true;
    options.render.unsafe_ = true;
    options
});

impl FieldValue {
    // Note that this comparison just skips fields that cannot be compared and
    // returns None.
    pub fn compare(&self, to: &FieldValue) -> Option<Ordering> {
        compare_values!(
            self,
            to,
            Self::String,
            Self::Markdown,
            Self::Number,
            Self::Date,
            Self::Boolean,
            Self::File
        )
    }
    pub fn val_with_type(f_type: &FieldType, value: String) -> Result<Self, Box<dyn Error>> {
        let t_val = toml::Value::try_from(&value)?;
        Ok(match f_type {
            FieldType::Boolean => Self::Boolean(t_val.as_bool().ok_or_else(|| err(f_type, value))?),
            FieldType::Markdown => Self::Markdown(
                t_val
                    .as_str()
                    .ok_or_else(|| err(f_type, value))?
                    .to_string(),
            ),
            FieldType::Number => Self::Number(
                t_val.as_float().unwrap_or(
                    t_val
                        .as_integer()
                        .ok_or_else(|| err(f_type, value))
                        .map(|v| v as f64)?,
                ),
            ),
            FieldType::String => Self::String(
                t_val
                    .as_str()
                    .ok_or_else(|| err(f_type, value))?
                    .to_string(),
            ),
            FieldType::Enum(_) => Self::Enum(
                t_val
                    .as_str()
                    .ok_or_else(|| err(f_type, value))?
                    .to_string(),
            ),
            FieldType::Date => Self::Date(DateTime::from_toml(
                t_val.as_datetime().ok_or_else(|| err(f_type, value))?,
            )?),
            FieldType::Image => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::image().fill_from_toml_map(f_info).unwrap())
            }
            FieldType::Video => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::video().fill_from_toml_map(f_info).unwrap())
            }
            FieldType::Audio => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::audio().fill_from_toml_map(f_info).unwrap())
            }
            FieldType::Upload => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::download().fill_from_toml_map(f_info).unwrap())
            }
            FieldType::Meta => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::Meta(Meta::from(f_info))
            }
            FieldType::Alias(a) => Self::val_with_type(&a.0, value)?,
        })
    }

    pub fn typed_objects(
        &self,
        definition: &ObjectDefinition,
        field_config: &FieldConfig,
    ) -> model::Value {
        if let FieldValue::Objects(children) = self {
            model::Value::Array(
                children
                    .iter()
                    .map(|child| {
                        model::Value::Object(object_to_liquid(child, definition, field_config))
                    })
                    .collect(),
            )
        } else {
            panic!("cannot call typed_objects on FieldValue: {:?}", self);
        }
    }

    #[cfg(test)]
    pub fn liquid_date(&self) -> model::DateTime {
        match self {
            FieldValue::Date(d) => d.as_liquid_datetime(),
            _ => panic!("Not a date"),
        }
    }
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string(Some(&FieldConfig::default())))
    }
}

impl From<&FieldValue> for Option<toml::Value> {
    fn from(value: &FieldValue) -> Self {
        match value {
            FieldValue::String(v) => Some(toml::Value::String(v.to_owned())),
            FieldValue::Enum(v) => Some(toml::Value::String(v.to_owned())),
            FieldValue::Markdown(v) => Some(toml::Value::String(v.to_owned())),
            FieldValue::Number(n) => Some(toml::Value::Float(*n)),
            FieldValue::Date(d) => {
                let d = d.as_liquid_datetime();
                Some(toml::Value::Datetime(toml_datetime::Datetime {
                    date: Some(toml_datetime::Date {
                        year: d.year() as u16,
                        month: d.month(),
                        day: d.day(),
                    }),
                    time: Some(toml_datetime::Time {
                        hour: d.hour(),
                        minute: d.minute(),
                        second: d.second(),
                        nanosecond: d.nanosecond(),
                    }),
                    offset: None,
                }))
            }
            FieldValue::Boolean(v) => Some(toml::Value::Boolean(v.to_owned())),
            FieldValue::Objects(o) => Some(toml::Value::Array(
                o.iter()
                    .map(|child| {
                        let mut vals: toml::map::Map<String, Value> = toml::map::Map::new();
                        for (key, cv) in child {
                            if let Some(val) = cv.into() {
                                vals.insert(key.to_string(), val);
                            }
                        }
                        toml::Value::Table(vals)
                    })
                    .collect(),
            )),
            FieldValue::File(f) => Some(toml::Value::Table(f.to_toml())),
            FieldValue::Meta(m) => Some(toml::Value::Table(m.to_toml())),
            FieldValue::Null => None,
        }
    }
}

impl ValueView for FieldValue {
    /// Get a `Debug` representation
    fn as_debug(&self) -> &dyn fmt::Debug {
        self
    }
    /// A `Display` for a `BoxedValue` rendered for the user.
    fn render(&self) -> model::DisplayCow<'_> {
        model::DisplayCow::Owned(Box::new(self))
    }
    /// A `Display` for a `Value` as source code.
    fn source(&self) -> model::DisplayCow<'_> {
        model::DisplayCow::Owned(Box::new(self))
    }

    /// Report the data type (generally for error reporting).
    fn type_name(&self) -> &'static str {
        match self {
            FieldValue::String(_) => "string",
            FieldValue::Enum(_) => "enum",
            FieldValue::Markdown(_) => "markdown",
            FieldValue::Number(_) => "number",
            FieldValue::Date(_) => "date",
            FieldValue::Objects(_) => "objects",
            FieldValue::Boolean(_) => "boolean",
            FieldValue::File(_) => "file",
            FieldValue::Meta(_) => "meta",
            FieldValue::Null => "null",
        }
    }
    /// Interpret as a string.
    fn to_kstr(&self) -> model::KStringCow<'_> {
        model::KStringCow::from(self.as_string(None))
    }
    /// Query the value's state
    fn query_state(&self, state: model::State) -> bool {
        match state {
            model::State::Truthy => false,
            model::State::DefaultValue => false,
            model::State::Empty => false,
            model::State::Blank => false,
        }
    }

    fn as_scalar(&self) -> Option<model::ScalarCow<'_>> {
        match self {
            FieldValue::String(s) => Some(model::ScalarCow::new(s)),
            FieldValue::Enum(s) => Some(model::ScalarCow::new(s)),
            FieldValue::Number(n) => Some(model::ScalarCow::new(*n)),
            // TODO: should be able to return a datetime value here
            FieldValue::Date(d) => Some(model::ScalarCow::new((*d).as_liquid_datetime())),
            FieldValue::Markdown(s) => Some(model::ScalarCow::new(markdown_to_html(
                s,
                &MARKDOWN_OPTIONS,
            ))),
            FieldValue::Boolean(b) => Some(model::ScalarCow::new(*b)),
            FieldValue::Objects(_) => None,
            FieldValue::File(_f) => None,
            FieldValue::Meta(_m) => None,
            FieldValue::Null => None,
        }
    }
    fn as_array(&self) -> Option<&dyn model::ArrayView> {
        match self {
            FieldValue::Objects(a) => Some(a),
            _ => None,
        }
    }
    fn as_object(&self) -> Option<&dyn model::ObjectView> {
        match self {
            FieldValue::Meta(m) => Some(m),
            _ => None,
        }
    }

    fn to_value(&self) -> liquid::model::Value {
        match self {
            FieldValue::String(_) => self.as_scalar().to_value(),
            FieldValue::Enum(_) => self.as_scalar().to_value(),
            FieldValue::Markdown(_) => self.as_scalar().to_value(),
            FieldValue::Number(_) => self.as_scalar().to_value(),
            FieldValue::Date(_) => self.as_scalar().to_value(),
            FieldValue::Boolean(_) => self.as_scalar().to_value(),
            FieldValue::Objects(_) => self.as_array().to_value(),
            FieldValue::File(_) => panic!("files cannot be rendered via value parsing"),
            FieldValue::Meta(_) => self.as_object().to_value(),
            FieldValue::Null => self.as_scalar().to_value(),
        }
    }
}

impl FieldValue {
    pub fn from_string(
        key: &String,
        field_type: &FieldType,
        value: String,
    ) -> Result<FieldValue, InvalidFieldError> {
        if value.is_empty() {
            // Defaults
            let default_val = match field_type {
                FieldType::String => Ok(FieldValue::String(value.clone())),
                FieldType::Markdown => Ok(FieldValue::Markdown(value.clone())),
                FieldType::Number => Ok(FieldValue::Number(0.0)),
                FieldType::Boolean => Ok(FieldValue::Boolean(false)),
                _ => Err(InvalidFieldError::NoDefaultForType(field_type.to_string())),
            };
            if default_val.is_ok() {
                return default_val;
            }
        }
        match field_type {
            FieldType::String => Ok(FieldValue::String(value)),
            FieldType::Enum(valid_values) => {
                if !valid_values.contains(&value) {
                    Err(InvalidFieldError::EnumMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    })?
                }
                Ok(FieldValue::Enum(value))
            }
            FieldType::Markdown => Ok(FieldValue::Markdown(value)),
            FieldType::Number => Ok(FieldValue::Number(value.parse::<f64>().map_err(|_| {
                InvalidFieldError::TypeMismatch {
                    field: key.to_owned(),
                    field_type: field_type.to_string(),
                    value,
                }
            })?)),
            FieldType::Boolean => Ok(FieldValue::Boolean(value.parse::<bool>().map_err(
                |_| InvalidFieldError::TypeMismatch {
                    field: key.to_owned(),
                    field_type: field_type.to_string(),
                    value,
                },
            )?)),
            FieldType::Date => {
                let date_str = DateTime::parse_date_string(value.to_string()).map_err(|_| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value,
                    }
                })?;
                Ok(FieldValue::Date(DateTime::from(&date_str)?))
            }
            _ => Err(InvalidFieldError::UnsupportedStringValue(
                field_type.to_string(),
            )),
        }
    }

    #[instrument(skip(value))]
    pub fn from_toml(
        key: &String,
        field_type: &FieldType,
        value: &Value,
    ) -> Result<FieldValue, Box<dyn Error>> {
        match field_type {
            FieldType::String => Ok(FieldValue::String(
                value
                    .as_str()
                    .ok_or_else(|| InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    })?
                    .to_string(),
            )),
            FieldType::Enum(valid_values) => {
                let value = value
                    .as_str()
                    .ok_or_else(|| InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    })?
                    .to_string();
                if !valid_values.contains(&value) {
                    Err(InvalidFieldError::EnumMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    })?
                } else {
                    Ok(FieldValue::Enum(value))
                }
            }
            FieldType::Markdown => Ok(FieldValue::Markdown(
                value
                    .as_str()
                    .ok_or_else(|| InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    })?
                    .to_string(),
            )),
            FieldType::Number => {
                let number = if let Some(float_val) = value.as_float() {
                    Ok(float_val)
                } else if let Some(int_val) = value.as_integer() {
                    Ok(int_val as f64)
                } else {
                    Err(InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    })
                }?;
                Ok(FieldValue::Number(number))
            }
            FieldType::Boolean => Ok(FieldValue::Boolean(value.as_bool().ok_or_else(|| {
                InvalidFieldError::TypeMismatch {
                    field: key.to_owned(),
                    field_type: field_type.to_string(),
                    value: value.to_string(),
                }
            })?)),
            FieldType::Date => {
                if let Value::Datetime(val) = value {
                    return Ok(FieldValue::Date(DateTime::from_toml(val)?));
                }
                let mut date_str =
                    (value
                        .as_str()
                        .ok_or_else(|| InvalidFieldError::TypeMismatch {
                            field: key.to_owned(),
                            field_type: field_type.to_string(),
                            value: value.to_string(),
                        })?)
                    .to_string();
                date_str = DateTime::parse_date_string(date_str)?;
                Ok(FieldValue::Date(DateTime::from(&date_str)?))
            }
            FieldType::Audio => Ok(FieldValue::File(
                File::audio().fill_from_toml_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?)?,
            )),
            FieldType::Video => Ok(FieldValue::File(
                File::video().fill_from_toml_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?)?,
            )),
            FieldType::Upload => Ok(FieldValue::File(
                File::download().fill_from_toml_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?)?,
            )),
            FieldType::Image => Ok(FieldValue::File(
                File::image().fill_from_toml_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?)?,
            )),
            FieldType::Meta => Ok(FieldValue::Meta(Meta::from(value.as_table().ok_or_else(
                || InvalidFieldError::TypeMismatch {
                    field: key.to_owned(),
                    field_type: field_type.to_string(),
                    value: value.to_string(),
                },
            )?))),
            FieldType::Alias(a) => Self::from_toml(key, &a.0, value),
        }
    }

    fn as_string(&self, config: Option<&FieldConfig>) -> String {
        match self {
            FieldValue::String(s) => s.clone(),
            FieldValue::Enum(s) => s.clone(),
            FieldValue::Markdown(n) => n.clone(),
            FieldValue::Number(n) => n.to_string(),
            FieldValue::Date(d) => d.as_liquid_datetime().to_rfc2822(),
            FieldValue::Boolean(b) => b.to_string(),
            FieldValue::Objects(o) => format!("{:?}", o),
            FieldValue::File(f) => format!(
                "{:?}",
                config
                    .map(|c| f.clone().into_map(Some(c)))
                    .expect("cannot render files without a config")
            ),
            FieldValue::Meta(m) => format!("{:?}", serde_json::Value::from(m)),
            FieldValue::Null => "null".to_string(),
        }
    }
}

impl From<&serde_json::Value> for FieldValue {
    fn from(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::String(s) => FieldValue::String(s.to_string()),
            serde_json::Value::Bool(b) => FieldValue::Boolean(*b),
            serde_json::Value::Number(n) => FieldValue::Number(n.as_f64().unwrap()),
            serde_json::Value::Null => FieldValue::String("".into()),
            serde_json::Value::Object(o) => {
                // fill_from_json_map fails when a file field is missing, so we
                // may incorrectly map to meta if the source data is not
                // structured correctly, which will likely cause a serialization
                // error downstream.
                match File::download().fill_from_json_map(o) {
                    Ok(file) => FieldValue::File(file),
                    Err(_) => FieldValue::Meta(Meta::from(o)),
                }
            }
            serde_json::Value::Array(v) => FieldValue::Objects(
                v.iter()
                    .map(|val| {
                        let mut map = BTreeMap::new();
                        if let Some(obj) = val.as_object() {
                            for (k, v) in obj.iter() {
                                map.insert(k.to_string(), FieldValue::from(v));
                            }
                        } else {
                            panic!("Invalid value {} for child", val);
                        }
                        map
                    })
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
pub mod enum_tests {

    use super::*;

    #[test]
    fn enum_value_validation_from_toml() -> Result<(), Box<dyn Error>> {
        let enum_field_type = FieldType::Enum(vec!["emo".to_string(), "metal".to_string()]);
        assert!(FieldValue::from_toml(
            &"some_key".to_string(),
            &enum_field_type,
            &Value::String("butt rock".to_string())
        )
        .is_err_and(|e| {
            let inner = e.downcast::<InvalidFieldError>().unwrap();
            matches!(
                *inner,
                InvalidFieldError::EnumMismatch {
                    field: _,
                    field_type: _,
                    value: _,
                }
            )
        }));

        Ok(())
    }
    #[test]
    fn enum_value_validation_from_string() -> Result<(), Box<dyn Error>> {
        let enum_field_type = FieldType::Enum(vec!["emo".to_string(), "metal".to_string()]);
        assert!(FieldValue::from_string(
            &"some_key".to_string(),
            &enum_field_type,
            "butt rock".to_string()
        )
        .is_err_and(|inner| {
            matches!(
                inner,
                InvalidFieldError::EnumMismatch {
                    field: _,
                    field_type: _,
                    value: _,
                }
            )
        }));

        Ok(())
    }
}

#[cfg(test)]
pub mod markdown_tests {

    use super::*;

    // We use tagfilter
    // (https://github.github.com/gfm/#disallowed-raw-html-extension-) instead
    // of fully removing or disabling html, so most things users want to do will
    // still work.
    #[test]
    fn some_html_is_allowed() {
        // tricky; indenting these will cause them to be parsed as code, which
        // will fail the test.
        let value = FieldValue::Markdown(
            "# Hello!
here is some markdown.

Within it I can add some tags like: <a href=\"https://taskmastersbirthday.com\">links</a>
"
            .to_string(),
        );

        let rendered = value.as_scalar().expect("parsing failed").into_string();

        println!("rendered: {}", rendered);

        assert!(
            rendered.contains("<a href=\"https://"),
            "links are rendered properly"
        );
    }
}
