use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Debug, Display};
use thiserror::Error;

use crate::manifest::EditorTypes;

#[cfg(feature = "json-schema")]
use super::{file::DisplayType, File};

#[derive(Error, Debug, Clone)]
pub enum InvalidFieldError {
    #[error("unrecognized type {0}")]
    UnrecognizedType(String),
    #[error("invalid enum {0} - only string enums supported.")]
    InvalidEnum(String),
    #[error("invalid oneof {0:?} - parse failed")]
    InvalidOneof(String),
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
    #[error(
        "oneof mismatch for field {field:?} - expected type {value:?} to be in {field_type:?}"
    )]
    OneofMismatch {
        field: String,
        field_type: String,
        value: String,
    },
    #[error(
        "enum mismatch for field {field:?} - expected value {value:?} to be in {field_type:?}"
    )]
    EnumMismatch {
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
    pub struct FieldTypeDef;
    impl TypeDef for FieldTypeDef {
        const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
            r#ref: TypeExpr::ident(Ident("FieldType")),
        });
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct OneofOption {
    pub name: String,
    // Avoid cycle by just inlining the def
    #[cfg_attr(feature = "typescript", type_def(type_of = "typedefs::FieldTypeDef"))]
    pub r#type: FieldType,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum FieldType {
    String,
    Number,
    Date,
    Enum(Vec<String>),
    Markdown,
    Boolean,
    Image,
    Video,
    Upload,
    Audio,
    Meta,
    Oneof(Vec<OneofOption>),
    Alias(
        #[cfg_attr(feature = "typescript", type_def(type_of = "typedefs::AliasTypeDef"))]
        Box<(FieldType, String)>,
    ),
}

impl FieldType {
    pub fn as_str<'a>(&'a self) -> Cow<'a, str> {
        match self {
            Self::String => "string".into(),
            Self::Number => "number".into(),
            Self::Enum(v) => format!("[{}]", v.join(",")).into(),
            Self::Date => "date".into(),
            Self::Markdown => "markdown".into(),
            Self::Boolean => "boolean".into(),
            Self::Image => "image".into(),
            Self::Video => "video".into(),
            Self::Audio => "audio".into(),
            Self::Upload => "upload".into(),
            Self::Meta => "meta".into(),
            Self::Oneof(v) => v
                .iter()
                .map(|f| format!("{}:{}", f.name, f.r#type.as_str()))
                .collect::<Vec<_>>()
                .join("|")
                .to_string()
                .into(),
            Self::Alias(a) => a.0.as_str(),
        }
    }
    pub fn from_str(
        string: &str,
        editor_types: &EditorTypes,
    ) -> Result<FieldType, InvalidFieldError> {
        match string {
            "string" => Ok(FieldType::String),
            // Note that enums are only supported via direct instantiation
            "number" => Ok(FieldType::Number),
            "date" => Ok(FieldType::Date),
            "markdown" => Ok(FieldType::Markdown),
            "boolean" => Ok(FieldType::Boolean),
            "image" => Ok(FieldType::Image),
            "video" => Ok(FieldType::Video),
            "audio" => Ok(FieldType::Audio),
            "upload" => Ok(FieldType::Upload),
            "meta" => Ok(FieldType::Meta),
            // Note that oneofs are only supported via direct instantiation
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
        write!(f, "{}", self.as_str())
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
                    if let Some(date) = options.set_dates_to {
                        let fmt = time::format_description::parse(
                            "[year]-[month]-[day] [hour]:[minute]:[second]",
                        )
                        .unwrap();
                        let date_str = date.with_time(time::Time::MIDNIGHT).format(&fmt).unwrap();
                        schema.insert("const".into(), date_str.into());
                    } else {
                        schema.insert("format".into(), "date".into());
                    }
                    schema
                } else if let Self::Enum(valid_values) = self {
                    use serde_json::json;

                    let mut schema = serde_json::Map::new();
                    schema.insert("description".into(), description.into());
                    schema.insert("type".into(), "string".into());
                    schema.insert("enum".into(), json!(valid_values));
                    schema
                } else if let Self::Oneof(field_types) = self {
                    let mut schema = serde_json::Map::new();
                    schema.insert("description".into(), description.into());
                    schema.insert(
                        "oneOf".into(),
                        field_types
                            .iter()
                            .map(|t| {
                                let mut schema = serde_json::Map::new();
                                schema.insert("type".into(), "object".into());
                                schema.insert("additionalProperties".into(), false.into());
                                schema.insert("properties".into(), serde_json::json!({
                                    "type": {
                                        "type": "string",
                                        "const": t.name
                                    },
                                    "value": t.r#type.to_json_schema_property(&format!("{} - {}", description, t.name), options)
                                }));
                                schema.insert("required".into(), serde_json::json!(["type", "value"]));
                                schema
                            })
                            .collect(),
                    );
                    schema
                } else {
                    let mut schema = serde_json::Map::new();
                    schema.insert("description".into(), description.into());
                    // Simple types
                    let mut is_object = false;
                    schema.insert(
                        "type".into(),
                        match self {
                            Self::String => "string".into(),
                            Self::Number => "number".into(),
                            Self::Markdown => "string".into(),
                            Self::Boolean => "boolean".into(),
                            // At some point, we should support providing a
                            // schema for meta types, which would require
                            // either inferring types based on validation or
                            // allowing the template to directly provide a
                            // schema for a given type.
                            Self::Meta => {
                                is_object = true;
                                "object".into()
                            }
                            _ => panic!("don't know how to parse a schema from {:?}", self),
                        },
                    );
                    if is_object {
                        schema.insert("additionalProperties".into(), false.into());
                        schema.insert("properties".into(), serde_json::json!({}));
                        schema.insert("required".into(), serde_json::json!([]));
                    }
                    schema
                }
            }
        }
    }
}
