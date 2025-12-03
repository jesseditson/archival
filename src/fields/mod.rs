mod date_time;
pub(crate) mod field_type;
pub(crate) mod field_value;
mod file;
pub(crate) mod meta;
pub use date_time::DateTime;
pub use field_type::{FieldType, InvalidFieldError};
pub use field_value::{FieldValue, ObjectValues, RenderedFieldValue, RenderedObjectValues};
pub use file::{DisplayType, File};
pub use meta::{Meta, MetaValue};
use serde::{Deserialize, Serialize};

use crate::{constants::UPLOADS_URL, manifest::Manifest, ArchivalError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfig {
    pub uploads_url: String,
    pub upload_prefix: String,
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self {
            uploads_url: UPLOADS_URL.to_owned(),
            upload_prefix: "".to_owned(),
        }
    }
}

impl FieldConfig {
    pub fn from_manifest(
        manifest: Option<&Manifest>,
        upload_prefix: Option<&str>,
    ) -> Result<Self, ArchivalError> {
        Ok(Self {
            uploads_url: manifest
                .and_then(|m| m.uploads_url.to_owned())
                .unwrap_or_else(|| UPLOADS_URL.to_owned()),
            upload_prefix: upload_prefix
                .map(|p| Ok(p.to_string()))
                .unwrap_or_else(|| {
                    manifest.map(|m| m.upload_prefix.to_owned()).ok_or_else(|| {
                        ArchivalError::new(
                            "No upload_prefix provided to field_config and none found in manifest",
                        )
                    })
                })?,
        })
    }
    pub fn template_config(uploads_url: String) -> Self {
        Self {
            uploads_url,
            upload_prefix: "".to_owned(),
        }
    }
}
