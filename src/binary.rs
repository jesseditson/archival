use crate::{
    check_compatibility, constants::AUTH_URL, file_system::WatchableFileSystemAPI,
    file_system_stdlib, server, site::Site,
};
use clap::{arg, command, value_parser, Command};
use ctrlc;
use indicatif::{ProgressBar, ProgressStyle};
use nanoid::nanoid;
use reqwest::StatusCode;
use std::path::Path;
use std::time::Duration;
use std::{
    env,
    error::Error,
    fs,
    path::PathBuf,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread, time,
};
use tracing::{info, warn};

pub enum ExitStatus {
    ERROR,
    OK,
}

pub fn binary(args: impl Iterator<Item = String>) -> Result<ExitStatus, Box<dyn Error>> {
    let mut build_dir = env::current_dir()?;
    let cmd = command!()
        .arg_required_else_help(true)
        .subcommand(
            Command::new("build").about("builds an archival site").arg(
                arg!([path] "an optional path to build")
                    .required(false)
                    .value_parser(value_parser!(PathBuf)),
            ),
        )
        .subcommand(
            Command::new("run")
                .about("auto-rebuild an archival site")
                .arg(
                    arg!(-p --port <port> "static server port")
                        .required(false)
                        .value_parser(value_parser!(u16)),
                )
                .arg(arg!(-n --noserve "disables the static server").required(false))
                .arg(
                    arg!([path] "an optional path to build")
                        .required(false)
                        .value_parser(value_parser!(PathBuf)),
                ),
        )
        .subcommand(
            Command::new("compat")
                .about(
                    "checks the compatibility of this version of archival against a version string",
                )
                .arg(
                    arg!([version] "a version string (e.g. 0.4.1-alpha)")
                        .required(true)
                        .value_parser(value_parser!(String)),
                ),
        )
        .subcommand(
            Command::new("login")
                .about("Log in to archival.dev and store credentials locally in ~/.archivalrc"),
        );
    let matches = cmd.get_matches_from(args);
    if let Some(build) = matches.subcommand_matches("build") {
        if let Some(path) = build.get_one::<PathBuf>("path") {
            build_dir = fs::canonicalize(build_dir.join(path))?;
        }
        let mut fs = file_system_stdlib::NativeFileSystem::new(&build_dir);
        let site = Site::load(&fs)?;
        println!("Building site: {}", &site);
        site.build(&mut fs)?;
        Ok(ExitStatus::OK)
    } else if let Some(run) = matches.subcommand_matches("run") {
        if let Some(path) = run.get_one::<PathBuf>("path") {
            build_dir = fs::canonicalize(build_dir.join(path))?;
        }
        let fs = file_system_stdlib::NativeFileSystem::new(&build_dir);
        let site = Site::load(&fs)?;
        println!("Watching site: {}", &site);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // This won't leak because the process is ended when we
        // abort anyway
        let kill_watcher = fs.watch(
            fs.root.to_owned(),
            site.manifest.watched_paths(),
            move |paths| {
                info!("changed: {:?}", paths);
                for path in paths {
                    if let Err(e) = tx.send(path) {
                        warn!("Failed sending change event: {}", e);
                    }
                }
            },
        )?;
        let path = build_dir.join(&site.manifest.build_dir);
        if !run.get_one::<bool>("noserve").unwrap() {
            let mut sb = server::ServerBuilder::new(&path, Some("404.html"));
            if let Some(port) = run.get_one::<u16>("port") {
                sb.port(*port);
            }
            let server = sb.build();
            println!("Serving {}", path.display());
            println!("See http://{}", server.addr());
            println!("Hit CTRL-C to stop");
            thread::spawn(move || {
                server.serve().unwrap();
            });
        }
        let aborted = Arc::new(AtomicBool::new(false));
        let aborted_clone = aborted.clone();
        ctrlc::set_handler(move || {
            aborted_clone.store(true, Ordering::SeqCst);
            exit(0);
        })?;
        loop {
            if let Ok(path) = rx.try_recv() {
                // Batch changes every 500ms
                thread::sleep(time::Duration::from_millis(500));
                while rx.try_recv().is_ok() {
                    // Flush events
                }
                let mut fs = file_system_stdlib::NativeFileSystem::new(&build_dir);
                println!("Rebuilding");
                site.invalidate_file(path.strip_prefix(&build_dir)?);
                if let Err(e) = site.build(&mut fs) {
                    println!("Build failed: {}", e);
                } else {
                    println!("Rebuilt.");
                }
            }
            if aborted.load(Ordering::SeqCst) {
                kill_watcher();
                exit(0);
            }
        }
    } else if let Some(compat) = matches.subcommand_matches("compat") {
        let (compat, message) = check_compatibility(compat.get_one::<String>("version").unwrap());
        println!("{}", message);
        match compat {
            true => Ok(ExitStatus::OK),
            false => Ok(ExitStatus::ERROR),
        }
    } else if let Some(_l) = matches.subcommand_matches("login") {
        login();
        println!("successfully logged in.");
        Ok(ExitStatus::OK)
    } else {
        panic!("No command provided.");
    }
}

fn login() {
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
    handle.join().unwrap()
}
