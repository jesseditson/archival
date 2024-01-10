use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub use native::NativeFileSystem;

pub trait FileSystemAPI {
    fn remove_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>>;
    fn create_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>>;
    fn read_to_string(&self, path: &Path) -> Result<String, Box<dyn Error>>;
    fn write(&self, path: &Path, contents: String) -> Result<(), Box<dyn Error>>;
    fn copy_contents(&self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>>;
}

pub trait WatchableFileSystemAPI {
    fn watch(
        self,
        path: PathBuf,
        ignore: Vec<String>,
        changed: impl Fn(Arc<Mutex<dyn FileSystemAPI>>, Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Result<Box<dyn FnOnce()>, Box<dyn Error>>;
}

mod native {
    use fs_extra;
    use notify::{RecursiveMode, Watcher};
    use std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

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
        fn read_to_string(&self, path: &Path) -> Result<String, Box<dyn Error>> {
            Ok(fs::read_to_string(path)?)
        }
        fn write(&self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
            Ok(fs::write(path, contents)?)
        }
        fn copy_contents(&self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
            let mut options = fs_extra::dir::CopyOptions::new();
            options.overwrite = true;
            options.content_only = true;
            fs_extra::dir::copy(from, to, &options)?;
            Ok(())
        }
    }

    impl WatchableFileSystemAPI for NativeFileSystem {
        fn watch(
            self,
            path: PathBuf,
            ignore: Vec<String>,
            changed: impl Fn(Arc<Mutex<dyn FileSystemAPI>>, Vec<PathBuf>) + Send + Sync + 'static,
        ) -> Result<Box<dyn FnOnce()>, Box<dyn Error>> {
            // Automatically select the best implementation for your platform.
            let self_box = Arc::new(Mutex::new(self));
            let watch_path = path.to_owned();
            let mut watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                    match res {
                        Ok(event) => {
                            let paths: Vec<PathBuf> = event
                                .paths
                                .into_iter()
                                .filter(|p| {
                                    if let Ok(rel) = p.strip_prefix(&path) {
                                        for dir in &ignore {
                                            if rel.starts_with(dir) {
                                                return false;
                                            }
                                        }
                                        true
                                    } else {
                                        println!("File changed outside of root: {}", p.display());
                                        true
                                    }
                                })
                                .collect();
                            if paths.len() > 0 {
                                changed(self_box.clone(), paths);
                            }
                        }
                        Err(e) => println!("watch error: {:?}", e),
                    }
                })?;

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
}
