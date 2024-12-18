use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use thiserror::Error;

use crate::manifest::EditorTypes;

#[cfg(feature = "json-schema")]
use super::{file::DisplayType, File};

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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
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

    pub fn is_file_type(&self) -> bool {
        matches!(
            self,
            FieldType::Image | FieldType::Audio | FieldType::Video | FieldType::Upload
        )
    }
}

impl Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

#[cfg(feature = "json-schema")]
impl FieldType {
    fn maybe_file_type(&self) -> Option<DisplayType> {
        match self {
            FieldType::Image => Some(DisplayType::Image),
            FieldType::Video => Some(DisplayType::Video),
            FieldType::Audio => Some(DisplayType::Audio),
            FieldType::Upload => Some(DisplayType::Download),
            _ => None,
        }
    }

    pub fn to_json_schema_property(
        &self,
        description: &str,
        options: &crate::json_schema::ObjectSchemaOptions,
    ) -> crate::json_schema::ObjectSchema {
        match self {
            Self::Alias(a) => a.0.to_json_schema_property(description, options),
            _ => {
                if let Some(display_type) = self.maybe_file_type() {
                    File::to_json_schema_property(description, display_type, options)
                } else if matches!(self, Self::Date) {
                    let mut schema = serde_json::Map::new();
                    schema.insert("description".into(), description.into());
                    schema.insert("type".into(), "string".into());
                    schema.insert("format".into(), "date".into());
                    schema
                } else {
                    let mut schema = serde_json::Map::new();
                    schema.insert("description".into(), description.into());
                    // Simple types
                    schema.insert(
                        "type".into(),
                        match self {
                            Self::String => "string".into(),
                            Self::Number => "number".into(),
                            Self::Markdown => "string".into(),
                            Self::Boolean => "boolean".into(),
                            // At some point, we should support providing a schema for meta types
                            Self::Meta => "object".into(),
                            _ => panic!("don't know how to parse a schema from {:?}", self),
                        },
                    );
                    schema
                }
            }
        }
    }
}
