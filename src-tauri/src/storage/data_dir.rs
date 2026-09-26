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

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// Where workflow files live: `workflows/<id>.json`.
    pub fn workflows_path(&self) -> PathBuf {
        self.root.join("workflows")
    }
}
