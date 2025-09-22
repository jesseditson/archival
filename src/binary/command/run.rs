use super::BinaryCommand;
use crate::{file_system::WatchableFileSystemAPI, file_system_stdlib, server, site::Site};
use crate::{FieldConfig, FileSystemAPI};
use clap::{arg, value_parser, ArgMatches};
use std::time::Duration;
use std::{
    path::Path,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};
use tracing::{info, warn};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "run"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("auto-rebuild an archival site")
            .arg(
                arg!(-p --port <port> "static server port")
                    .required(false)
                    .value_parser(value_parser!(u16)),
            )
            .arg(arg!(-n --noserve "disables the static server").required(false))
    }
    fn uses_uploads(&self) -> bool {
        true
    }
    fn handler(
        &self,
        build_dir: &Path,
        args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let mut fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let site = Site::load(
            &fs,
            args.get_one::<String>("upload_prefix").map(|s| s.as_str()),
        )?;
        FieldConfig::set_global(site.get_field_config(None)?);
        let _ = fs.remove_dir_all(&site.manifest.build_dir);
        site.sync_static_files(&mut fs)?;
        if let Err(e) = site.build(&mut fs) {
            println!("Initial build failed: {}", e);
        }
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
        if !args.get_one::<bool>("noserve").unwrap() {
            let mut sb = server::ServerBuilder::new(&path, Some("404.html"));
            if let Some(port) = args.get_one::<u16>("port") {
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
                thread::sleep(Duration::from_millis(500));
                while rx.try_recv().is_ok() {
                    // Flush events
                }
                let mut fs = file_system_stdlib::NativeFileSystem::new(build_dir);
                println!("Rebuilding");
                site.invalidate_file(path.strip_prefix(build_dir)?);
                site.sync_static_files(&mut fs)?;
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
}
