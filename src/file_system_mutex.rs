use std::{error::Error, ops::DerefMut, sync::Mutex};
use thiserror::Error;

use crate::FileSystemAPI;

#[derive(Error, Debug)]
pub enum FileSystemMutexError {
    #[cfg(not(debug_assertions))]
    #[error("File System mutex lock failed.")]
    LockFailed,
}

#[derive(Debug)]
pub struct FileSystemMutex<F: FileSystemAPI>(Mutex<F>);

impl<F> FileSystemMutex<F>
where
    F: FileSystemAPI,
{
    pub fn init(fs: F) -> Self {
        Self(Mutex::new(fs))
    }
    pub fn with_fs<R>(
        &self,
        f: impl FnOnce(&mut F) -> Result<R, Box<dyn Error>>,
    ) -> Result<R, Box<dyn Error>> {
        if let Ok(mut fs) = self.0.try_lock() {
            let r = f(fs.deref_mut())?;
            Ok(r)
        } else {
            #[cfg(debug_assertions)]
            panic!("File System Lock Failed");
            #[cfg(not(debug_assertions))]
            Err(FileSystemMutexError::LockFailed.into())
        }
    }
    pub fn take_fs(self) -> F {
        self.0
            .into_inner()
            .expect("attempt to take fs while locked.")
    }
}
