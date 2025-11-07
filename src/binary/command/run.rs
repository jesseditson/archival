use super::BinaryCommand;
use crate::FieldConfig;
use crate::{file_system::WatchableFileSystemAPI, file_system_stdlib, server, site::Site};
use clap::{arg, value_parser, ArgMatches};
use console::{style, Term};
use indicatif::ProgressBar;
use rsa::pkcs8::der::Writer;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{
    path::Path,
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};
use tracing::warn;

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
        quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let mut term = Term::stdout();
        let is_interactive = term.features().is_attended();
        let mut fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let site = Site::load(
            &fs,
            args.get_one::<String>("upload-prefix").map(|s| s.as_str()),
        )?;
        FieldConfig::set_global(site.get_field_config(None)?);
        site.sync_static_files(&mut fs)?;
        let initial_build = site.build(&mut fs);
        let mut init_message = format!("Watching site: {}", &site);
        let (tx, rx) = mpsc::channel();
        // This won't leak because the process is ended when we
        // abort anyway
        let kill_watcher = fs.watch(
            fs.root.to_owned(),
            site.manifest.watched_paths(),
            move |paths| {
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
            init_message += &format!("Serving {}\n", path.display());
            init_message += &format!("See http://{}\n", server.addr());
            init_message += "Hit CTRL-C to stop\n";
            thread::spawn(move || {
                server.serve().unwrap();
            });
        }
        let quit_clone = quit.clone();
        ctrlc::set_handler(move || {
            quit_clone.store(true, Ordering::SeqCst);
            exit(0);
        })?;
        let mut last_build = Instant::now();
        let mut changed = false;
        if is_interactive {
            term.clear_screen()?;
        }
        term.write(init_message.as_bytes())?;
        if let Err(e) = initial_build {
            let bar = ProgressBar::new_spinner();
            bar.finish_with_message(format!("Initial build failed: {}", e));
        }
        loop {
            match rx.try_recv() {
                Ok(path) => {
                    site.invalidate_file(path.strip_prefix(build_dir).unwrap());
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("Build Channel Disconnected.")
                }
            }
            if quit.load(Ordering::SeqCst) {
                kill_watcher();
                exit(0);
            }
            // Batch changes every 200ms
            if changed && Instant::now() - last_build > Duration::from_millis(200) {
                last_build = Instant::now();
                changed = false;
                let bar = if is_interactive {
                    let bar = ProgressBar::new_spinner();
                    bar.enable_steady_tick(Duration::from_millis(100));
                    term.clear_screen()?;
                    term.write(init_message.as_bytes())?;
                    bar.set_message("Rebuilding...");
                    Some(bar)
                } else {
                    println!("Rebuilding...");
                    None
                };
                let mut fs = file_system_stdlib::NativeFileSystem::new(build_dir);
                site.sync_static_files(&mut fs).unwrap();
                let output = if let Err(e) = site.build(&mut fs) {
                    format!("{} {}", style("Build failed:").red(), style(e).red())
                } else {
                    format!(
                        "{} {:?}",
                        style("Rebuilt in").green(),
                        style(Instant::now() - last_build).green()
                    )
                };
                if let Some(bar) = bar {
                    bar.finish_with_message(output);
                } else {
                    println!("{}", output);
                }
            }
        }
    }
}
