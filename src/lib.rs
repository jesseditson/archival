use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

mod manifest;
mod objects;
mod page;
mod reserved_fields;

use constants::MANIFEST_FILE_NAME;
use manifest::Manifest;
use objects::{ObjectDefinition, Objects};
use serde::{Deserialize, Serialize};
use toml::Table;

mod constants;

#[derive(Deserialize, Serialize)]
pub struct Site {
    pub root: PathBuf,
    pub objects: Objects,
    pub manifest: Manifest,
}

pub fn load_site(root: &Path) -> Result<Site, Box<dyn Error>> {
    // Load our manifest (should it exist)
    let manifest = match Manifest::from_file(&root.join(MANIFEST_FILE_NAME)) {
        Ok(m) => m,
        Err(_) => Manifest::default(root),
    };
    // Load our object definitions
    let objects_toml = fs::read_to_string(&manifest.object_definition_file)?;
    let objects_table: Table = toml::from_str(&objects_toml)?;
    let objects = ObjectDefinition::from_table(&objects_table)?;
    Ok(Site {
        root: root.to_path_buf(),
        manifest,
        objects,
    })
}

pub fn build_site(site: &Site) {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
