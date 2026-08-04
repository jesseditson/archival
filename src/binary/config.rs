use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use toml::de;
use toml_edit::{value, DocumentMut};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ArchivalConfig {
    pub access_token: Option<String>,
}

impl ArchivalConfig {
    pub fn location() -> PathBuf {
        Path::join(
            &home::home_dir().expect("unable to determine $HOME"),
            ".archivalrc",
        )
    }

    pub fn from_fs() -> Result<Option<Self>, de::Error> {
        let path = Self::location();
        if let Ok(existing) = fs::read_to_string(path) {
            Ok(toml::from_str(&existing)?)
        } else {
            Ok(None)
        }
    }

    pub fn get() -> Self {
        match Self::from_fs() {
            Ok(config) => config.unwrap_or_default(),
            Err(e) => {
                eprintln!("config parse failed, using default value. ({})", e);
                ArchivalConfig::default()
            }
        }
    }

    pub fn set_access_token(access_token: &str) -> Result<()> {
        Self::update(&Self::location(), |doc| {
            doc["access_token"] = value(access_token)
        })
    }

    fn update(path: &Path, edit: impl FnOnce(&mut DocumentMut)) -> Result<()> {
        let existing = match fs::read_to_string(path) {
            Ok(existing) => existing,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let mut doc = existing.parse::<DocumentMut>()?;
        edit(&mut doc);
        fs::write(path, doc.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update_str(existing: &str) -> String {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), existing).unwrap();
        ArchivalConfig::update(file.path(), |doc| doc["access_token"] = value("new-token"))
            .unwrap();
        fs::read_to_string(file.path()).unwrap()
    }

    #[test]
    fn update_preserves_other_keys() {
        let updated = update_str(
            "# my config\nsome_key = \"value\"\naccess_token = \"old-token\"\n\n[a_table]\nnested = true\n",
        );
        assert_eq!(
            updated,
            "# my config\nsome_key = \"value\"\naccess_token = \"new-token\"\n\n[a_table]\nnested = true\n"
        );
    }

    #[test]
    fn update_adds_missing_key() {
        let updated = update_str("some_key = \"value\"\n");
        assert_eq!(
            updated,
            "some_key = \"value\"\naccess_token = \"new-token\"\n"
        );
    }

    #[test]
    fn update_creates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".archivalrc");
        ArchivalConfig::update(&path, |doc| doc["access_token"] = value("new-token")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "access_token = \"new-token\"\n"
        );
    }

    #[test]
    fn update_refuses_to_clobber_invalid_config() {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), "this is not toml").unwrap();
        assert!(
            ArchivalConfig::update(file.path(), |doc| doc["access_token"] = value("new-token"))
                .is_err()
        );
        assert_eq!(fs::read_to_string(file.path()).unwrap(), "this is not toml");
    }
}
