use std::{
    error::Error,
    path::{Path, PathBuf},
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

mod native {
    use fs_extra;
    use std::{fs, path::Path};

    use super::FileSystemAPI;

    pub struct NativeFileSystem;
    impl FileSystemAPI for NativeFileSystem {
        fn remove_dir_all(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
            Ok(fs::remove_dir_all(path)?)
        }
        fn create_dir_all(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
            Ok(fs::create_dir_all(path)?)
        }
        fn read_dir(
            &self,
            path: &Path,
        ) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
            let mut files = vec![];
            for f in fs::read_dir(path)? {
                if let Ok(f) = f {
                    files.push(f.path());
                }
            }
            Ok(files)
        }
        fn read_to_string(&self, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
            Ok(fs::read_to_string(path)?)
        }
        fn write(&self, path: &Path, contents: String) -> Result<(), Box<dyn std::error::Error>> {
            Ok(fs::write(path, contents)?)
        }
        fn copy_contents(&self, from: &Path, to: &Path) -> Result<(), Box<dyn std::error::Error>> {
            let mut options = fs_extra::dir::CopyOptions::new();
            options.overwrite = true;
            options.content_only = true;
            fs_extra::dir::copy(from, to, &options)?;
            Ok(())
        }
    }
}
