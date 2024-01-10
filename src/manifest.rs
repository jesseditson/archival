use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::{constants::LAYOUT_DIR_NAME, file_system::FileSystemAPI};

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

#[derive(Deserialize, Serialize)]
pub struct Manifest {
    pub object_definition_file: PathBuf,
    pub pages_dir: PathBuf,
    pub objects_dir: PathBuf,
    pub build_dir: PathBuf,
    pub static_dir: PathBuf,
    pub layout_dir: PathBuf,
}

impl fmt::Display for Manifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"
        object file: {}
        objects: {}
        pages: {}
        static files: {}
        layout dir: {}
        build dir: {}
        "#,
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
            object_definition_file: root.join(OBJECT_DEFINITION_FILE_NAME),
            pages_dir: root.join(PAGES_DIR_NAME),
            objects_dir: root.join(OBJECTS_DIR_NAME),
            build_dir: root.join(BUILD_DIR_NAME),
            static_dir: root.join(STATIC_DIR_NAME),
            layout_dir: root.join(LAYOUT_DIR_NAME),
        }
    }
    pub fn from_file(
        manifest_path: &Path,
        fs: &impl FileSystemAPI,
    ) -> Result<Manifest, Box<dyn Error>> {
        let root = manifest_path.parent().ok_or(InvalidManifestError)?;
        let mut manifest = Manifest::default(&root);
        let string = fs.read_to_string(manifest_path)?;
        let values: Table = toml::from_str(&string)?;
        let path_or_err = |value: Value| -> Result<PathBuf, InvalidManifestError> {
            if let Some(string) = value.as_str() {
                return Ok(root.join(string));
            }
            Err(InvalidManifestError)
        };
        for (key, value) in values.into_iter() {
            match key.as_str() {
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

    pub fn watched_paths(&self) -> Vec<String> {
        vec![
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
