use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast},
    FileSystem, FileSystemEntryCallback, FileSystemFlags,
};

use super::{FileSystemAPI, WatchableFileSystemAPI};

pub struct WasmFileSystem {
    fs: FileSystem,
}

impl FileSystemAPI for WasmFileSystem {
    fn remove_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let flags = FileSystemFlags::new();
        let success_cb: Closure<dyn Fn()> = Closure::new(move || {
            println!("REMOVE");
        });
        let error_cb: Closure<dyn Fn()> = Closure::new(move || {
            println!("ERROR");
        });
        let mut success = FileSystemEntryCallback::new();
        success.handle_event(success_cb.as_ref().unchecked_ref());
        self.fs
            .root()
            .get_directory_with_path_and_options_and_file_system_entry_callback_and_callback(
                path.to_str(),
                &flags,
                &success,
                error_cb.as_ref().unchecked_ref(),
            );
        Ok(())
    }
    fn create_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, Box<dyn Error>> {
        let mut files = vec![];
        todo!();
        Ok(files)
    }
    fn read_to_string(&self, path: &Path) -> Result<String, Box<dyn Error>> {
        todo!();
        Ok("".to_string())
    }
    fn write(&self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
        todo!();
        Ok(())
    }
    fn copy_contents(&self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
        todo!();
        Ok(())
    }
}

impl WatchableFileSystemAPI for WasmFileSystem {
    fn watch(
        self,
        root: PathBuf,
        watch_paths: Vec<String>,
        changed: impl Fn(Arc<Mutex<dyn FileSystemAPI>>, Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Result<Box<dyn FnOnce()>, Box<dyn Error>> {
        todo!();
        Ok(Box::new(|| {}))
    }
}
