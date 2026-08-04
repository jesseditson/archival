use super::BinaryCommand;
use crate::{
    binary::{
        command::{add_args, CommandConfig},
        ArchivalConfig, ExitStatus,
    },
    constants::{API_URL, AUTH_URL, CLI_TOKEN_PUBLIC_KEY},
};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clap::ArgMatches;
use indicatif::{ProgressBar, ProgressStyle};
use nanoid::nanoid;
use reqwest::StatusCode;
use rsa::{pkcs8::DecodePublicKey, sha2::Sha256, Oaep, RsaPublicKey};
use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "login"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        add_args(
            cmd.about("Log in to archival.dev and store credentials locally in ~/.archivalrc"),
            CommandConfig::default(),
        )
    }
    fn handler(
        &self,
        _args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus> {
        let secret_client_id = nanoid!(21);
        let public_key = RsaPublicKey::from_public_key_pem(CLI_TOKEN_PUBLIC_KEY).unwrap();
        let mut rng = rand::thread_rng();
        let padding = Oaep::new::<Sha256>();
        let enc_data = public_key.encrypt(&mut rng, padding, secret_client_id.as_bytes())?;
        let b64_encoded = STANDARD.encode(enc_data);
        let auth_url = format!(
            "{}?cli-code={}",
            AUTH_URL,
            urlencoding::encode(&b64_encoded)
        );
        let token_url = format!("{}/cli-token", API_URL);
        println!("To log in, open this URL in your browser\n{}", auth_url);
        let bar = ProgressBar::new_spinner();
        bar.set_style(ProgressStyle::with_template("{msg} {spinner}").unwrap());
        bar.set_message("waiting for login to complete");
        bar.enable_steady_tick(Duration::from_millis(100));
        let handle = thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            loop {
                if let Ok(response) = client
                    .post(token_url.to_owned())
                    .body(secret_client_id.to_owned())
                    .send()
                {
                    let status = response.status();
                    if let Ok(t) = response.text() {
                        if status.is_success() {
                            bar.finish();
                            return t;
                        } else if status != StatusCode::NOT_FOUND {
                            bar.println(format!("server returned an error: {}", t));
                        }
                    }
                }
                thread::sleep(Duration::from_millis(1000));
            }
        });
        let access_token = handle.join().unwrap();
        ArchivalConfig::set_access_token(&access_token)?;
        println!("successfully logged in.");
        Ok(ExitStatus::Ok)
    }
}
