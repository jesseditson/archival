use super::BinaryCommand;
use crate::{binary::ExitStatus, constants::AUTH_URL};
use clap::ArgMatches;
use indicatif::{ProgressBar, ProgressStyle};
use nanoid::nanoid;
use reqwest::StatusCode;
use std::{fs, path::Path, thread, time::Duration};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "login"
    }
    fn has_path(&self) -> bool {
        false
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("Log in to archival.dev and store credentials locally in ~/.archivalrc")
    }
    fn handler(
        &self,
        _build_dir: &Path,
        _args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let config_file = Path::join(
            &home::home_dir().expect("unable to determine $HOME"),
            ".archivalrc",
        );
        let login_token = nanoid!(21);
        let auth_url = format!("{}?code={}", AUTH_URL, login_token);
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
                    .body(login_token.to_owned())
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
            let access_token = access_token.unwrap();
            fs::write(config_file, format!("access_token={}", access_token))
                .expect("failed writing config");
            bar.finish();
        });
        handle.join().unwrap();
        println!("successfully logged in.");
        Ok(ExitStatus::Ok)
    }
}
