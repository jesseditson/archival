use crate::{file_system::WatchableFileSystemAPI, file_system_stdlib, server, site::Site};
use console::{style, Term};
use indicatif::ProgressBar;
use rsa::pkcs8::der::Writer;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, RwLock};
use std::time::{Duration, Instant};
use std::{process::exit, sync::atomic::Ordering, thread};
use tracing::warn;

#[derive(Debug, Default)]
pub enum DevServerMode {
    #[default]
    NoServe,
    Serve(Option<u16>),
}

#[derive(Debug, Default)]
pub struct UploadsConfig<'a> {
    pub url: Option<&'a str>,
    pub prefix: Option<&'a str>,
}

impl<'a> UploadsConfig<'a> {
    pub fn prefix(prefix: &'a str) -> Self {
        UploadsConfig {
            url: None,
            prefix: Some(prefix),
        }
    }
    pub fn new(url: &'a str, prefix: &'a str) -> Self {
        UploadsConfig {
            url: Some(url),
            prefix: Some(prefix),
        }
    }
}

pub fn watch(
    root_dir: PathBuf,
    uploads_config: UploadsConfig,
    mode: DevServerMode,
    change_sender: Option<mpsc::Sender<Vec<PathBuf>>>,
    watch_paths: Option<Vec<String>>,
    quit: Arc<AtomicBool>,
) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
    let mut term = Term::stdout();
    let is_interactive = term.features().is_attended();
    let mut fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
    let mut site = Site::load(&fs, uploads_config.prefix)?;
    if let Some(uploads_url) = uploads_config.url {
        site.modify_manifest(&mut fs, |manifest| {
            manifest.uploads_url = Some(uploads_url.to_string());
        })?;
    }
    site.sync_static_files(&mut fs)?;
    let (tx, rx) = mpsc::channel();
    let initial_build = site.build(&mut fs);
    let mut init_message = format!("Watching site: {}", &site);
    let change_queue = Arc::new(RwLock::new(vec![]));
    let queue_changes = change_sender.is_some();
    // This won't leak because the process is ended when we
    // abort anyway
    let watcher_queue = change_queue.clone();
    let mut merged_watch_paths = watch_paths.unwrap_or_default();
    merged_watch_paths.append(&mut site.manifest.watched_paths());
    let kill_watcher = fs.watch(fs.root.to_owned(), merged_watch_paths, move |paths| {
        for path in paths {
            if queue_changes {
                watcher_queue.write().unwrap().push(path.clone());
            }
            if let Err(e) = tx.send(path.clone()) {
                warn!("Failed sending change event to builder: {}", e);
            }
        }
    })?;
    let path = root_dir.join(&site.manifest.build_dir);
    if let DevServerMode::Serve(port) = mode {
        let mut sb = server::ServerBuilder::new(&path, Some("404.html"));
        if let Some(port) = port {
            sb.port(port);
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
                site.invalidate_file(path.strip_prefix(&root_dir).unwrap());
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
                if is_interactive {
                    term.clear_screen()?;
                }
                term.write(init_message.as_bytes())?;
                bar.set_message("Rebuilding...");
                Some(bar)
            } else {
                println!("Rebuilding...");
                None
            };
            let mut fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
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
                if let Some(cs) = &change_sender {
                    let mut guard = change_queue.write().unwrap();
                    let q = std::mem::take(&mut *guard);
                    if let Err(e) = cs.send(q) {
                        warn!("Failed sending change event to host: {}", e);
                    }
                }
            } else {
                println!("{}", output);
            }
        }
    }
}
