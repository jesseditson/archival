use crate::{file_system::WatchableFileSystemAPI, file_system_stdlib, server, site::Site};
use clap::{arg, command, value_parser, Command};
use ctrlc;
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
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn binary(mut _args: impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

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
        );
    let matches = cmd.get_matches();
    if let Some(build) = matches.subcommand_matches("build") {
        if let Some(path) = build.get_one::<PathBuf>("path") {
            build_dir = fs::canonicalize(build_dir.join(path))?;
        }
        let mut fs = file_system_stdlib::NativeFileSystem::new(&build_dir);
        let site = Site::load(&fs)?;
        println!("Building site: {}", &site);
        site.build(&mut fs)
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
    } else {
        panic!("No command provided.");
    }
}
