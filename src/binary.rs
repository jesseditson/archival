use ctrlc;
use std::{
    env,
    error::Error,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tracing::{debug, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::{
    file_system::WatchableFileSystemAPI, file_system_mutex::FileSystemMutex, file_system_stdlib,
    site, ArchivalError,
};

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
        build_dir = build_dir.join(path);
    }
    let fs_a = FileSystemMutex::init(file_system_stdlib::NativeFileSystem::new(&build_dir));
    fs_a.with_fs(|fs| {
        let site = site::load(fs)?;
        match &command_arg[..] {
            "build" => {
                println!("Building site: {}", &site);
                site::build(&site, fs)
            }
            "run" => {
                println!("Watching site: {}", &site);
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                // This won't leak because the process is ended when we
                // abort anyway
                _ = fs.watch(
                    fs.root.to_owned(),
                    site.manifest.watched_paths(),
                    move |paths| {
                        debug!("Changed: {:?}", paths);
                        if let Err(e) = tx.send(0) {
                            warn!("Failed sending change event: {}", e);
                        }
                    },
                )?;
                let aborted = Arc::new(AtomicBool::new(false));
                let aborted_clone = aborted.clone();
                ctrlc::set_handler(move || {
                    aborted_clone.store(true, Ordering::SeqCst);
                })?;
                loop {
                    if rx.try_recv().is_ok() {
                        if let Err(e) = site::build(&site, fs) {
                            warn!("Build failed: {}", e);
                        }
                    }
                    if aborted.load(Ordering::SeqCst) {
                        exit(0);
                    }
                }
            }
            _ => Err(ArchivalError::new(&invalid_command_msg).into()),
        }
    })
}
