use crate::{
    file_system::{FileSystemAPI, WatchableFileSystemAPI},
    ArchivalError,
};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};
#[cfg(feature = "verbose-logging")]
use tracing::debug;
use tracing::warn;
use walkdir::WalkDir;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeFileSystem {
    pub root: PathBuf,
}

impl NativeFileSystem {
    pub fn new(root: &Path) -> Self {
        Self {
            root: Path::new(root).to_owned(),
        }
    }

    fn get_path(&self, rel: &Path) -> PathBuf {
        self.root.join(rel)
    }
}

impl FileSystemAPI for NativeFileSystem {
    fn root_dir(&self) -> &Path {
        &self.root
    }
    fn exists(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        Ok(fs::metadata(self.get_path(path)).is_ok())
    }
    fn is_dir(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        Ok(self.get_path(path).is_dir())
    }
    fn remove_dir_all(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        Ok(fs::remove_dir_all(self.get_path(path))?)
    }
    fn create_dir_all(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        Ok(fs::create_dir_all(self.get_path(path))?)
    }
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        Ok(Some(fs::read(self.get_path(path))?))
    }
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, Box<dyn Error>> {
        Ok(Some(fs::read_to_string(self.get_path(path))?))
    }
    fn delete(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        if self.is_dir(path)? {
            return Err(ArchivalError::new("use remove_dir_all to delete directories").into());
        }
        Ok(fs::remove_file(self.get_path(path))?)
    }
    fn write_str(&mut self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
        Ok(fs::write(self.get_path(path), contents)?)
    }
    fn write(&mut self, path: &Path, contents: Vec<u8>) -> Result<(), Box<dyn Error>> {
        Ok(fs::write(self.get_path(path), contents)?)
    }
    fn walk_dir(
        &self,
        path: &Path,
        include_dirs: bool,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>, Box<dyn Error>> {
        let root = self.get_path(path);
        let iterator = WalkDir::new(&root)
            .follow_links(true)
            .into_iter()
            .filter(move |e| {
                if include_dirs {
                    true
                } else {
                    e.as_ref().is_ok_and(|de| de.file_type().is_file())
                }
            })
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_name().to_string_lossy().starts_with("."))
            .filter_map(move |e| {
                e.into_path()
                    .strip_prefix(&root)
                    .ok()
                    .map(|p| p.to_path_buf())
            });
        Ok(Box::new(iterator))
    }
}

impl WatchableFileSystemAPI for NativeFileSystem {
    fn watch(
        &self,
        root: PathBuf,
        watch_paths: Vec<String>,
        changed: impl Fn(Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Result<Box<dyn FnOnce()>, Box<dyn Error>> {
        let root = fs::canonicalize(self.get_path(&root)).unwrap();
        let watch_path = root.to_owned();
        changed(vec![watch_path.to_path_buf()]);
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    let changed_paths: Vec<PathBuf> = event
                        .paths
                        .into_iter()
                        .filter(|p| {
                            if p.file_name()
                                .is_some_and(|f| f.to_string_lossy().starts_with("."))
                            {
                                return false;
                            }
                            let p = if let Ok(f) = fs::canonicalize(p) {
                                f
                            } else {
                                #[cfg(feature = "verbose-logging")]
                                debug!("Invalid path {}", p.display());
                                return false;
                            };
                            if let Ok(rel) = p.strip_prefix(&root) {
                                for dir in &watch_paths {
                                    let mut dir = dir.to_string();
                                    if let Ok(stripped) = Path::new(&dir).strip_prefix(&root) {
                                        dir = stripped.to_string_lossy().into_owned();
                                    }
                                    if rel.starts_with(dir) {
                                        return true;
                                    }
                                }
                                false
                            } else {
                                warn!(
                                    "File changed outside of root ({}): {}",
                                    root.display(),
                                    p.display()
                                );
                                true
                            }
                        })
                        .collect();
                    if !changed_paths.is_empty() {
                        changed(changed_paths);
                    }
                }
                Err(e) => println!("watch error: {:?}", e),
            },
        )?;

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(&watch_path, RecursiveMode::Recursive)?;
        let path = watch_path.to_owned();
        let unwatch = move || {
            watcher.unwatch(&path).unwrap();
        };
        Ok(Box::new(unwatch))
    }
}
