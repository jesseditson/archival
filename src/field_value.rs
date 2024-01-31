use crate::{
    events::EditFieldValue,
    object_definition::{FieldType, InvalidFieldError},
};
use comrak::{markdown_to_html, ComrakOptions};
use liquid::{model, ValueView};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fmt::{self, Debug},
    ops::Deref,
};
use time::{format_description, UtcOffset};
use toml::Value;

pub type ObjectValues = HashMap<String, FieldValue>;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct DateTime {
    #[serde(skip)]
    inner: model::DateTime,
    raw: String,
}

impl DateTime {
    pub fn from(str: &str) -> Result<Self, InvalidFieldError> {
        let liquid_date = model::DateTime::from_str(&str)
            .ok_or(InvalidFieldError::InvalidDate(str.to_owned()))?;
        Ok(Self {
            inner: liquid_date,
            raw: str.to_owned(),
        })
    }
    pub fn now() -> Self {
        let inner = model::DateTime::now();
        let raw = inner.to_string();
        Self { inner, raw }
    }
    pub fn from_ymd(year: i32, month: u8, date: u8) -> Self {
        let inner = model::DateTime::from_ymd(year, month, date);
        let raw = inner.to_string();
        Self { inner, raw }
    }

    #[cfg(test)]
    pub fn as_liquid(&self) -> &model::DateTime {
        &self.inner
    }
}

impl Deref for DateTime {
    type Target = model::DateTime;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum FieldValue {
    String(String),
    Markdown(String),
    Number(f64),
    Date(DateTime),
    #[serde(skip)]
    Objects(Vec<ObjectValues>),
    Boolean(bool),
}
// impl fmt::Debug for Position {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_tuple("")
//          .field(&self.longitude)
//          .field(&self.latitude)
//          .finish()
//     }
// }

impl FieldValue {
    #[cfg(test)]
    pub fn liquid_date(&self) -> &model::DateTime {
        match self {
            FieldValue::Date(d) => d.as_liquid(),
            _ => panic!("Not a date"),
        }
    }
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

impl From<EditFieldValue> for FieldValue {
    fn from(value: EditFieldValue) -> Self {
        match value {
            EditFieldValue::String(v) => Self::String(v.to_owned()),
            EditFieldValue::Markdown(v) => Self::Markdown(v.to_owned()),
            EditFieldValue::Number(n) => Self::Number(n),
            EditFieldValue::Date(str) => FieldValue::Date(DateTime::from(&str).unwrap()),
            EditFieldValue::Boolean(b) => FieldValue::Boolean(b),
        }
    }
}

impl From<&FieldValue> for toml::Value {
    fn from(value: &FieldValue) -> Self {
        match value {
            FieldValue::String(v) => Self::String(v.to_owned()),
            FieldValue::Markdown(v) => Self::String(v.to_owned()),
            FieldValue::Number(n) => Self::Float(*n),
            FieldValue::Date(d) => Self::Datetime(toml_datetime::Datetime {
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
            }),
            FieldValue::Boolean(v) => Self::Boolean(v.to_owned()),
            FieldValue::Objects(o) => Self::Array(
                o.into_iter()
                    .map(|child| {
                        let mut vals: toml::map::Map<String, Value> = toml::map::Map::new();
                        for (key, cv) in child {
                            vals.insert(key.to_string(), cv.into());
                        }
                        Self::Table(vals)
                    })
                    .collect(),
            ),
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
            FieldValue::Date(d) => Some(model::ScalarCow::new(**d)),
            FieldValue::Markdown(s) => Some(model::ScalarCow::new(markdown_to_html(
                s,
                &ComrakOptions::default(),
            ))),
            _ => None,
        }
    }
    fn as_array(&self) -> Option<&dyn model::ArrayView> {
        match self {
            FieldValue::Objects(a) => Some(a),
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
        }
    }
}

impl FieldValue {
    pub fn from_toml(
        key: &String,
        field_type: &FieldType,
        value: &Value,
    ) -> Result<FieldValue, Box<dyn Error>> {
        match field_type {
            FieldType::String => Ok(FieldValue::String(
                value
                    .as_str()
                    .ok_or(InvalidFieldError::TypeMismatch {
                        field: key.to_owned(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    })?
                    .to_string(),
            )),
            FieldType::Markdown => Ok(FieldValue::Markdown(
                value
                    .as_str()
                    .ok_or(InvalidFieldError::TypeMismatch {
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
            FieldType::Boolean => Ok(FieldValue::Boolean(value.as_bool().ok_or(
                InvalidFieldError::TypeMismatch {
                    field: key.to_owned(),
                    field_type: field_type.to_string(),
                    value: value.to_string(),
                },
            )?)),
            FieldType::Date => {
                let mut date_str = (value.as_str().ok_or(InvalidFieldError::TypeMismatch {
                    field: key.to_owned(),
                    field_type: field_type.to_string(),
                    value: value.to_string(),
                })?)
                .to_string();
                // Legacy: support year-first formats:
                let year_first_fmt =
                    Regex::new(r"(?<year>\d{4})[\/-](?<month>\d{2})[\/-](?<day>\d{2})").unwrap();
                date_str = year_first_fmt
                    .replace(&date_str, "$month/$day/$year")
                    .to_string();
                // Also pretty lazy: check if we're missing time and add it
                if !date_str.contains(':') {
                    date_str = format!("{} 00:00:00", date_str);
                }
                // Append local offset if available
                if let Ok(offset) = UtcOffset::current_local_offset() {
                    let fmt =
                        format_description::parse("[offset_hour sign:mandatory][offset_minute]")?;
                    date_str = format!("{} {}", date_str, offset.format(&fmt)?);
                }
                // Supported formats:
                //
                // * `default` - `YYYY-MM-DD HH:MM:SS`
                // * `day_month` - `DD Month YYYY HH:MM:SS`
                // * `day_mon` - `DD Mon YYYY HH:MM:SS`
                // * `mdy` -  `MM/DD/YYYY HH:MM:SS`
                // * `dow_mon` - `Dow Mon DD HH:MM:SS YYYY`
                //
                // Offsets in one of the following forms, and are catenated with any of
                // the above formats.
                //
                // * `+HHMM`
                // * `-HHMM`
                // TODO: use this strategy for more accurate values
                // let toml_date = m_value.as_datetime().ok_or(InvalidFieldError {
                //     field: key.to_string(),
                // })?;
                // let date = toml_date.date.ok_or(InvalidFieldError {
                //     field: key.to_string(),
                // })?;
                // let offset = toml_date.offset.ok_or(InvalidFieldError {
                //     field: key.to_string(),
                // })?;
                // let liquid_date =
                //     DateTime::from_ymd(date.year as i32, date.month, date.day)
                //         .with_offset(offset);
                Ok(FieldValue::Date(DateTime::from(&date_str)?))
            }
        }
    }

    fn as_string(&self) -> String {
        match self {
            FieldValue::String(s) => s.clone(),
            FieldValue::Markdown(n) => n.clone(),
            FieldValue::Number(n) => n.to_string(),
            FieldValue::Date(d) => d.to_rfc2822(),
            FieldValue::Boolean(b) => b.to_string(),
            FieldValue::Objects(o) => format!("{:?}", o),
        }
    }
}
