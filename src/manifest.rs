use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};
use toml::{Table, Value};

use crate::{
    constants::LAYOUT_DIR_NAME, file_system::FileSystemAPI, object::ValuePath, FieldConfig,
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

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct ManifestEditorTypePathValidator {
    path: ValuePath,
    validate: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum ManifestEditorTypeValidator {
    Value(String),
    Path(ManifestEditorTypePathValidator),
}

impl From<&ManifestEditorTypeValidator> for toml::Value {
    fn from(value: &ManifestEditorTypeValidator) -> Self {
        match value {
            ManifestEditorTypeValidator::Value(v) => toml::Value::String(v.to_string()),
            ManifestEditorTypeValidator::Path(p) => {
                let mut map = toml::map::Map::new();
                map.insert("path".to_string(), toml::Value::String(p.path.to_string()));
                map.insert(
                    "validate".to_string(),
                    toml::Value::String(p.validate.to_string()),
                );
                map.into()
            }
        }
    }
}
#[derive(Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct ManifestEditorType {
    alias_of: String,
    validate: Vec<ManifestEditorTypeValidator>,
    editor_url: String,
}

impl From<&ManifestEditorType> for toml::Value {
    fn from(value: &ManifestEditorType) -> Self {
        let mut map = toml::map::Map::new();
        map.insert(
            "validate".into(),
            toml::Value::Array(value.validate.iter().map(|v| v.into()).collect()),
        );
        map.insert("editor_url".into(), value.editor_url.to_string().into());
        map.into()
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
    pub editor_types: HashMap<String, ManifestEditorType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "binary", derive(clap::ValueEnum))]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum ManifestField {
    ArchivalVersion,
    SiteUrl,
    ArchivalSite,
    ObjectDefinitionFile,
    ObjectsDir,
    Prebuild,
    PagesDir,
    BuildDir,
    StaticDir,
    LayoutDir,
    CdnUrl,
    EditorTypes,
}

impl ManifestField {
    fn field_name(&self) -> &str {
        match self {
            ManifestField::ArchivalVersion => "archival_version",
            ManifestField::ArchivalSite => "archival_site",
            ManifestField::SiteUrl => "site_url",
            ManifestField::ObjectDefinitionFile => "object_file",
            ManifestField::ObjectsDir => "objects",
            ManifestField::Prebuild => "prebuild",
            ManifestField::PagesDir => "pages",
            ManifestField::BuildDir => "build_dir",
            ManifestField::StaticDir => "static_dir",
            ManifestField::LayoutDir => "layout_dir",
            ManifestField::CdnUrl => "uploads_url",
            ManifestField::EditorTypes => "editor_types",
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
        editor types: {:?}
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
            self.build_dir.display(),
            self.editor_types
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
            editor_types: HashMap::new(),
        }
    }
    fn is_default(&self, field: &ManifestField) -> bool {
        let str_value = self.field_as_string(field);
        match field {
            ManifestField::Prebuild => str_value == "[]",
            ManifestField::ObjectDefinitionFile => {
                str_value
                    == self
                        .root
                        .join(OBJECT_DEFINITION_FILE_NAME)
                        .to_string_lossy()
            }
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
    pub fn from_string(root: &Path, string: String) -> Result<Manifest, Box<dyn Error>> {
        let mut manifest = Manifest::default(root);
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
                "object_file" => manifest.object_definition_file = path_or_err(value)?,
                "editor_types" => manifest.parse_editor_types(value)?,
                _ => {}
            }
        }
        Ok(manifest)
    }

    pub fn from_file(
        manifest_path: &Path,
        fs: &impl FileSystemAPI,
    ) -> Result<Manifest, Box<dyn Error>> {
        let root = manifest_path.parent().ok_or(InvalidManifestError)?;
        let string = fs.read_to_string(manifest_path)?.unwrap_or_default();
        Manifest::from_string(root, string)
    }

    fn toml_field(&self, field: &ManifestField) -> Option<Value> {
        match field {
            ManifestField::ArchivalVersion => self.archival_version.to_owned().map(Value::String),
            ManifestField::ArchivalSite => self.archival_site.to_owned().map(Value::String),
            ManifestField::SiteUrl => self.site_url.to_owned().map(Value::String),
            ManifestField::ObjectDefinitionFile => Some(Value::String(
                self.object_definition_file.to_string_lossy().to_string(),
            )),
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
            ManifestField::EditorTypes => {
                let mut map = toml::map::Map::new();
                for (type_name, type_val) in &self.editor_types {
                    map.insert(type_name.into(), type_val.into());
                }
                Some(Value::Table(map))
            }
        }
    }

    fn parse_editor_types(&mut self, types: toml::Value) -> Result<(), InvalidManifestError> {
        let types = match types {
            toml::Value::Table(t) => t,
            _ => return Err(InvalidManifestError),
        };
        let mut editor_types = HashMap::new();
        for (type_name, info) in types {
            let mut editor_type = ManifestEditorType::default();
            let info_map = match info {
                toml::Value::Table(t) => t,
                _ => return Err(InvalidManifestError),
            };
            editor_type.alias_of = info_map
                .get("type")
                .ok_or(InvalidManifestError)?
                .as_str()
                .ok_or(InvalidManifestError)?
                .to_string();
            let validator_val = info_map.get("validate").ok_or(InvalidManifestError)?;
            editor_type.validate = match validator_val {
                toml::Value::Array(arr) => arr
                    .iter()
                    .map(|val| match val {
                        toml::Value::Table(t) => {
                            let path = ValuePath::from_string(
                                t.get("path")
                                    .ok_or(InvalidManifestError)?
                                    .as_str()
                                    .ok_or(InvalidManifestError)?,
                            );
                            let validate = t
                                .get("validate")
                                .ok_or(InvalidManifestError)?
                                .as_str()
                                .ok_or(InvalidManifestError)?
                                .to_string();
                            Ok(ManifestEditorTypeValidator::Path(
                                ManifestEditorTypePathValidator { path, validate },
                            ))
                        }
                        toml::Value::String(s) => {
                            Ok(ManifestEditorTypeValidator::Value(s.to_owned()))
                        }
                        _ => Err(InvalidManifestError),
                    })
                    .collect::<Result<Vec<ManifestEditorTypeValidator>, _>>()?,
                _ => return Err(InvalidManifestError),
            };
            editor_types.insert(type_name, editor_type);
        }
        self.editor_types = editor_types;
        Ok(())
    }

    pub fn set(&mut self, field: &ManifestField, value: String) {
        match field {
            ManifestField::ArchivalVersion => self.archival_version = Some(value),
            ManifestField::ArchivalSite => self.archival_site = Some(value),
            ManifestField::ObjectDefinitionFile => {
                self.object_definition_file = PathBuf::from(value)
            }
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
            ManifestField::EditorTypes => {
                todo!("EditorTypes are not modifiable via events")
            }
        }
    }

    pub fn field_as_string(&self, field: &ManifestField) -> String {
        match self.toml_field(field) {
            Some(fv) => match fv {
                Value::Array(a) => toml::to_string(&a).unwrap_or_default(),
                Value::String(s) => s,
                Value::Table(t) => toml::to_string(&t).unwrap_or_default(),
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
            ManifestField::ObjectDefinitionFile,
            ManifestField::ArchivalSite,
            ManifestField::PagesDir,
            ManifestField::ObjectsDir,
            ManifestField::BuildDir,
            ManifestField::StaticDir,
            ManifestField::ObjectsDir,
            ManifestField::EditorTypes,
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

#[cfg(test)]
mod tests {

    use super::*;

    fn full_manifest_content() -> &'static str {
        "archival_version = '0.6.0'
        archival_site = 'jesse'
        site_url = 'https://jesse.onarchival.dev'
        object_file = 'm_objects.toml'
        prebuild = ['echo \"HELLO!\"']
        objects = 'm_objects'
        pages = 'm_pages'
        build_dir = 'm_dist'
        static_dir = 'm_public'
        layout_dir = 'm_layout'
        uploads_url = 'https://uploads.archival.dev'
        [editor_types.day]
        type = 'date'
        validate = ['\\d{2}/\\d{2}/\\d{4}']
        [editor_types.custom]
        type = 'meta'
        [[editor_types.custom.validate]]
        path = 'field_a'
        validate = '.+'
        [[editor_types.custom.validate]]
        path = 'field_b'
        validate = '.+'
        "
    }

    #[test]
    fn manifest_parsing() -> Result<(), Box<dyn Error>> {
        let m = Manifest::from_string(Path::new(""), full_manifest_content().to_string())?;
        println!("M: {:?}", m);
        assert_eq!(m.archival_version, Some("0.6.0".to_string()));
        assert_eq!(m.archival_site, Some("jesse".to_string()));
        assert_eq!(m.site_url, Some("https://jesse.onarchival.dev".to_string()));
        assert_eq!(
            m.object_definition_file,
            Path::new("m_objects.toml").to_path_buf()
        );
        assert_eq!(m.objects_dir, Path::new("m_objects").to_path_buf());
        assert_eq!(m.pages_dir, Path::new("m_pages").to_path_buf());
        assert_eq!(m.build_dir, Path::new("m_dist").to_path_buf());
        assert_eq!(m.static_dir, Path::new("m_public").to_path_buf());
        assert_eq!(m.layout_dir, Path::new("m_layout").to_path_buf());
        assert_eq!(
            m.uploads_url,
            Some("https://uploads.archival.dev".to_string())
        );
        assert_eq!(m.prebuild.len(), 1);
        let t1 = &m.editor_types["day"];
        assert_eq!(t1.alias_of, "date");
        assert_eq!(t1.validate.len(), 1);
        assert!(matches!(
            t1.validate[0],
            ManifestEditorTypeValidator::Value(_)
        ));
        if let ManifestEditorTypeValidator::Value(v) = &t1.validate[0] {
            assert_eq!(v, "\\d{2}/\\d{2}/\\d{4}");
        }
        let t2 = &m.editor_types["custom"];
        assert_eq!(t2.alias_of, "meta");
        assert_eq!(t2.validate.len(), 2);
        assert!(matches!(
            t2.validate[0],
            ManifestEditorTypeValidator::Path(_)
        ));
        if let ManifestEditorTypeValidator::Path(v) = &t2.validate[0] {
            assert_eq!(v.path.to_string(), "field_a");
            assert_eq!(v.validate, ".+");
        }
        assert!(matches!(
            t2.validate[1],
            ManifestEditorTypeValidator::Path(_)
        ));
        if let ManifestEditorTypeValidator::Path(v) = &t2.validate[1] {
            assert_eq!(v.path.to_string(), "field_b");
            assert_eq!(v.validate, ".+");
        }
        let manifest_output = m.to_toml()?;
        println!("MTOML {}", manifest_output);
        assert!(manifest_output.contains("[editor_types.day]"));
        assert!(manifest_output.contains("[editor_types.custom]"));
        assert!(manifest_output.contains("[[editor_types.custom.validate]]"));
        Ok(())
    }
}
