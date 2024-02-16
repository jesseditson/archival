use crate::{file_system::WatchableFileSystemAPI, file_system_stdlib, site::Site, ArchivalError};
use ctrlc;
use std::{
    env,
    error::Error,
    fs,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static VALID_COMMANDS: &[&str] = &["build", "run"];

pub fn binary(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let mut build_dir = env::current_dir()?;
    let _bin_name = args.next();
    let invalid_command_msg = format!(
        "Valid commands are {}.",
        VALID_COMMANDS
            .iter()
            .map(|c| format!("`{}`", c))
            .collect::<Vec<String>>()
            .join(", ")
    );
    let command_arg = if let Some(cmd) = args.next() {
        if VALID_COMMANDS.contains(&&cmd[..]) {
            cmd
        } else {
            return Err(ArchivalError::new(&invalid_command_msg).into());
        }
    } else {
        return Err(ArchivalError::new(&invalid_command_msg).into());
    };
    let path_arg = args.next();
    if let Some(path) = path_arg {
        build_dir = fs::canonicalize(build_dir.join(path))?;
    }
    match &command_arg[..] {
        "build" => {
            let mut fs = file_system_stdlib::NativeFileSystem::new(&build_dir);
            let site = Site::load(&fs)?;
            println!("Building site: {}", &site);
            site.build(&mut fs)
        }
        "run" => {
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
            let aborted = Arc::new(AtomicBool::new(false));
            let aborted_clone = aborted.clone();
            ctrlc::set_handler(move || {
                aborted_clone.store(true, Ordering::SeqCst);
                exit(0);
            })?;
            loop {
                if let Ok(path) = rx.try_recv() {
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
        }
        _ => Err(ArchivalError::new(&invalid_command_msg).into()),
    }
}
