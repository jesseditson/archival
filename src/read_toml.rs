use std::{error::Error, fmt::Display, fs, path::Path};

use toml::Table;

#[derive(Debug)]
struct TomlError {
    error: Box<dyn Error>,
    file: String,
}
impl Display for TomlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}) {}", self.file, self.error)
    }
}
impl Error for TomlError {}

pub fn read_toml(path: &Path) -> Result<Table, Box<dyn Error>> {
    let toml = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(error) => {
            return Err(TomlError {
                error: error.into(),
                file: path.to_string_lossy().to_string(),
            }
            .into())
        }
    };
    let table: Table = toml::from_str(&toml)?;
    Ok(table)
}
