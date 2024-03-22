use crate::constants::CDN_URL;
use liquid::{ObjectView, ValueView};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use tracing::warn;

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayType {
    Image,
    Video,
    Audio,
    Download,
}
impl DisplayType {
    fn to_str(&self) -> &str {
        match self {
            DisplayType::Image => "image",
            DisplayType::Audio => "audio",
            DisplayType::Video => "video",
            DisplayType::Download => "upload",
        }
    }
}
impl<'a> From<&'a DisplayType> for &'a str {
    fn from(value: &'a DisplayType) -> Self {
        value.to_str()
    }
}
impl Display for DisplayType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_str())
    }
}
impl From<DisplayType> for String {
    fn from(value: DisplayType) -> Self {
        value.to_str().to_string()
    }
}
impl From<&str> for DisplayType {
    fn from(value: &str) -> Self {
        match value {
            "image" => DisplayType::Image,
            "audio" => DisplayType::Audio,
            "video" => DisplayType::Video,
            "upload" => DisplayType::Download,
            &_ => todo!(),
        }
    }
}

#[derive(Debug, ObjectView, ValueView, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct File {
    pub sha: String,
    pub name: Option<String>,
    pub filename: String,
    pub mime: String,
    pub display_type: String,
    pub url: String,
}

impl File {
    pub fn new(
        sha: &str,
        name: Option<&str>,
        filename: &str,
        mime: &str,
        display_type: DisplayType,
    ) -> Self {
        Self {
            url: Self::url(sha),
            sha: sha.to_string(),
            name: name.map(|n| n.to_string()),
            filename: filename.to_string(),
            mime: mime.to_string(),
            display_type: display_type.to_string(),
        }
    }
    pub fn fill_from_map(mut self, map: &toml::map::Map<String, toml::Value>) -> Self {
        for (k, v) in map {
            match &k[..] {
                "sha" => {
                    self.sha = v.as_str().unwrap().into();
                    self.url = Self::url(&self.sha);
                }
                "name" => self.name = Some(v.as_str().unwrap().into()),
                "filename" => self.filename = v.as_str().unwrap().into(),
                "mime" => self.mime = v.as_str().unwrap().into(),
                "display_type" => self.display_type = v.as_str().unwrap().into(),
                _ => {
                    warn!("unknown file field {}", k);
                }
            }
        }
        self
    }
    pub fn to_toml(&self) -> toml::map::Map<std::string::String, toml::Value> {
        let mut m = toml::map::Map::new();
        for (k, v) in self.to_map(false) {
            m.insert(k.to_string(), toml::Value::String(v.to_owned()));
        }
        m
    }
    pub fn image() -> Self {
        Self {
            url: Self::url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: "image/*".to_string(),
            display_type: DisplayType::Image.to_string(),
        }
    }
    pub fn video() -> Self {
        Self {
            url: Self::url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: "image/*".to_string(),
            display_type: DisplayType::Video.to_string(),
        }
    }
    pub fn audio() -> Self {
        Self {
            url: Self::url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: "image/*".to_string(),
            display_type: DisplayType::Audio.to_string(),
        }
    }
    pub fn download() -> Self {
        Self {
            url: Self::url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: "*/*".to_string(),
            display_type: DisplayType::Download.to_string(),
        }
    }
    fn url(sha: &str) -> String {
        format!("{}/{}", CDN_URL, sha)
    }
    pub fn to_map(&self, include_url: bool) -> HashMap<&str, &String> {
        let mut m = HashMap::new();
        m.insert("sha", &self.sha);
        if let Some(name) = &self.name {
            m.insert("name", name);
        }
        m.insert("filename", &self.filename);
        m.insert("mime", &self.mime);
        m.insert("display_type", &self.display_type);
        if include_url {
            m.insert("url", &self.url);
        }
        m
    }
}
