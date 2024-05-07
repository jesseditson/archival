use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};
use toml::{Table, Value};

use crate::{constants::LAYOUT_DIR_NAME, file_system::FileSystemAPI, FieldConfig};

use super::constants::{
    BUILD_DIR_NAME, OBJECTS_DIR_NAME, OBJECT_DEFINITION_FILE_NAME, PAGES_DIR_NAME, STATIC_DIR_NAME,
};

#[derive(Debug, Clone)]
struct InvalidManifestError;
impl Error for InvalidManifestError {}
impl fmt::Display for InvalidManifestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid manifest")
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct Manifest {
    #[serde(skip)]
    root: PathBuf,
    pub archival_version: Option<String>,
    pub prebuild: Vec<String>,
    pub site_url: Option<String>,
    pub archival_site: Option<String>,
    pub object_definition_file: PathBuf,
    pub pages_dir: PathBuf,
    pub objects_dir: PathBuf,
    pub build_dir: PathBuf,
    pub static_dir: PathBuf,
    pub layout_dir: PathBuf,
    pub uploads_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "binary", derive(clap::ValueEnum))]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum ManifestField {
    ArchivalVersion,
    SiteUrl,
    ArchivalSite,
    ObjectsDir,
    Prebuild,
    PagesDir,
    BuildDir,
    StaticDir,
    LayoutDir,
    CdnUrl,
}

impl ManifestField {
    fn field_name(&self) -> &str {
        match self {
            ManifestField::ArchivalVersion => "archival_version",
            ManifestField::ArchivalSite => "archival_site",
            ManifestField::SiteUrl => "site_url",
            ManifestField::ObjectsDir => "objects",
            ManifestField::Prebuild => "prebuild",
            ManifestField::PagesDir => "pages",
            ManifestField::BuildDir => "build_dir",
            ManifestField::StaticDir => "static_dir",
            ManifestField::LayoutDir => "layout_dir",
            ManifestField::CdnUrl => "uploads_url",
        }
    }
}

impl fmt::Display for Manifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"
        archival_version: {}
        archival_site: {}
        site_url: {}
        uploads_url: {}
        object file: {}
        objects: {}
        pages: {}
        static files: {}
        layout dir: {}
        build dir: {}
        "#,
            self.archival_version
                .as_ref()
                .unwrap_or(&"unknown".to_owned()),
            self.archival_site.as_ref().unwrap_or(&"none".to_owned()),
            self.site_url.as_ref().unwrap_or(&"none".to_owned()),
            self.uploads_url
                .as_ref()
                .unwrap_or(&FieldConfig::get().uploads_url.to_string()),
            self.object_definition_file.display(),
            self.objects_dir.display(),
            self.pages_dir.display(),
            self.static_dir.display(),
            self.layout_dir.display(),
            self.build_dir.display()
        )
    }
}

impl Manifest {
    pub fn default(root: &Path) -> Manifest {
        Manifest {
            root: root.to_owned(),
            archival_version: None,
            prebuild: vec![],
            site_url: None,
            uploads_url: None,
            archival_site: None,
            object_definition_file: root.join(OBJECT_DEFINITION_FILE_NAME),
            pages_dir: root.join(PAGES_DIR_NAME),
            objects_dir: root.join(OBJECTS_DIR_NAME),
            build_dir: root.join(BUILD_DIR_NAME),
            static_dir: root.join(STATIC_DIR_NAME),
            layout_dir: root.join(LAYOUT_DIR_NAME),
        }
    }
    fn is_default(&self, field: &ManifestField) -> bool {
        let str_value = self.field_as_string(field);
        match field {
            ManifestField::Prebuild => str_value == "[]",
            ManifestField::ObjectsDir => {
                str_value == self.root.join(OBJECTS_DIR_NAME).to_string_lossy()
            }
            ManifestField::PagesDir => {
                str_value == self.root.join(PAGES_DIR_NAME).to_string_lossy()
            }
            ManifestField::BuildDir => {
                str_value == self.root.join(BUILD_DIR_NAME).to_string_lossy()
            }
            ManifestField::StaticDir => {
                str_value == self.root.join(STATIC_DIR_NAME).to_string_lossy()
            }
            ManifestField::LayoutDir => {
                str_value == self.root.join(LAYOUT_DIR_NAME).to_string_lossy()
            }
            _ => str_value.is_empty(),
        }
    }
    pub fn from_file(
        manifest_path: &Path,
        fs: &impl FileSystemAPI,
    ) -> Result<Manifest, Box<dyn Error>> {
        let root = manifest_path.parent().ok_or(InvalidManifestError)?;
        let mut manifest = Manifest::default(root);
        let string = fs.read_to_string(manifest_path)?.unwrap_or_default();
        let values: Table = toml::from_str(&string)?;
        let path_or_err = |value: Value| -> Result<PathBuf, InvalidManifestError> {
            if let Some(string) = value.as_str() {
                return Ok(root.join(string));
            }
            Err(InvalidManifestError)
        };
        for (key, value) in values.into_iter() {
            match key.as_str() {
                "archival_version" => {
                    manifest.archival_version = value.as_str().map(|s| s.to_string())
                }
                "archival_site" => manifest.archival_site = value.as_str().map(|s| s.to_string()),
                "uploads_url" => manifest.uploads_url = value.as_str().map(|s| s.to_string()),
                "site_url" => manifest.site_url = value.as_str().map(|s| s.to_string()),
                "prebuild" => {
                    manifest.prebuild = value.as_array().map_or(vec![], |v| {
                        v.iter()
                            .map(|s| s.as_str().map_or("".to_string(), |s| s.to_string()))
                            .collect()
                    })
                }
                "pages" => manifest.pages_dir = path_or_err(value)?,
                "objects" => manifest.objects_dir = path_or_err(value)?,
                "build_dir" => manifest.build_dir = path_or_err(value)?,
                "static_dir" => manifest.static_dir = path_or_err(value)?,
                "layout_dir" => manifest.layout_dir = path_or_err(value)?,
                _ => {}
            }
        }
        Ok(manifest)
    }

