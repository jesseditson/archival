use crate::{
    file_system::{FileSystemAPI, WatchableFileSystemAPI},
    ArchivalError,
};
use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
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

    fn get_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        let rel_path = rel.as_ref();
        if rel_path.is_absolute() {
            // If the path is already inside root, use it as-is (e.g. an
            // absolute build-dir passed via CLI). Otherwise treat it as a
            // root-relative absolute path (e.g. "/pages/foo.liquid").
            if rel_path.starts_with(&self.root) {
                return rel_path.to_path_buf();
            }
            let normalized = rel_path.strip_prefix("/").unwrap_or(rel_path);
            self.root.join(normalized)
        } else {
            self.root.join(rel_path)
        }
    }
}

impl FileSystemAPI for NativeFileSystem {
    fn root_dir(&self) -> &Path {
        &self.root
    }
    fn exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        Ok(fs::metadata(self.get_path(path)).is_ok())
    }
    fn is_dir(&self, path: impl AsRef<Path>) -> Result<bool> {
        Ok(self.get_path(path).is_dir())
    }
    fn remove_dir_all(&mut self, path: impl AsRef<Path>) -> Result<()> {
        Ok(fs::remove_dir_all(self.get_path(path))?)
    }
    fn create_dir_all(&mut self, path: impl AsRef<Path>) -> Result<()> {
        Ok(fs::create_dir_all(self.get_path(path))?)
    }
    fn read(&self, path: impl AsRef<Path>) -> Result<Option<Vec<u8>>> {
        Ok(Some(fs::read(self.get_path(path))?))
    }
    fn read_to_string(&self, path: impl AsRef<Path>) -> Result<Option<String>> {
        Ok(Some(fs::read_to_string(self.get_path(path))?))
    }
    fn delete(&mut self, path: impl AsRef<Path>) -> Result<()> {
        if self.is_dir(&path)? {
            return Err(ArchivalError::new("use remove_dir_all to delete directories").into());
        }
        Ok(fs::remove_file(self.get_path(path))?)
    }
    fn write_str(&mut self, path: impl AsRef<Path>, contents: String) -> Result<()> {
        let full = self.get_path(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(fs::write(full, contents)?)
    }
    fn write(&mut self, path: impl AsRef<Path>, contents: Vec<u8>) -> Result<()> {
        let full = self.get_path(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(fs::write(full, contents)?)
    }
    fn rename(&mut self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
        Ok(fs::rename(self.get_path(from), self.get_path(to))?)
    }
    fn list_dir(
        &self,
        path: impl AsRef<Path>,
        recursive: bool,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>> {
        if recursive {
            self.walk_dir(path, true)
        } else {
            let files_iter = fs::read_dir(self.get_path(path))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    !p.file_name()
                        .is_some_and(|f| f.to_string_lossy().starts_with("."))
                });
            Ok(Box::new(files_iter))
        }
    }
    fn walk_dir(
        &self,
        path: impl AsRef<Path>,
        include_dirs: bool,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>> {
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
            .filter_map(move |e| {
                if e.path() == root {
                    None
                } else {
                    e.into_path()
                        .strip_prefix(&root)
                        .ok()
                        .map(|p| p.to_path_buf())
                }
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
    ) -> Result<Box<dyn FnOnce()>> {
        let root = fs::canonicalize(root).unwrap();
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

impl std::fmt::Display for NativeFileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.walk_dir("", false) {
            Ok(paths) => {
                write!(
                    f,
                    "{}:\n\t{}",
                    self.root_dir().display(),
                    paths
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("\n\t")
                )
            }
            Err(e) => write!(f, "{}: {}", self.root_dir().display(), e),
        }
    }
}
