use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use thiserror::Error;

use crate::manifest::EditorTypes;

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
    #[error("field '{0}' failed validator '{1}'")]
    FailedValidation(String, String),
}

#[cfg(feature = "typescript")]
mod typedefs {
    use typescript_type_def::{
        type_expr::{Ident, NativeTypeInfo, TypeExpr, TypeInfo},
        TypeDef,
    };
    pub struct AliasTypeDef;
    impl TypeDef for AliasTypeDef {
        const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
            r#ref: TypeExpr::ident(Ident("[FieldType, string]")),
        });
    }
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
    Alias(
        #[cfg_attr(feature = "typescript", type_def(type_of = "typedefs::AliasTypeDef"))]
        Box<(FieldType, String)>,
    ),
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
            Self::Alias(a) => a.0.to_str(),
        }
    }
    pub fn from_str(
        string: &str,
        editor_types: &EditorTypes,
    ) -> Result<FieldType, InvalidFieldError> {
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
            t => {
                if let Some(et) = editor_types.get(t) {
                    Ok(FieldType::Alias(Box::new((
                        FieldType::from_str(&et.alias_of, editor_types)?,
                        t.to_string(),
                    ))))
                } else {
                    Err(InvalidFieldError::UnrecognizedType(string.to_string()))
                }
            }
        }
    }
}

impl Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}
