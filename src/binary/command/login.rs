use super::BinaryCommand;
use crate::{
    binary::{ArchivalConfig, ExitStatus},
    constants::{AUTH_URL, CLI_TOKEN_PUBLIC_KEY},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clap::ArgMatches;
use indicatif::{ProgressBar, ProgressStyle};
use nanoid::nanoid;
use reqwest::StatusCode;
use rsa::{pkcs8::DecodePublicKey, sha2::Sha256, Oaep, RsaPublicKey};
use std::{fs, path::Path, thread, time::Duration};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "login"
    }
    fn no_path(&self) -> bool {
        true
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("Log in to archival.dev and store credentials locally in ~/.archivalrc")
    }
    fn handler(
        &self,
        _build_dir: &Path,
        _args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let config_file_path = ArchivalConfig::location();
        let secret_client_id = nanoid!(21);
        let public_key = RsaPublicKey::from_public_key_pem(CLI_TOKEN_PUBLIC_KEY)?;
        let mut rng = rand::thread_rng();
        let padding = Oaep::new::<Sha256>();
        let enc_data = public_key.encrypt(&mut rng, padding, secret_client_id.as_bytes())?;
        let b64_encoded = STANDARD.encode(enc_data);
        let auth_url = format!("{}?code={}", AUTH_URL, urlencoding::encode(&b64_encoded));
        let token_url = format!("{}/token", AUTH_URL);
        println!("To log in, open this URL in your browser\n{}", auth_url);
        let bar = ProgressBar::new_spinner();
        bar.set_style(ProgressStyle::with_template("{msg} {spinner}").unwrap());
        bar.set_message("waiting for login to complete");
        bar.enable_steady_tick(Duration::from_millis(100));
        let handle = thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let mut access_token = None;
            loop {
                if let Ok(response) = client
                    .post(token_url.to_owned())
                    .body(secret_client_id.to_owned())
                    .send()
                {
                    let status = response.status();
                    if let Ok(t) = response.text() {
                        if status.is_success() {
                            access_token = Some(t);
                        } else if status != StatusCode::NOT_FOUND {
                            bar.println(format!("server returned an error: {}", t));
                        }
                    }
                }
                if access_token.is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(1000));
            }
            let config = if let Ok(Some(mut existing)) = ArchivalConfig::from_fs() {
                existing.access_token = access_token;
                existing
            } else {
                ArchivalConfig { access_token }
            };
            let config_str = toml::to_string(&config).unwrap();
            fs::write(config_file_path, config_str.as_bytes()).expect("failed writing config");
            bar.finish();
        });
        handle.join().unwrap();
        println!("successfully logged in.");
        Ok(ExitStatus::Ok)
    }
}
