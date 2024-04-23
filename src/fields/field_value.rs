use super::file::File;
use super::DateTime;
use super::{FieldType, InvalidFieldError};
use comrak::{markdown_to_html, ComrakOptions};
use liquid::{model, ValueView};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
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

pub type ObjectValues = HashMap<String, FieldValue>;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum FieldValue {
    String(String),
    Markdown(String),
    Number(f64),
    Date(DateTime),
    #[cfg_attr(feature = "typescript", serde(skip))]
    Objects(Vec<ObjectValues>),
    Boolean(bool),
    File(File),
}
fn err(f_type: &FieldType, value: String) -> FieldValueError {
    FieldValueError::InvalidValue(f_type.to_string(), value.to_owned())
}

impl FieldValue {
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
            FieldType::Date => Self::Date(DateTime::from_toml(
                t_val.as_datetime().ok_or_else(|| err(f_type, value))?,
            )?),
            FieldType::Image => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::image().fill_from_map(f_info))
            }
            FieldType::Video => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::video().fill_from_map(f_info))
            }
            FieldType::Audio => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::audio().fill_from_map(f_info))
            }
            FieldType::Upload => {
                let f_info = t_val.as_table().ok_or_else(|| err(f_type, value))?;
                Self::File(File::download().fill_from_map(f_info))
            }
        })
    }

    #[cfg(test)]
    pub fn liquid_date(&self) -> model::DateTime {
        match self {
            FieldValue::Date(d) => d.as_datetime(),
            _ => panic!("Not a date"),
        }
    }
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

impl From<&FieldType> for FieldValue {
    fn from(f_type: &FieldType) -> Self {
        f_type.default_value()
    }
}

impl From<&FieldValue> for toml::Value {
    fn from(value: &FieldValue) -> Self {
        match value {
            FieldValue::String(v) => Self::String(v.to_owned()),
            FieldValue::Markdown(v) => Self::String(v.to_owned()),
            FieldValue::Number(n) => Self::Float(*n),
            FieldValue::Date(d) => {
                let d = d.as_datetime();
                Self::Datetime(toml_datetime::Datetime {
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
                })
            }
            FieldValue::Boolean(v) => Self::Boolean(v.to_owned()),
            FieldValue::Objects(o) => Self::Array(
                o.iter()
                    .map(|child| {
                        let mut vals: toml::map::Map<String, Value> = toml::map::Map::new();
                        for (key, cv) in child {
                            vals.insert(key.to_string(), cv.into());
                        }
                        Self::Table(vals)
                    })
                    .collect(),
            ),
            FieldValue::File(f) => Self::Table(f.to_toml()),
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
            FieldValue::Markdown(_) => "markdown",
            FieldValue::Number(_) => "number",
            FieldValue::Date(_) => "date",
            FieldValue::Objects(_) => "objects",
            FieldValue::Boolean(_) => "boolean",
            FieldValue::File(_) => "file",
        }
    }
    /// Interpret as a string.
    fn to_kstr(&self) -> model::KStringCow<'_> {
        model::KStringCow::from(self.as_string())
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
            FieldValue::Number(n) => Some(model::ScalarCow::new(*n)),
            // TODO: should be able to return a datetime value here
            FieldValue::Date(d) => Some(model::ScalarCow::new((*d).as_datetime())),
            FieldValue::Markdown(s) => Some(model::ScalarCow::new(markdown_to_html(
                s,
                &ComrakOptions::default(),
            ))),
            FieldValue::Boolean(b) => Some(model::ScalarCow::new(*b)),
            FieldValue::Objects(_) => None,
            FieldValue::File(_f) => None,
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
            FieldValue::File(f) => Some(f),
            _ => None,
        }
    }

    fn to_value(&self) -> liquid::model::Value {
        match self {
            FieldValue::String(_) => self.as_scalar().to_value(),
            FieldValue::Markdown(_) => self.as_scalar().to_value(),
            FieldValue::Number(_) => self.as_scalar().to_value(),
            FieldValue::Date(_) => self.as_scalar().to_value(),
            FieldValue::Boolean(_) => self.as_scalar().to_value(),
            FieldValue::Objects(_) => self.as_array().to_value(),
            FieldValue::File(_) => self.as_object().to_value(),
        }
    }
}

impl FieldValue {
    pub fn from_string(
        key: &String,
        field_type: &FieldType,
        value: String,
    ) -> Result<FieldValue, InvalidFieldError> {
        match field_type {
            FieldType::String => Ok(FieldValue::String(value)),
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
                File::audio().fill_from_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?),
            )),
            FieldType::Video => Ok(FieldValue::File(
                File::video().fill_from_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?),
            )),
            FieldType::Upload => Ok(FieldValue::File(
                File::download().fill_from_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?),
            )),
            FieldType::Image => Ok(FieldValue::File(
                File::image().fill_from_map(value.as_table().ok_or_else(|| {
                    InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    }
                })?),
            )),
        }
    }

    fn as_string(&self) -> String {
        match self {
            FieldValue::String(s) => s.clone(),
            FieldValue::Markdown(n) => n.clone(),
            FieldValue::Number(n) => n.to_string(),
            FieldValue::Date(d) => d.as_datetime().to_rfc2822(),
            FieldValue::Boolean(b) => b.to_string(),
            FieldValue::Objects(o) => format!("{:?}", o),
            FieldValue::File(f) => format!("{:?}", f.to_map(true)),
        }
    }
}

pub fn def_to_values(def: &HashMap<String, FieldType>) -> HashMap<String, FieldValue> {
    let mut vals = HashMap::new();
    for (key, f_type) in def {
        vals.insert(key.to_string(), FieldValue::from(f_type));
    }
    vals
}
