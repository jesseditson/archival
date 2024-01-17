use fs_extra;
use notify::{RecursiveMode, Watcher};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use super::{FileSystemAPI, WatchableFileSystemAPI};

pub struct NativeFileSystem;

impl FileSystemAPI for NativeFileSystem {
    fn remove_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        Ok(fs::remove_dir_all(path)?)
    }
    fn create_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        Ok(fs::create_dir_all(path)?)
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, Box<dyn Error>> {
        let mut files = vec![];
        for f in fs::read_dir(path)? {
            if let Ok(f) = f {
                files.push(f.path());
            }
        }
        Ok(files)
    }
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, Box<dyn Error>> {
        Ok(Some(fs::read_to_string(path)?))
    }
    fn write(&mut self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
        Ok(fs::write(path, contents)?)
    }
    fn copy_contents(&mut self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
        let mut options = fs_extra::dir::CopyOptions::new();
        options.overwrite = true;
        options.content_only = true;
        fs_extra::dir::copy(from, to, &options)?;
        Ok(())
    }
    fn walk_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = PathBuf>>, Box<dyn Error>> {
        let iterator = WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok());
        Ok(Box::new(iterator.map(|de| de.into_path())))
    }
}

impl WatchableFileSystemAPI for NativeFileSystem {
    fn watch(
        &mut self,
        root: PathBuf,
        watch_paths: Vec<String>,
        changed: impl Fn(Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Result<Box<dyn FnOnce()>, Box<dyn Error>> {
        let watch_path = root.to_owned();
        changed(vec![]);
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    let changed_paths: Vec<PathBuf> = event
                        .paths
                        .into_iter()
                        .filter(|p| {
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
                                println!("File changed outside of root: {}", p.display());
                                true
                            }
                        })
                        .collect();
                    if changed_paths.len() > 0 {
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
