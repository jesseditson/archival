#[cfg(feature = "binary")]
use ctrlc;
use futures::executor;
use reqwest;
use std::{
    env,
    error::Error,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::{
    build_site, file_system_mutex::FileSystemMutex, file_system_stdlib, load_site, ArchivalError,
};

static INVALID_COMMAND: &str = "Valid commands are `build` and `run`.";

pub fn binary(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let mut build_dir = env::current_dir()?;
    let _bin_name = args.next();
    if let Some(command_arg) = args.next() {
        let path_arg = args.next();
        if let Some(path) = path_arg {
            build_dir = build_dir.join(path);
        }
        let fs_a = FileSystemMutex::init(file_system_stdlib::NativeFileSystem);

        let site = fs_a.with_fs(|fs| load_site(&build_dir, fs))?;
        match &command_arg[..] {
            "build" => {
                println!("Building site: {}", &site);
                build_site(&site, fs_a)?;
            }
            "run" => {
                println!("Watching site: {}", &site);
                fs_a.clone().with_fs(|fs| {
                    // This won't leak because the process is ended when we
                    // abort anyway
                    _ = fs.watch(
                        site.root.to_owned(),
                        site.manifest.watched_paths(),
                        move |paths| {
                            println!("Changed: {:?}", paths);
                            build_site(&site, fs_a.clone()).unwrap_or_else(|err| {
                                println!("Failed reloading site: {}", err);
                            })
                        },
                    )?;
                    Ok(())
                })?;
                let aborted = Arc::new(AtomicBool::new(false));
                let aborted_clone = aborted.clone();
                ctrlc::set_handler(move || {
                    aborted_clone.store(true, Ordering::SeqCst);
                })?;
                loop {
                    if aborted.load(Ordering::SeqCst) {
                        exit(0);
                    }
                }
            }
            _ => {
                return Err(ArchivalError::new(INVALID_COMMAND).into());
            }
        }
    } else {
        return Err(ArchivalError::new(INVALID_COMMAND).into());
    }
    Ok(())
}

pub fn download_site(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = executor::block_on(reqwest::get(url))?;
    match response.error_for_status() {
        Ok(r) => {
            let r = executor::block_on(r.bytes())?;
            Ok(r.to_vec())
        }
        Err(e) => Err(e),
    }
}
