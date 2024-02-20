use crate::{file_system::WatchableFileSystemAPI, file_system_stdlib, server, site::Site};
use clap::{arg, command, value_parser, Command};
use ctrlc;
use semver::{Version, VersionReq};
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

const MIN_VERSION: &str = ">=0.4.1";

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
        let req = VersionReq::parse(MIN_VERSION).unwrap();
        let version_string = compat.get_one::<String>("version").unwrap();
        match Version::parse(version_string) {
            Ok(version) => {
                if req.matches(&version) {
                    println!("passed compatibility check.");
                    Ok(ExitStatus::OK)
                } else {
                    println!("version {} is incompatible with this version of archival (minimum required version {}).", version, MIN_VERSION);
                    Ok(ExitStatus::ERROR)
                }
            }
            Err(e) => {
                println!("invalid version {}: {}", version_string, e);
                Ok(ExitStatus::ERROR)
            }
        }
    } else {
        panic!("No command provided.");
    }
}
