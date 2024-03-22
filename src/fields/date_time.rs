use super::InvalidFieldError;
use liquid::model;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{self, Debug, Display},
};
use time::{format_description, UtcOffset};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct DateTime {
    #[serde(skip)]
    inner: Option<model::DateTime>,
    raw: String,
}

static YEAR_FIRST_FMT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?<year>\d{4})[\/-](?<month>\d{2})[\/-](?<day>\d{2})").unwrap());

impl DateTime {
    pub fn from(str: &str) -> Result<Self, InvalidFieldError> {
        let liquid_date = model::DateTime::from_str(str)
            .ok_or_else(|| InvalidFieldError::InvalidDate(str.to_owned()))?;
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
        date_str = YEAR_FIRST_FMT_RE
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
