//! settings.json: the user's settings and connections. Never holds a key.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::atomic::write_atomic;
use super::data_dir::DataDir;

/// The settings format this build writes.
pub const SETTINGS_VERSION: u32 = 1;

/// Mirrors `Settings` in src/api/types.ts; the JSON field names are the contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub setup_complete: bool,
    pub open_at_login: bool,
    pub connections: Vec<Connection>,
    /// The model each kind uses unless a step overrides it, keyed by kind id.
    pub defaults: BTreeMap<String, Option<ModelRef>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            setup_complete: false,
            open_at_login: true,
            connections: Vec::new(),
            defaults: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRef {
    pub connection_id: String,
    pub model_id: String,
}

/// How the settings came to be what `load` returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    /// No settings file yet: first launch.
    Fresh,
    Loaded,
    /// The file was unreadable. It was moved to `backup` and defaults were used.
    Recovered {
        backup: PathBuf,
    },
}

#[derive(Debug)]
pub struct Loaded {
    pub settings: Settings,
    pub outcome: LoadOutcome,
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("couldn't read or write the settings file: {0}")]
    Io(#[from] std::io::Error),
    /// Written by a newer FolderFlow. Left untouched so switching back loses nothing.
    #[error("the settings file is from a newer version of FolderFlow (format {found})")]
    TooNew { found: u32 },
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(dir: &DataDir) -> Self {
        Self {
            path: dir.settings_path(),
        }
    }

    pub fn load(&self) -> Result<Loaded, SettingsError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Loaded {
                    settings: Settings::default(),
                    outcome: LoadOutcome::Fresh,
                });
            }
            Err(e) => return Err(e.into()),
        };

        match parse(&bytes) {
            Parsed::Current(settings) => Ok(Loaded {
                settings,
                outcome: LoadOutcome::Loaded,
            }),
            Parsed::TooNew(found) => Err(SettingsError::TooNew { found }),
            Parsed::Damaged => {
                let backup = self.set_aside()?;
                Ok(Loaded {
                    settings: Settings::default(),
                    outcome: LoadOutcome::Recovered { backup },
                })
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        // Never replace a file a newer FolderFlow wrote.
        match fs::read(&self.path) {
            Ok(bytes) => {
                if let Parsed::TooNew(found) = parse(&bytes) {
                    return Err(SettingsError::TooNew { found });
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        let file = OnDisk {
            version: SETTINGS_VERSION,
            settings,
        };
        let bytes = serde_json::to_vec_pretty(&file).map_err(io::Error::other)?;
        write_atomic(&self.path, &bytes)?;
        Ok(())
    }

    /// Moves an unreadable settings file next to where it was, under a name no
    /// earlier backup uses, so nothing the user had is thrown away.
    fn set_aside(&self) -> io::Result<PathBuf> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let short = &uuid::Uuid::new_v4().simple().to_string()[..8];
        let backup = self
            .path
            .with_file_name(format!("settings.damaged-{stamp}-{short}.json"));
        fs::rename(&self.path, &backup)?;
        Ok(backup)
    }
}

/// The file on disk: the settings plus the format version they were written in.
#[derive(Serialize)]
struct OnDisk<'a> {
    version: u32,
    #[serde(flatten)]
    settings: &'a Settings,
}

enum Parsed {
    Current(Settings),
    TooNew(u32),
    Damaged,
}

fn parse(bytes: &[u8]) -> Parsed {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Parsed::Damaged;
    };
    // A file without a version predates versioning and is read as the current format.
    let version = match value.get("version") {
        None => SETTINGS_VERSION,
        Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => n,
            None => return Parsed::Damaged,
        },
    };
    if version > SETTINGS_VERSION {
        return Parsed::TooNew(version);
    }
    // Older formats are migrated here once a second version exists.
    match serde_json::from_value::<Settings>(value) {
        Ok(settings) => Parsed::Current(settings),
        Err(_) => Parsed::Damaged,
    }
}
