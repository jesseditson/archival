use core::fmt::Display;
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

static MANIFEST_FILE_NAME: &'static str = "manifest.toml";
static PAGES_DIR_NAME: &'static str = "pages";
static OBJECTS_DIR_NAME: &'static str = "objects";
static BUILD_DIR_NAME: &'static str = "dist";
static STATIC_DIR_NAME: &'static str = "public";

#[derive(Deserialize, Serialize)]
struct ObjectDefinition {}

#[derive(Deserialize, Serialize)]
pub struct Manifest {
    pub pages_dir: PathBuf,
    pub objects_dir: PathBuf,
    pub build_dir: PathBuf,
    pub static_dir: PathBuf,
}

impl Manifest {
    fn default(root: &Path) -> Manifest {
        Manifest {
            pages_dir: root.join(PAGES_DIR_NAME),
            objects_dir: root.join(OBJECTS_DIR_NAME),
            build_dir: root.join(BUILD_DIR_NAME),
            static_dir: root.join(STATIC_DIR_NAME),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Site {
    pub objects: HashMap<String, ObjectDefinition>,
    pub manifest: Manifest,
}

#[derive(Debug)]
enum LoadFileErr {
    OpenError(std::io::Error),
    NotFoundError(std::io::Error),
    CorruptedError(std::io::Error),
    InvalidError(toml::de::Error),
}
impl Display for LoadFileErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadFileErr::OpenError(io_error) => write!(f, "{}", io_error),
            LoadFileErr::NotFoundError(io_error) => write!(f, "{}", io_error),
            LoadFileErr::InvalidError(de_error) => write!(f, "{}", de_error),
            LoadFileErr::CorruptedError(io_error) => write!(f, "{}", io_error),
        }
    }
}
impl std::error::Error for LoadFileErr {}

pub fn load_files(root: &Path) -> Result<Site, LoadFileErr> {
    // Load our manifest (should it exist)
    let manifest: Manifest = match toml_from_file(&root.join(MANIFEST_FILE_NAME)) {
        Ok(file) => file,
        Err(e) => match e {
            LoadFileErr::NotFoundError(e) => Manifest::default(&root),
            LoadFileErr::OpenError(e) => return Err(LoadFileErr::OpenError(e)),
            LoadFileErr::InvalidError(e) => return Err(LoadFileErr::InvalidError(e)),
            LoadFileErr::CorruptedError(e) => return Err(LoadFileErr::CorruptedError(e)),
        },
    };
    let objects: HashMap<String, ObjectDefinition> = HashMap::new();
    Ok(Site { manifest, objects })
}

fn toml_from_file<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: DeserializeOwned,
{
    let file_content = fs::read_to_string(path)?;
    let parse_result = toml::from_str(&file_content);
    let table = match parse_result {
        Ok(val) => val,
        Err(e) => Err(LoadFileErr::InvalidError(e)),
    };
    table
}
