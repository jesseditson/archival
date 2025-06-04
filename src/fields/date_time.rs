use super::InvalidFieldError;
use liquid::model;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{self, Debug, Display},
};
use time::{
    format_description, macros::format_description, OffsetDateTime as DateTimeImpl, UtcOffset,
};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, PartialOrd, Hash)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct DateTime {
    #[serde(skip)]
    inner: Option<model::DateTime>,
    raw: String,
}

static YEAR_FIRST_FMT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?<year>\d{4})[\/-](?<month>\d{2})[\/-](?<day>\d{2})").unwrap());
static SHORT_DATE_FMT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?<month>\d{1,2})\/(?<day>\d{1,2})\/(?<year>\d{2,4})(?<rest>.*)$").unwrap()
});

impl DateTime {
    pub fn from(str: &str) -> Result<Self, InvalidFieldError> {
        let offset_date_time =
            parse_date_time(str).ok_or_else(|| InvalidFieldError::InvalidDate(str.to_owned()))?;
        // TODO: this is equivalent to
        // model::DateTime { inner: offset_date_time }
        // but inner is private, so we pay serialize, deserialize, and alloc
        // here.
        // See: https://github.com/cobalt-org/liquid-rust/issues/581
        let offset_date_string = offset_date_time
            .format(DATE_TIME_FORMAT)
            .map_err(|_| InvalidFieldError::InvalidDate(str.to_owned()))?;
        let liquid_date = model::DateTime::from_str(&offset_date_string)
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

    pub fn bounce(&mut self) {
        let date_str = Self::parse_date_string(self.raw.to_string())
            .unwrap_or_else(|_| format!("Invalid date value {}", self.raw));
        self.inner = Some(
            model::DateTime::from_str(&date_str)
                .unwrap_or_else(|| panic!("Invalid date value {}", self.raw)),
        )
    }

    pub fn borrowed_as_datetime(&self) -> &model::DateTime {
        if self.inner.is_none() {
            panic!("cannot borrow datetime before it is bounced");
        }
        self.inner.as_ref().unwrap()
    }

    pub fn as_liquid_datetime(&self) -> model::DateTime {
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

/// Parse a string representing the date and time.
/// Note that this is mostly directly derived from the datetime module in
/// liquid-core, because the supported formats are hardcoded into the lib.
///
/// Accepts any of the formats listed below and builds return an `Option`
/// containing a `DateTimeImpl`.
///
/// Supported formats:
///
/// * `default` - `YYYY-MM-DD HH:MM:SS`
/// * `day_month` - `DD Month YYYY HH:MM:SS`
/// * `day_mon` - `DD Mon YYYY HH:MM:SS`
/// * `mdy` -  `MM/DD/YYYY HH:MM:SS`
/// * `mdy short` -  `M/D/YY HH:MM:SS`
/// * `dow_mon` - `Dow Mon DD HH:MM:SS YYYY`
///
/// Offsets in one of the following forms, and are catenated with any of
/// the above formats.
///
/// * `+HHMM`
/// * `-HHMM`
///
/// Example:
///
/// * `dow_mon` format with an offset: "Tue Feb 16 10:00:00 2016 +0100"
fn parse_date_time(s: &str) -> Option<DateTimeImpl> {
    if s.is_empty() {
        None
    } else if let "now" | "today" = s.to_lowercase().trim() {
        Some(DateTimeImpl::now_utc())
    } else {
        let mut s = s.to_string();
        if let Some(matches) = SHORT_DATE_FMT_RE.captures(&s) {
            let mut year = matches["year"].to_string();
            if year.len() == 2 {
                let current_year = format!("{}", time::OffsetDateTime::now_utc().year());
                year = format!("{}{}", &current_year[..2], year);
            }
            s = format!(
                "{:0>2}/{:0>2}/{}{}",
                &matches["month"], &matches["day"], year, &matches["rest"]
            );
        }

        let offset_re = Regex::new(r"[+-][01][0-9]{3}$").unwrap();

        let offset = if offset_re.is_match(&s) { "" } else { " +0000" };
        let s = s + offset;

        USER_FORMATS
            .iter()
            .find_map(|f| DateTimeImpl::parse(s.as_str(), f).ok())
    }
}

const USER_FORMATS: &[&[time::format_description::FormatItem<'_>]] = &[
        DATE_TIME_FORMAT,
        DATE_TIME_FORMAT_SUBSEC,
        format_description!("[day] [month repr:long] [year] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"),
        format_description!("[day] [month repr:short] [year] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"),
        format_description!("[month]/[day]/[year] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"),
        // This doesn't work - last_two appears to only work as a format, and
        // always fails when parsing.
        // format_description!("[month padding:none]/[day padding:none]/[year padding:none repr:last_two] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"),
        format_description!("[weekday repr:short] [month repr:short] [day padding:none] [hour]:[minute]:[second] [year] [offset_hour sign:mandatory][offset_minute]"),
    ];

const DATE_TIME_FORMAT: &[time::format_description::FormatItem<'static>] = time::macros::format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"
);

const DATE_TIME_FORMAT_SUBSEC: &[time::format_description::FormatItem<'static>] = time::macros::format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond] [offset_hour sign:mandatory][offset_minute]"
);

mod test {
    use super::DateTime;
    impl DateTime {
        #[cfg(test)]
        fn unix_timestamp(&self) -> i64 {
            self.as_liquid_datetime().unix_timestamp()
        }
    }

    #[test]
    fn parse_date_time_empty_is_bad() {
        let input = "";
        let actual = DateTime::from(input);
        assert!(actual.is_err());
    }

    #[test]
    fn parse_date_time_bad() {
        let input = "aaaaa";
        let actual = DateTime::from(input);
        assert!(actual.is_err());
    }

    #[test]
    fn parse_date_time_now() {
        let input = "now";
        let actual = DateTime::from(input);
        assert!(actual.is_ok());
    }

    #[test]
    fn parse_date_time_today() {
        let input = "today";
        let actual = DateTime::from(input);
        assert!(actual.is_ok());

        let input = "Today";
        let actual = DateTime::from(input);
        assert!(actual.is_ok());
    }

    #[test]
    fn parse_date_time_serialized_format() {
        let input = "2016-02-16 10:00:00 +0100"; // default format with offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455613200);

        let input = "2016-02-16 10:00:00 +0000"; // default format UTC
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);

        let input = "2016-02-16 10:00:00"; // default format no offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);
    }

    #[test]
    fn parse_date_time_day_month_format() {
        let input = "16 February 2016 10:00:00 +0100"; // day_month format with offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455613200);

        let input = "16 February 2016 10:00:00 +0000"; // day_month format UTC
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);

        let input = "16 February 2016 10:00:00"; // day_month format no offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);
    }

    #[test]
    fn parse_date_time_day_mon_format() {
        let input = "16 Feb 2016 10:00:00 +0100"; // day_mon format with offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455613200);

        let input = "16 Feb 2016 10:00:00 +0000"; // day_mon format UTC
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);

        let input = "16 Feb 2016 10:00:00"; // day_mon format no offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);
    }

    #[test]
    fn parse_date_time_mdy_format() {
        let input = "02/16/2016 10:00:00 +0100"; // mdy format with offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455613200);

        let input = "02/16/2016 10:00:00 +0000"; // mdy format UTC
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);

        let input = "02/16/2016 10:00:00"; // mdy format no offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);
    }
    #[test]
    fn parse_date_time_short_mdy_format() {
        let input = "2/16/16 10:00:00 +0100"; // mdy format with offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455613200);

        let input = "2/16/16 10:00:00 +0000"; // mdy format UTC
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);

        let input = "2/16/16 10:00:00"; // mdy format no offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);
    }

    #[test]
    fn parse_date_time_dow_mon_format() {
        let input = "Tue Feb 16 10:00:00 2016 +0100"; // dow_mon format with offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455613200);

        let input = "Tue Feb 16 10:00:00 2016 +0000"; // dow_mon format UTC
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);

        let input = "Tue Feb 16 10:00:00 2016"; // dow_mon format no offset
        let actual = DateTime::from(input);
        assert_eq!(actual.unwrap().unix_timestamp(), 1455616800);
    }
}
