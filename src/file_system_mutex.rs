use std::{
    error::Error,
    ops::DerefMut,
    sync::{Arc, Mutex},
};
use thiserror::Error;

use crate::FileSystemAPI;

#[derive(Error, Debug)]
pub enum FileSystemMutexError {
    #[error("File System mutex lock failed.")]
    LockFailed,
}

pub struct FileSystemMutex<F: FileSystemAPI>(Arc<Mutex<F>>);

impl<F> FileSystemMutex<F>
where
    F: FileSystemAPI,
{
    pub fn init(fs: F) -> Self {
        Self(Arc::new(Mutex::new(fs)))
    }
    pub fn with_fs<R>(
        &self,
        f: impl FnOnce(&mut F) -> Result<R, Box<dyn Error>>,
    ) -> Result<R, Box<dyn Error>> {
        if let Ok(mut fs) = self.0.try_lock() {
            let r = f(fs.deref_mut())?;
            Ok(r)
        } else {
            Err(FileSystemMutexError::LockFailed.into())
        }
    }
    pub fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
