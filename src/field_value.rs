use crate::object_definition::{FieldType, InvalidFieldError};
use comrak::{markdown_to_html, ComrakOptions};
use liquid::{model, ValueView};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fmt::{self, Debug, Display},
};
use thiserror::Error;
use time::{format_description, UtcOffset};
use toml::Value;

#[derive(Debug, Error)]
pub enum FieldValueError {
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

pub type ObjectValues = HashMap<String, FieldValue>;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct DateTime {
    #[serde(skip)]
    inner: Option<model::DateTime>,
    raw: String,
}

impl DateTime {
    pub fn from(str: &str) -> Result<Self, InvalidFieldError> {
        let liquid_date =
            model::DateTime::from_str(str).ok_or(InvalidFieldError::InvalidDate(str.to_owned()))?;
        Ok(Self {
            inner: Some(liquid_date),
            raw: str.to_owned(),
        })
    }
    pub fn from_toml(toml_datetime: &toml_datetime::Datetime) -> Result<Self, InvalidFieldError> {
        // Convert to `YYYY-MM-DD HH:MM:SS`
        let mut date_str = if let Some(date) = toml_datetime.date {
            date.to_string()
        } else {
            let (y, m, d) = model::DateTime::now().to_calendar_date();
            format!("{:04}-{:02}-{:02}", y, m as u8, d)
        };
        if let Some(time) = toml_datetime.time {
            date_str += &format!(" {}", time);
        } else {
            date_str += " 00:00:00";
        }
        let liquid_date = if let Some(dt) = model::DateTime::from_str(&date_str) {
            dt
        } else {
            return Err(InvalidFieldError::InvalidDate(toml_datetime.to_string()));
        };
        Ok(Self {
            inner: Some(liquid_date),
            raw: toml_datetime.to_string(),
        })
    }
    pub fn now() -> Self {
        let inner = model::DateTime::now();
        let raw = inner.to_string();
        Self {
            inner: Some(inner),
            raw,
        }
    }
    pub fn from_ymd(year: i32, month: u8, date: u8) -> Self {
        let inner = model::DateTime::from_ymd(year, month, date);
        let raw = inner.to_string();
        Self {
            inner: Some(inner),
            raw,
        }
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
    pub fn parse_date_string(mut date_str: String) -> Result<String, Box<dyn Error>> {
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
            let fmt = format_description::parse("[offset_hour sign:mandatory][offset_minute]")?;
            date_str = format!("{} {}", date_str, offset.format(&fmt)?);
        }
        Ok(date_str)
    }

    pub fn as_datetime(&self) -> model::DateTime {
        if let Some(inner) = self.inner {
            inner
        } else {
            let date_str = Self::parse_date_string(self.raw.to_string())
                .unwrap_or_else(|_| format!("Invalid date value {}", self.raw));
            model::DateTime::from_str(&date_str)
                .unwrap_or_else(|| panic!("Invalid date value {}", self.raw))
        }
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw.to_owned())
    }
}

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
    List(Vec<String>),
}
// impl fmt::Debug for Position {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_tuple("")
//          .field(&self.longitude)
//          .field(&self.latitude)
//          .finish()
//     }
// }

fn err(f_type: &FieldType, value: String) -> FieldValueError {
    FieldValueError::InvalidValue(f_type.to_string(), value.to_owned())
}

impl FieldValue {
    pub fn val_with_type(f_type: &FieldType, value: String) -> Result<Self, Box<dyn Error>> {
        let t_val = toml::Value::try_from(&value)?;
        Ok(match f_type {
            FieldType::Boolean => Self::Boolean(t_val.as_bool().ok_or(err(f_type, value))?),
            FieldType::Markdown => {
                Self::Markdown(t_val.as_str().ok_or(err(f_type, value))?.to_string())
            }
            FieldType::Number => Self::Number(
                t_val.as_float().unwrap_or(
                    t_val
                        .as_integer()
                        .ok_or(err(f_type, value))
                        .map(|v| v as f64)?,
                ),
            ),
            FieldType::String => {
                Self::String(t_val.as_str().ok_or(err(f_type, value))?.to_string())
            }
            FieldType::Date => Self::Date(DateTime::from_toml(
                t_val.as_datetime().ok_or(err(f_type, value))?,
            )?),
            FieldType::List => Self::List(t_val.as_array().ok_or(err(f_type, value))?.to_vec()),
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
        match f_type {
            FieldType::Boolean => Self::Boolean(false),
            FieldType::Markdown => Self::Markdown("".to_owned()),
            FieldType::Number => Self::Number(0.0),
            FieldType::String => Self::String("".to_owned()),
            FieldType::Date => Self::Date(DateTime::now()),
        }
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
            FieldValue::Date(d) => Some(model::ScalarCow::new((*d).as_datetime())),
            FieldValue::Markdown(s) => Some(model::ScalarCow::new(markdown_to_html(
                s,
                &ComrakOptions::default(),
            ))),
            FieldValue::Boolean(b) => Some(model::ScalarCow::new(*b)),
            FieldValue::Objects(_) => None,
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
                if let Value::Datetime(val) = value {
                    return Ok(FieldValue::Date(DateTime::from_toml(val)?));
                }
                let mut date_str = (value.as_str().ok_or(InvalidFieldError::TypeMismatch {
                    field: key.to_owned(),
                    field_type: field_type.to_string(),
                    value: value.to_string(),
                })?)
                .to_string();
                date_str = DateTime::parse_date_string(date_str)?;
                Ok(FieldValue::Date(DateTime::from(&date_str)?))
            }
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
