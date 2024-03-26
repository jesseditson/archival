use super::BinaryCommand;
use crate::{
    binary::{ArchivalConfig, ExitStatus},
    constants::API_URL,
    events::{ArchivalEvent, EditFieldEvent},
    fields::FieldType,
    fields::File,
    file_system_stdlib,
    object::ValuePath,
    Archival, FieldValue,
};
use clap::{arg, value_parser, ArgMatches};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::header::{HeaderMap, AUTHORIZATION};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum UploadError {
    #[error("no access token found. Run archival login first.")]
    NotLoggedIn,
    #[error("file {0} doesn't exist")]
    FileNotExists(PathBuf),
    #[error("current archival manifest does not define archival_site")]
    NoArchivalSite,
    #[error("invalid object path {0}")]
    InvalidObjectPath(PathBuf),
    #[error("invalid object type {0}")]
    InvalidObjectType(String),
    #[error("invalid field {0}")]
    InvalidField(String),
    #[error("cannot upload to type {0}")]
    NonUploadableType(String),
    #[error("upload failed {0}")]
    UploadFailed(String),
}

impl FieldType {
    pub fn is_uploadable(&self) -> bool {
        matches!(
            self,
            FieldType::Audio | FieldType::Video | FieldType::Upload | FieldType::Image
        )
    }
}

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "upload"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("uploads a local file to archival")
            .arg(
                arg!([object] "The object to upload data for")
                    .required(true)
                    .value_parser(value_parser!(PathBuf)),
            )
            .arg(
                arg!([field] "The field to upload to")
                    .required(true)
                    .value_parser(value_parser!(String)),
            )
            .arg(
                arg!([file] "The file to upload")
                    .required(true)
                    .value_parser(value_parser!(PathBuf)),
            )
    }
    fn handler(
        &self,
        build_dir: &Path,
        args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        // Fail fast if we aren't logged in
        let config = ArchivalConfig::get();
        let access_token = config.access_token.ok_or(UploadError::NotLoggedIn)?;
        // Fail fast if file doesn't exist
        let file_path = args.get_one::<PathBuf>("file").unwrap();
        if fs::metadata(file_path).is_err() {
            return Err(UploadError::FileNotExists(file_path.to_owned()).into());
        }
        let object = args.get_one::<PathBuf>("object").unwrap();
        // TODO: handle root objects
        let object_name = object
            .with_extension("")
            .file_name()
            .ok_or_else(|| UploadError::InvalidObjectPath(object.to_owned()))?
            .to_string_lossy()
            .to_string();
        let object_type = object
            .parent()
            .ok_or_else(|| UploadError::InvalidObjectPath(object.to_owned()))?
            .to_string_lossy()
            .to_string();
        let field = args.get_one::<String>("field").unwrap();
        let field_path = ValuePath::from_string(field);
        // Set up an archival site to make sure we're able to modify fields
        let fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let archival = Archival::new(fs)?;
        // Make sure we have a site
        let archival_site = archival
            .site
            .manifest
            .archival_site
            .as_ref()
            .ok_or(UploadError::NoArchivalSite)?;
        // Find the specified object definition
        let obj_def = archival
            .site
            .object_definitions
            .get(&object_type)
            .ok_or_else(|| UploadError::InvalidObjectType(object_type.to_owned()))?;
        // Find the specified field in the object definition
        let field_def = field_path
            .get_field_definition(obj_def)
            .map_err(|_| UploadError::InvalidField(field.to_owned()))?;
        // Validate that this is an ok field type to upload
        if !field_def.is_uploadable() {
            return Err(UploadError::NonUploadableType(field_def.to_string()).into());
        }
        // Make sure that the specified object exists
        if !archival.object_exists(&object_type, &object_name)? {
            return Err(UploadError::InvalidObjectPath(object.to_owned()).into());
        }
        // Ok, this looks legit. Upload the file
        let sha = archival.sha_for_file(file_path)?;
        let upload_url = format!("{}/upload/{}/{}", API_URL, archival_site, sha);
        let mime = mime_guess::from_path(file_path);
        let mut file = File::from_mime(mime);
        file.sha = sha;
        file.filename = file_path
            .file_name()
            .map_or("".to_string(), |f| f.to_string_lossy().to_string());
        let field_data = FieldValue::File(file);
        let bar = ProgressBar::new_spinner();
        bar.set_style(ProgressStyle::with_template("{msg} {spinner}").unwrap());
        bar.set_message(format!("uploading {}", file_path.to_string_lossy()));
        let mut headers = HeaderMap::new();
        headers.append(
            AUTHORIZATION,
            format!("token {}", access_token).parse().unwrap(),
        );
        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()?;
        let r = client
            .post(&upload_url)
            .query(&[("action", "mpu-create")])
            .send()?;
        if !r.status().is_success() {
            return Err(UploadError::UploadFailed(r.text()?).into());
        }
        let upload_r = r.json::<api_response::CreateUpload>()?;
        // TODO: set a max chunk size and parallelize chunked uploads
        let put_r = client
            .put(&upload_url)
            .query(&[
                ("uploadId", &upload_r.upload_id),
                ("partNumber", &"1".to_string()),
            ])
            .send()?;
        if !put_r.status().is_success() {
            return Err(UploadError::UploadFailed(put_r.text()?).into());
        }
        let part = put_r.json::<api_response::UploadedPart>()?;
        let r = client
            .post(&upload_url)
            .query(&[
                ("action", "mpu-complete"),
                ("uploadId", &upload_r.upload_id),
            ])
            .body(json!({ "parts": vec![part] }).to_string())
            .send()?;
        println!("status: {}", r.status());
        if !r.status().is_success() {
            return Err(UploadError::UploadFailed(r.text()?).into());
        }
        bar.finish();
        // Now write our file
        archival.send_event(ArchivalEvent::EditField(EditFieldEvent {
            object: object_type,
            filename: object_name,
            path: field_path,
            value: field_data,
        }))?;
        Ok(ExitStatus::Ok)
    }
}

mod api_response {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CreateUpload {
        pub key: String,
        pub upload_id: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct UploadedPart {
        pub part_number: usize,
        pub etag: String,
    }
}
