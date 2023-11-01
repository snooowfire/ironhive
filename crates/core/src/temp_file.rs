use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, io};

use crate::error::Error;
use tokio::io::AsyncWriteExt;
use tracing::error;

/// A temporary directory.
///
/// This is a RAII object which will remove the underlying temporary directory
/// when dropped.
#[derive(Debug)]
#[must_use]
pub struct TempFile {
    path: PathBuf,
}

impl TempFile {
    /// Returns the path to the underlying temporary directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn new(prefix: &str, content: impl AsRef<[u8]>) -> Result<Self, Error> {
        let base = std::env::temp_dir();
        tokio::fs::create_dir_all(&base).await?;

        static CNT: AtomicUsize = AtomicUsize::new(0);

        let mut n_try = 0u32;
        loop {
            let cnt = CNT.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("ironhive-tmp-file-{}{}", cnt, prefix));

            match tokio::fs::File::create(&path).await {
                Ok(mut f) => {
                    f.write_all(content.as_ref()).await?;
                    return Ok(Self { path });
                }
                Err(err) if n_try == 1024 => {
                    error!("create tempfile {path:?} failed: {err}");
                    return Err(Error::TokioIoError(err));
                }
                Err(_) => n_try += 1,
            }
        }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = remove_file(&self.path);
    }
}

#[cfg(not(windows))]
fn remove_file(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

#[cfg(windows)]
fn remove_file(path: &Path) -> io::Result<()> {
    for _ in 0..99 {
        if fs::remove_file(path).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    fs::remove_file(path)
}
