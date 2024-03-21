use std::{error::Error, fmt::Display, path::Path};

use toml::Table;
use tracing::instrument;

use crate::FileSystemAPI;

#[derive(Debug)]
pub struct NotFoundError;

impl Display for NotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Not Found")
    }
}

impl Error for NotFoundError {}

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

#[instrument(skip(fs))]
pub fn read_toml(path: &Path, fs: &impl FileSystemAPI) -> Result<Table, Box<dyn Error>> {
    let toml = match fs.read_to_string(path) {
        Ok(c) => match c {
            Some(c) => c,
            None => {
                return Err(TomlError {
                    error: NotFoundError.into(),
                    file: path.to_string_lossy().to_string(),
                }
                .into())
            }
        },
        Err(error) => {
            return Err(TomlError {
                error,
                file: path.to_string_lossy().to_string(),
            }
            .into())
        }
    };
    let table: Table = toml::from_str(&toml)?;
    Ok(table)
}
