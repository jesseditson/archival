use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use toml::de;

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
            Ok(config) => {
                if let Some(config) = config {
                    config
                } else {
                    ArchivalConfig::default()
                }
            }
            Err(e) => {
                eprintln!("config parse failed, using default value. ({})", e);
                ArchivalConfig::default()
            }
        }
    }
}
