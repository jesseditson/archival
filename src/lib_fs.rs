use crate::{Archival, ArchivalError, FileSystemAPI};
use anyhow::Result;
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

/// This API is primarily intended for direct modifications to site files,
/// rather than changing objects, which should always be done via archival events.
impl<F: FileSystemAPI + Clone + Debug> Archival<F> {
    pub fn fs_list_files(&self, dir: impl AsRef<Path>, recursive: bool) -> Result<Vec<PathBuf>> {
        Ok(self
            .fs_mutex
            .with_fs(|fs| fs.list_dir(dir, recursive))?
            .collect())
    }
    pub fn fs_read_file(&self, path: impl AsRef<Path>) -> Result<String> {
        self.fs_mutex.with_fs(|fs| {
            Ok(fs
                .read_to_string(path)?
                .ok_or_else(|| ArchivalError::new("file not found"))?)
        })
    }
    pub fn fs_write_file(&self, path: impl AsRef<Path>, contents: String) -> Result<()> {
        self.fs_mutex.with_fs(|fs| fs.write_str(path, contents))
    }
    pub fn fs_delete_file(&self, path: impl AsRef<Path>) -> Result<()> {
        self.fs_mutex.with_fs(|fs| fs.delete(path))
    }
    pub fn fs_rename_file(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
        self.fs_mutex.with_fs(|fs| fs.rename(from, to))
    }
}
