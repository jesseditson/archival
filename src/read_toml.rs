use std::{error::Error, fs, path::Path};

use toml::Table;

pub fn read_toml(path: &Path) -> Result<Table, Box<dyn Error>> {
    let toml = fs::read_to_string(path)?;
    let table: Table = toml::from_str(&toml)?;
    Ok(table)
}
