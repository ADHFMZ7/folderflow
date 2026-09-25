//! Shared helpers for the storage tests.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::storage::secrets::{MemorySecretStore, Secret, SecretError, SecretStore};

/// A fresh data folder inside a temp dir that is deleted when the guard drops.
pub fn data_dir() -> (tempfile::TempDir, DataDir) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = DataDir::open(tmp.path().join("data")).unwrap();
    (tmp, dir)
}

pub fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

/// Every regular file under `root`, however deep.
pub fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// Fails if any file under `root` contains `needle`.
pub fn assert_no_file_contains(root: &Path, needle: &str) {
    for file in files_under(root) {
        let bytes = fs::read(&file).unwrap();
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "{} contains the secret",
            file.display()
        );
    }
}

/// A memory store whose writes and deletes can be made to fail.
#[derive(Default)]
pub struct FlakySecrets {
    inner: MemorySecretStore,
    pub fail_set: AtomicBool,
    pub fail_delete: AtomicBool,
}

impl FlakySecrets {
    pub fn failing_set() -> Self {
        let s = Self::default();
        s.fail_set.store(true, Ordering::SeqCst);
        s
    }
}

impl SecretStore for FlakySecrets {
    fn set(&self, account: &str, secret: &Secret) -> Result<(), SecretError> {
        if self.fail_set.load(Ordering::SeqCst) {
            return Err(SecretError::Backend("the Keychain is locked".into()));
        }
        self.inner.set(account, secret)
    }

    fn get(&self, account: &str) -> Result<Option<Secret>, SecretError> {
        self.inner.get(account)
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(SecretError::Backend("the Keychain is locked".into()));
        }
        self.inner.delete(account)
    }
}