    fn toml_field(&self, field: &ManifestField) -> Option<Value> {
        match field {
            ManifestField::ArchivalVersion => self.archival_version.to_owned().map(Value::String),
            ManifestField::ArchivalSite => self.archival_site.to_owned().map(Value::String),
            ManifestField::SiteUrl => self.site_url.to_owned().map(Value::String),
            ManifestField::CdnUrl => self.uploads_url.to_owned().map(Value::String),
            ManifestField::Prebuild => {
                if self.prebuild.is_empty() {
                    None
                } else {
                    Some(Value::Array(
                        self.prebuild
                            .iter()
                            .map(|v| Value::String(v.to_string()))
                            .collect(),
                    ))
                }
            }
            ManifestField::ObjectsDir => Some(Value::String(
                self.objects_dir.to_string_lossy().to_string(),
            )),
            ManifestField::PagesDir => {
                Some(Value::String(self.pages_dir.to_string_lossy().to_string()))
            }
            ManifestField::BuildDir => {
                Some(Value::String(self.build_dir.to_string_lossy().to_string()))
            }
            ManifestField::StaticDir => {
                Some(Value::String(self.static_dir.to_string_lossy().to_string()))
            }
            ManifestField::LayoutDir => {
                Some(Value::String(self.layout_dir.to_string_lossy().to_string()))
            }
        }
    }

    pub fn set(&mut self, field: &ManifestField, value: String) {
        match field {
            ManifestField::ArchivalVersion => self.archival_version = Some(value),
            ManifestField::ArchivalSite => self.archival_site = Some(value),
            ManifestField::SiteUrl => self.site_url = Some(value),
            ManifestField::CdnUrl => self.uploads_url = Some(value),
            ManifestField::Prebuild => {
                todo!("Prebuild is not modifiable via events")
            }
            ManifestField::ObjectsDir => self.objects_dir = PathBuf::from(value),
            ManifestField::PagesDir => self.pages_dir = PathBuf::from(value),
            ManifestField::BuildDir => self.build_dir = PathBuf::from(value),
            ManifestField::StaticDir => self.static_dir = PathBuf::from(value),
            ManifestField::LayoutDir => self.layout_dir = PathBuf::from(value),
        }
    }

    pub fn field_as_string(&self, field: &ManifestField) -> String {
        match self.toml_field(field) {
            Some(fv) => match fv {
                Value::Array(a) => toml::to_string(&a).unwrap_or_default(),
                Value::String(s) => s,
                _ => panic!("unsupported manifest field type"),
            },
            None => String::default(),
        }
    }

    fn durable_fields() -> Vec<ManifestField> {
        vec![
            ManifestField::ArchivalVersion,
            ManifestField::ArchivalSite,
            ManifestField::SiteUrl,
            ManifestField::CdnUrl,
            ManifestField::Prebuild,
            ManifestField::ArchivalSite,
            ManifestField::PagesDir,
            ManifestField::ObjectsDir,
            ManifestField::BuildDir,
            ManifestField::StaticDir,
            ManifestField::ObjectsDir,
        ]
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        let mut write_obj = Table::new();
        for field in Self::durable_fields() {
            let key = field.field_name();
            if !self.is_default(&field) {
                if let Some(value) = self.toml_field(&field) {
                    write_obj.insert(key.to_string(), value);
                }
            }
        }
        toml::to_string_pretty(&write_obj)
    }

    pub fn watched_paths(&self) -> Vec<String> {
        [
            &self.object_definition_file,
            &self.objects_dir,
            &self.pages_dir,
            &self.static_dir,
            &self.layout_dir,
        ]
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
    }
}
