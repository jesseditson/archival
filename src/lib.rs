use std::{error::Error, fs, path::Path};

mod manifest;
mod objects;

use manifest::Manifest;
use objects::{ObjectDefinition, Objects};
use serde::{Deserialize, Serialize};
use toml::Table;

static MANIFEST_FILE_NAME: &'static str = "manifest.toml";

#[derive(Deserialize, Serialize)]
pub struct Site {
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
    Ok(Site { manifest, objects })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
