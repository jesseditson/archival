use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum InvalidFieldError {
    #[error("unrecognized type {0}")]
    UnrecognizedType(String),
    #[error("invalid date {0}")]
    InvalidDate(String),
    #[error(
        "type mismatch for field {field:?} - expected type {field_type:?}, got value {value:?}"
    )]
    TypeMismatch {
        field: String,
        field_type: String,
        value: String,
    },
    #[error("invalid child {key:?}[{index:?}] {child:?}")]
    InvalidChild {
        key: String,
        index: usize,
        child: String,
    },
    #[error("not an array: {key:?} ({value:?})")]
    NotAnArray { key: String, value: String },
    #[error("cannot define an object with reserved name {0}")]
    ReservedObjectNameError(String),
    #[error("cannot create type {0} from a string value")]
    UnsupportedStringValue(String),
    #[error("type {0} was not provided a value and has no default")]
    NoDefaultForType(String),
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum FieldType {
    String,
    Number,
    Date,
    Markdown,
    Boolean,
    Image,
    Video,
    Upload,
    Audio,
    Meta,
}

impl FromStr for FieldType {
    type Err = InvalidFieldError;
    fn from_str(string: &str) -> Result<FieldType, InvalidFieldError> {
        match string {
            "string" => Ok(FieldType::String),
            "number" => Ok(FieldType::Number),
            "date" => Ok(FieldType::Date),
            "markdown" => Ok(FieldType::Markdown),
            "boolean" => Ok(FieldType::Boolean),
            "image" => Ok(FieldType::Image),
            "video" => Ok(FieldType::Video),
            "audio" => Ok(FieldType::Audio),
            "upload" => Ok(FieldType::Upload),
            "meta" => Ok(FieldType::Meta),
            _ => Err(InvalidFieldError::UnrecognizedType(string.to_string())),
        }
    }
}

impl FieldType {
    pub fn to_str(&self) -> &str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Date => "date",
            Self::Markdown => "markdown",
            Self::Boolean => "boolean",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Upload => "upload",
            Self::Meta => "meta",
        }
    }
}

impl Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}
