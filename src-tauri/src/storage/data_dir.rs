//! The app's data folder, e.g. ~/Library/Application Support/<bundle id>/.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// Opens the folder, creating it if needed, and makes it readable only by its owner.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        Ok(Self { root })
    }

    /// Opens the folder as `open` does, first moving `old` there if the
    /// folder doesn't exist yet: the data folder of an earlier bundle id.
    pub fn open_moving(root: impl Into<PathBuf>, old: &Path) -> io::Result<Self> {
        let root = root.into();
        if !root.exists() && old.is_dir() {
            fs::rename(old, &root)?;
        }
        Self::open(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// The engine's journals of file actions: `engine/journal/<run id>.jsonl`.
    pub fn journal_path(&self) -> PathBuf {
        self.root.join("engine").join("journal")
    }

    /// Where workflow files live: `workflows/<id>.json`.
    pub fn workflows_path(&self) -> PathBuf {
        self.root.join("workflows")
    }
}
