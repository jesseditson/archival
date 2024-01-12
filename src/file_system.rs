use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub trait FileSystemAPI {
    fn remove_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>>;
    fn create_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>>;
    fn read_to_string(&self, path: &Path) -> Result<String, Box<dyn Error>>;
    fn write(&self, path: &Path, contents: String) -> Result<(), Box<dyn Error>>;
    fn copy_contents(&self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>>;
    fn walk_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = PathBuf>>, Box<dyn Error>>;
}

pub trait WatchableFileSystemAPI {
    fn watch(
        self,
        root: PathBuf,
        watch_paths: Vec<String>,
        changed: impl Fn(Arc<Mutex<dyn FileSystemAPI>>, Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Result<Box<dyn FnOnce()>, Box<dyn Error>>;
}
