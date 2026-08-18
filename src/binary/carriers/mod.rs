//! Server side functions ("carriers") run alongside the dev server.
//!
//! A carrier is a directory under `carriers/` in the site root, served at
//! `/carriers/<name>`. It default-exports `(params, body, objects)`, where
//! `objects` is the site's object tree with `secret` fields included - the
//! whole point of the feature, and the reason this is not on by default.

pub(crate) mod build;
pub(crate) mod discovery;
pub(crate) mod host;
pub(crate) mod node;
pub(crate) mod objects;
pub(crate) mod proxy;

use self::{
    build::Fingerprints,
    discovery::{Carrier, CARRIERS_DIR_NAME},
    host::Sidecar,
    node::NodeInfo,
    objects::CarrierPayload,
    proxy::{CarrierProxy, ProxyState},
};
use anyhow::Result;
use console::style;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex, RwLock},
    thread,
};
use tracing::warn;

/// Tracks what each watched carrier file last contained.
///
/// A carrier's own `build` script writes into the directory being watched, so
/// reacting to every write would rebuild forever. Comparing contents stops that
/// after one pass, and - unlike ignoring changes for a while after a build -
/// never drops a real edit.
#[derive(Default)]
struct SeenContents(BTreeMap<PathBuf, u64>);

impl SeenContents {
    /// True the first time a path is seen, and whenever its contents differ
    /// from last time. A file that has gone away counts as changed.
    fn changed(&mut self, path: &Path) -> bool {
        let hash = std::fs::read(path).map(|bytes| seahash::hash(&bytes)).ok();
        match hash {
            Some(hash) => self.0.insert(path.to_path_buf(), hash) != Some(hash),
            None => self.0.remove(path).is_some(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CarrierOptions {
    /// `--no-carriers`.
    pub disabled: bool,
    /// Extra node flags, e.g. `--inspect`.
    pub node_args: Vec<String>,
    /// A pinned sidecar port, for attaching a debugger.
    pub port: Option<u16>,
}

enum Message {
    /// Carrier code changed (or this is the first run): install, build, restart.
    Rebuild,
    /// The site's objects changed: push them, without restarting.
    Objects(Box<CarrierPayload>),
}

pub(crate) struct CarrierSupervisor {
    to_worker: mpsc::Sender<Message>,
    state: Arc<RwLock<ProxyState>>,
    sidecar: Arc<Mutex<Option<Sidecar>>>,
    seen: Arc<Mutex<SeenContents>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl CarrierSupervisor {
    /// `None` when this site has no carriers, or they were turned off. The dev
    /// server then behaves exactly as it does without the feature.
    pub fn new(site_root: &Path, options: CarrierOptions) -> Option<Self> {
        if options.disabled {
            return None;
        }
        match discovery::discover(site_root) {
            Ok(carriers) if carriers.is_empty() => None,
            Ok(_) => Some(Self::start(site_root.to_path_buf(), options)),
            Err(e) => {
                warn!("couldn't read {}/: {}", CARRIERS_DIR_NAME, e);
                None
            }
        }
    }

    fn start(site_root: PathBuf, options: CarrierOptions) -> Self {
        let state = Arc::new(RwLock::new(ProxyState::Starting));
        let sidecar = Arc::new(Mutex::new(None));
        let seen = Arc::new(Mutex::new(SeenContents::default()));
        let (to_worker, inbox) = mpsc::channel();
        let worker = Worker {
            site_root,
            options,
            state: state.clone(),
            sidecar: sidecar.clone(),
            fingerprints: Fingerprints::default(),
            entries: BTreeMap::new(),
            payload: None,
        };
        let handle = thread::spawn(move || worker.run(inbox));
        // Everything up to here is cheap; installing and spawning node happens
        // on the worker so the dev server can start serving immediately.
        let supervisor = Self {
            to_worker,
            state,
            sidecar,
            seen,
            worker: Some(handle),
        };
        supervisor.send(Message::Rebuild);
        supervisor
    }

    pub fn proxy(&self) -> Arc<CarrierProxy> {
        Arc::new(CarrierProxy::new(self.state.clone()))
    }

    /// The sidecar, so a ctrl-C handler can kill it before `exit`.
    pub fn child(&self) -> Arc<Mutex<Option<Sidecar>>> {
        self.sidecar.clone()
    }

    /// Whether a changed path is a carrier's concern. Paths under `carriers/`
    /// are always claimed - they must never reach the site's cache invalidation,
    /// or an `npm install` would trigger a full rebuild per written file.
    pub fn claims(&self, changed: &Path) -> bool {
        changed.starts_with(CARRIERS_DIR_NAME)
    }

    /// Whether a claimed path should actually cause a restart. `changed` is
    /// relative to the site root.
    pub fn should_rebuild(&self, root: &Path, changed: &Path) -> bool {
        let ignored = changed.components().any(|c| {
            let name = c.as_os_str().to_string_lossy();
            name == "node_modules" || name.starts_with('.')
        });
        if ignored {
            return false;
        }
        self.seen.lock().unwrap().changed(&root.join(changed))
    }

    pub fn rebuild(&self) {
        self.send(Message::Rebuild);
    }

    pub fn set_objects(&self, payload: CarrierPayload) {
        self.send(Message::Objects(Box::new(payload)));
    }

    fn send(&self, message: Message) {
        if self.to_worker.send(message).is_err() {
            warn!("the carrier worker is gone; carriers will not update");
        }
    }

    pub fn shutdown(&mut self) {
        if let Ok(mut sidecar) = self.sidecar.lock() {
            if let Some(sidecar) = sidecar.as_mut() {
                sidecar.shutdown();
            }
        }
    }
}

impl Drop for CarrierSupervisor {
    fn drop(&mut self) {
        self.shutdown();
        // Dropping the sender ends the worker's loop.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Worker {
    site_root: PathBuf,
    options: CarrierOptions,
    state: Arc<RwLock<ProxyState>>,
    sidecar: Arc<Mutex<Option<Sidecar>>>,
    fingerprints: Fingerprints,
    entries: BTreeMap<String, PathBuf>,
    payload: Option<CarrierPayload>,
}

impl Worker {
    fn run(mut self, inbox: mpsc::Receiver<Message>) {
        let fingerprints_path =
            host::harness_dir(&self.site_root).join("install-fingerprints.json");
        self.fingerprints = Fingerprints::load(&fingerprints_path);
        while let Ok(message) = inbox.recv() {
            match message {
                Message::Rebuild => {
                    if let Err(e) = self.rebuild() {
                        self.fail(e);
                    }
                    self.fingerprints.save(&fingerprints_path);
                }
                Message::Objects(payload) => {
                    self.payload = Some(*payload);
                    if let Err(e) = self.push_state() {
                        warn!("couldn't send the site's objects to a carrier: {}", e);
                    }
                }
            }
        }
    }

    fn fail(&self, error: anyhow::Error) {
        let message = format!("{}", error);
        println!(
            "{} {}",
            style("Carriers failed:").red(),
            style(&message).red()
        );
        *self.state.write().unwrap() = ProxyState::Failed(message);
        *self.sidecar.lock().unwrap() = None;
    }

    fn rebuild(&mut self) -> Result<()> {
        let node = NodeInfo::detect()?;
        let carriers = discovery::discover(&self.site_root)?;
        self.entries.clear();
        for carrier in &carriers {
            match self.prepare(&node, carrier) {
                Ok(entry) => {
                    self.entries.insert(carrier.name.to_owned(), entry);
                }
                // One broken carrier must not stop the others from running.
                Err(e) => println!(
                    "{} {}",
                    style("Carrier skipped:").yellow(),
                    style(e).yellow()
                ),
            }
        }
        let harness = host::write_harness(&host::harness_dir(&self.site_root))?;
        // Node's module cache cannot be invalidated, so changed code means a
        // new process. Booting one costs about as much as an import.
        *self.sidecar.lock().unwrap() = None;
        let sidecar = Sidecar::spawn(&node, &harness, &self.options.node_args, self.options.port)?;
        sidecar.wait_until_healthy()?;
        *self.sidecar.lock().unwrap() = Some(sidecar);
        self.push_state()?;
        *self.state.write().unwrap() = ProxyState::Ready {
            port: self
                .sidecar
                .lock()
                .unwrap()
                .as_ref()
                .expect("just stored")
                .port(),
        };
        println!(
            "{} {}",
            style("Carriers ready:").green(),
            style(
                self.entries
                    .keys()
                    .map(|n| format!("/carriers/{}", n))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .green()
        );
        Ok(())
    }

    fn prepare(&mut self, node: &NodeInfo, carrier: &Carrier) -> Result<PathBuf> {
        let entry = build::prepare(carrier, &mut self.fingerprints)?;
        if discovery::is_typescript(&entry) && node.strip_types_flags().is_none() {
            return Err(node.typescript_unsupported(&carrier.name));
        }
        Ok(entry)
    }

    fn push_state(&self) -> Result<()> {
        let sidecar = self.sidecar.lock().unwrap();
        let (Some(sidecar), Some(payload)) = (sidecar.as_ref(), &self.payload) else {
            // Either nothing is running yet, or the first build hasn't produced
            // objects. Whichever comes second pushes.
            return Ok(());
        };
        sidecar.push_state(&self.entries, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::TempDir;

    #[test]
    fn a_new_file_counts_as_changed_once() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("index.js");
        write(&file, "a").unwrap();
        let mut seen = SeenContents::default();
        assert!(seen.changed(&file));
        assert!(!seen.changed(&file), "an unchanged file must not rebuild");
    }

    #[test]
    fn identical_contents_do_not_rebuild() {
        // This is what stops a carrier's own `build` script from looping: it
        // rewrites the same bytes, and the second pass is a no-op.
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("index.js");
        write(&file, "a").unwrap();
        let mut seen = SeenContents::default();
        assert!(seen.changed(&file));
        write(&file, "a").unwrap();
        assert!(!seen.changed(&file));
        write(&file, "b").unwrap();
        assert!(seen.changed(&file), "a real edit always rebuilds");
    }

    #[test]
    fn a_deleted_file_counts_as_changed_once() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("index.js");
        write(&file, "a").unwrap();
        let mut seen = SeenContents::default();
        assert!(seen.changed(&file));
        std::fs::remove_file(&file).unwrap();
        assert!(seen.changed(&file));
        assert!(!seen.changed(&file), "still gone is not a change");
    }
}
