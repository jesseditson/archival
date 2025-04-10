use super::BinaryCommand;
use crate::{
    binary::{ArchivalConfig, ExitStatus},
    constants::API_URL,
    events::{ArchivalEvent, EditFieldEvent},
    fields::{FieldType, File},
    file_system_stdlib,
    object::ValuePath,
    Archival, FieldValue,
};
use clap::{arg, value_parser, ArgMatches};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::{
    header::{HeaderMap, AUTHORIZATION},
    StatusCode,
};
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
    #[error("file '{0}' doesn't exist")]
    FileNotExists(PathBuf),
    #[error("invalid object path '{0}'")]
    InvalidObjectPath(PathBuf),
    #[error("invalid object type '{0}'")]
    InvalidObjectType(String),
    #[error("invalid field {0}")]
    InvalidField(String),
    #[error("cannot upload to type '{0}'")]
    NonUploadableType(String),
    #[error("upload failed {0}")]
    UploadFailed(String),
    #[error("could not infer repo from remotes: git command failed: {0}")]
    NoGit(String),
    #[error("could not infer repo from remotes: {0} - only github URLs supported.")]
    InferringRepoFailed(String),
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
                arg!(-r --repo <repo_name> "A repo name (e.g. github/jesseditson/blog) to use for this upload. If not provided, will be inferred from the first git remote.")
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
        let object_name = object
            .with_extension("")
            .file_name()
            .ok_or_else(|| UploadError::InvalidObjectPath(object.to_owned()))?
            .to_string_lossy()
            .to_string();
        let mut object_type = object
            .parent()
            .ok_or_else(|| UploadError::InvalidObjectPath(object.to_owned()))?
            .to_string_lossy()
            .to_string();
        // If we don't find the object type, it's likely that we mean to use a
        // root object, in which case they're the same. A check below will make
        // sure that this is truly the case (object_exists)
        if object_type.is_empty() {
            object_name.clone_into(&mut object_type);
        }
        let field = args.get_one::<String>("field").unwrap();
        let field_path = ValuePath::from_string(field);
        // Set up an archival site to make sure we're able to modify fields
        let fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let archival = Archival::new(fs)?;
        // Make sure that the specified object exists
        if !archival.object_exists(&object_type, &object_name)? {
            return Err(UploadError::InvalidObjectPath(object.to_owned()).into());
        }
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
        // Validate repo
        let repo_id = if let Some(repo) = args.get_one::<String>("repo") {
            repo.to_string()
        } else {
            let github_remote_match = Regex::new(r"github.com.+?\b(.+)\/(.+)\.git").unwrap();
            let git_command = std::process::Command::new("git")
                .current_dir(build_dir)
                .arg("remote")
                .arg("-v")
                .output()
                .map_err(|e| UploadError::NoGit(e.to_string()))?;
            let output = String::from_utf8(git_command.stdout.as_slice().to_vec())
                .map_err(|err| UploadError::InferringRepoFailed(err.to_string()))?;
            let first_origin = output.split("\n").next().ok_or_else(|| {
                UploadError::InferringRepoFailed(format!("No origins found in {}", output))
            })?;
            let first_match = github_remote_match
                .captures_iter(first_origin)
                .next()
                .ok_or_else(|| {
                    UploadError::InferringRepoFailed(format!(
                        "No github origin found in {}",
                        first_origin
                    ))
                })?;
            format!(
                "github/{}/{}",
                first_match.get(1).unwrap().as_str(),
                first_match.get(2).unwrap().as_str()
            )
        };
        // Ok, this looks legit. Upload the file
        let sha = archival.sha_for_file(file_path)?;
        let mime = mime_guess::from_path(file_path);
        let mut file = File::from_mime_guess(mime);
        file.sha = sha;
        file.filename = file_path
            .file_name()
            .map_or("".to_string(), |f| f.to_string_lossy().to_string());
        let upload_url = format!("{}/upload/{}/{}", API_URL, file.sha, file.filename);
        let field_data = FieldValue::File(file);
        let bar = ProgressBar::new_spinner();
        bar.set_style(ProgressStyle::with_template("{msg} {spinner}").unwrap());
        bar.set_message(format!("uploading {}", file_path.to_string_lossy()));
        let mut headers = HeaderMap::new();
        headers.append(
            AUTHORIZATION,
            format!("Bearer {}", access_token).parse().unwrap(),
        );
        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()?;
        let r = client
            .post(&upload_url)
            .query(&[
                ("action", "mpu-create"),
                ("content-type", mime.first_or_octet_stream().as_ref()),
                ("repo", &repo_id),
            ])
            .send()?;
        if !r.status().is_success() {
            return Err(UploadError::UploadFailed(r.text()?).into());
        }
        if !matches!(r.status(), StatusCode::CREATED) {
            let upload_r = r.json::<api_response::CreateUpload>()?;
            // TODO: set a max chunk size and parallelize chunked uploads
            let data = fs::read(file_path)?;
            let put_r = client
                .put(&upload_url)
                .query(&[
                    ("uploadId", &upload_r.upload_id),
                    ("partNumber", &"1".to_string()),
                ])
                .body(data)
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
        }
        // Now write our file
        archival.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: object_type.clone(),
                filename: object_name.clone(),
                path: ValuePath::empty(),
                value: Some(field_data.clone()),
                field: field.to_string(),
                source: None,
            }),
            None,
        )?;
        if let FieldValue::File(fd) = field_data {
            println!(
                "Wrote {} {} {}: {:?}",
                object_type, object_name, field_path, fd
            );
        } else {
            panic!("Invalid field data");
        }
        Ok(ExitStatus::Ok)
    }
}

mod api_response {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CreateUpload {
        #[allow(dead_code)]
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
