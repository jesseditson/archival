use crate::fields::FieldConfig;
use liquid::{ObjectView, ValueView};
use mime_guess::{mime::FromStrError, Mime, MimeGuess};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, str::FromStr};
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

#[cfg(feature = "typescript")]
mod typedefs {
    use typescript_type_def::{
        type_expr::{Ident, NativeTypeInfo, TypeExpr, TypeInfo},
        TypeDef,
    };
    pub struct DisplayTypeType;
    impl TypeDef for DisplayTypeType {
        const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
            r#ref: TypeExpr::ident(Ident("\"image\"|\"audio\"|\"video\"|\"upload\"")),
        });
    }
}

#[derive(Debug, ObjectView, ValueView, Deserialize, Serialize, Clone, PartialEq, PartialOrd)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct File {
    pub sha: String,
    pub name: Option<String>,
    pub filename: String,
    pub mime: String,
    #[cfg_attr(
        feature = "typescript",
        type_def(type_of = "typedefs::DisplayTypeType")
    )]
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
            url: Self::_url(sha),
            sha: sha.to_string(),
            name: name.map(|n| n.to_string()),
            filename: filename.to_string(),
            mime: mime.to_string(),
            display_type: display_type.to_string(),
        }
    }
    pub fn from_mime(mime_str: &str) -> Result<Self, FromStrError> {
        let mime = Mime::from_str(mime_str)?;
        let mut f = match mime.type_() {
            mime_guess::mime::VIDEO => Self::video(),
            mime_guess::mime::AUDIO => Self::audio(),
            mime_guess::mime::IMAGE => Self::image(),
            _ => Self::download(),
        };
        f.mime = mime.to_string();
        Ok(f)
    }
    pub fn from_mime_guess(mime: MimeGuess) -> Self {
        let m_type = mime.first_or_octet_stream();
        let mut f = match m_type.type_() {
            mime_guess::mime::VIDEO => Self::video(),
            mime_guess::mime::AUDIO => Self::audio(),
            mime_guess::mime::IMAGE => Self::image(),
            _ => Self::download(),
        };
        f.mime = m_type.to_string();
        f
    }
    pub fn get_key(&self, str: &str) -> Option<&String> {
        match str {
            "sha" => Some(&self.sha),
            "name" => self.name.as_ref(),
            "filename" => Some(&self.filename),
            "mime" => Some(&self.mime),
            "display_type" => Some(&self.display_type),
            _ => None,
        }
    }
    pub fn fill_from_map(mut self, map: &toml::map::Map<String, toml::Value>) -> Self {
        for (k, v) in map {
            match &k[..] {
                "sha" => {
                    self.sha = v.as_str().unwrap().into();
                    self.url = Self::_url(&self.sha);
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
        let mime = "image/*";
        Self {
            url: Self::_url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: mime.to_string(),
            display_type: DisplayType::Image.to_string(),
        }
    }
    pub fn video() -> Self {
        let mime = "video/*";
        Self {
            url: Self::_url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: mime.to_string(),
            display_type: DisplayType::Video.to_string(),
        }
    }
    pub fn audio() -> Self {
        let mime = "audio/*";
        Self {
            url: Self::_url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: mime.to_string(),
            display_type: DisplayType::Audio.to_string(),
        }
    }
    pub fn download() -> Self {
        let mime = "*/*";
        Self {
            url: Self::_url(""),
            sha: "".to_string(),
            name: None,
            filename: "".to_string(),
            mime: mime.to_string(),
            display_type: DisplayType::Download.to_string(),
        }
    }
    fn _url(sha: &str) -> String {
        if sha.is_empty() {
            return "".to_string();
        }
        let config = FieldConfig::get();
        format!("{}/{}", config.uploads_url, sha)
    }
    pub fn update_url(&mut self) {
        self.url = Self::_url(&self.sha);
    }
    pub fn url(&self) -> String {
        Self::_url(&self.sha)
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

#[cfg(test)]
pub mod tests {

    use super::*;

    #[test]
    fn files_have_urls_when_specific() {
        let mut i = File::image();
        i.mime = "image/jpeg".to_string();
        let mut a = File::audio();
        a.mime = "audio/ogg".to_string();
        let mut d = File::download();
        d.mime = "application/pdf".to_string();
        let mut v = File::video();
        v.mime = "video/mp4".to_string();
        let one_of_each = [i, a, d, v];
        for mut file in one_of_each {
            file.sha = "fake-sha".to_string();
            println!("{}", file.url());
            assert!(!file.url().is_empty());
        }
    }
    #[test]
    fn files_dont_have_urls_until_specific() {
        let one_of_each = [
            File::image(),
            File::audio(),
            File::download(),
            File::video(),
        ];
        for mut file in one_of_each {
            file.sha = "fake-sha".to_string();
            println!("URL: {}", file.url());
            assert!(!file.url().is_empty());
            assert!(!file.url().contains('.'));
        }
    }
}
