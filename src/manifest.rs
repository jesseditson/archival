use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};
use toml::{Table, Value};

use crate::{
    constants::{CDN_URL, LAYOUT_DIR_NAME},
    file_system::FileSystemAPI,
};

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
pub struct Manifest {
    pub archival_version: Option<String>,
    pub prebuild: Vec<String>,
    pub site_url: Option<String>,
    pub object_definition_file: PathBuf,
    pub pages_dir: PathBuf,
    pub objects_dir: PathBuf,
    pub build_dir: PathBuf,
    pub static_dir: PathBuf,
    pub layout_dir: PathBuf,
    pub cdn_url: String,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ManifestField {
    ArchivalVersion,
    SiteUrl,
    ObjectsDir,
    PagesDir,
    BuildDir,
    StaticDir,
    LayoutDir,
    CDNUrl,
}

impl fmt::Display for Manifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"
        archival_version: {}
        site_url: {}
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
            self.site_url.as_ref().unwrap_or(&"none".to_owned()),
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
            archival_version: None,
            prebuild: vec![],
            site_url: None,
            object_definition_file: root.join(OBJECT_DEFINITION_FILE_NAME),
            pages_dir: root.join(PAGES_DIR_NAME),
            objects_dir: root.join(OBJECTS_DIR_NAME),
            build_dir: root.join(BUILD_DIR_NAME),
            static_dir: root.join(STATIC_DIR_NAME),
            layout_dir: root.join(LAYOUT_DIR_NAME),
            cdn_url: CDN_URL.to_string(),
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

    pub fn field(&self, field: &ManifestField) -> Option<String> {
        match field {
            ManifestField::ArchivalVersion => self.archival_version.to_owned(),
            ManifestField::SiteUrl => self.site_url.to_owned(),
            ManifestField::ObjectsDir => Some(self.pages_dir.to_string_lossy().to_string()),
            ManifestField::PagesDir => Some(self.pages_dir.to_string_lossy().to_string()),
            ManifestField::BuildDir => Some(self.build_dir.to_string_lossy().to_string()),
            ManifestField::StaticDir => Some(self.static_dir.to_string_lossy().to_string()),
            ManifestField::LayoutDir => Some(self.layout_dir.to_string_lossy().to_string()),
            ManifestField::CDNUrl => Some(self.cdn_url.to_owned()),
        }
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        let mut write_obj = Table::new();
        for key in [
            "archival_version",
            "site_url",
            "object file",
            "objects",
            "pages",
            "static_dir",
            "layout_dir",
            "build_dir",
        ] {
            let value = match key {
                "archival_version" => self.field(&ManifestField::ArchivalVersion),
                "site_url" => self.field(&ManifestField::SiteUrl),
                "pages" => self.field(&ManifestField::PagesDir),
                "objects" => self.field(&ManifestField::ObjectsDir),
                "build_dir" => self.field(&ManifestField::BuildDir),
                "static_dir" => self.field(&ManifestField::StaticDir),
                "layout_dir" => self.field(&ManifestField::ObjectsDir),
                _ => None,
            };
            if let Some(value) = value {
                write_obj.insert(key.to_string(), Value::String(value));
            }
        }
        write_obj.insert(
            "prebuild".to_string(),
            Value::Array(
                self.prebuild
                    .iter()
                    .map(|v| Value::String(v.to_string()))
                    .collect(),
            ),
        );
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
