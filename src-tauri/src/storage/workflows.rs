//! workflows/<id>.json: one file per workflow. See docs/workflow-format.md.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use super::atomic::write_atomic;
use super::data_dir::DataDir;
use crate::workflow::{validate, Problem, SaveResult, Workflow, WORKFLOW_VERSION};

/// One file in the workflows folder, as `list` found it.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub id: String,
    pub file_name: String,
    pub entry: Entry,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Ok(Workflow),
    /// Unreadable. Left exactly as it is.
    Damaged,
    /// Written by a newer FolderFlow. Left exactly as it is.
    TooNew(u32),
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("workflow ids are UUIDs")]
    InvalidId,
    #[error("no workflow with that id")]
    NotFound,
    #[error("the workflow file is from a newer version of FolderFlow (format {found})")]
    TooNew { found: u32 },
    /// A link or a folder where the workflow file should be. Never followed.
    #[error("the workflow's file is a link or a folder")]
    NotAFile,
    #[error("the workflow file can't be read")]
    Damaged,
    #[error("the workflow was saved elsewhere (revision {on_disk} on disk)")]
    Conflict { on_disk: u32 },
    #[error("the workflow can't be turned on while it has problems")]
    NotClean(Vec<Problem>),
    #[error("couldn't read or write the workflow file: {0}")]
    Io(#[from] io::Error),
}

pub struct WorkflowStore {
    dir: PathBuf,
    /// Held for every write, so saves, creates and deletes never interleave.
    /// Reads don't need it: files are replaced atomically.
    lock: Mutex<()>,
}

impl WorkflowStore {
    pub fn new(dir: &DataDir) -> Self {
        Self {
            dir: dir.workflows_path(),
            lock: Mutex::new(()),
        }
    }

    /// Every workflow file, sorted by file name. A damaged or newer file is
    /// listed as such, never changed. Other files in the folder are skipped.
    pub fn list(&self) -> io::Result<Vec<Listed>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for entry in entries {
            let file_name = entry?.file_name().to_string_lossy().into_owned();
            let Some(id) = file_name.strip_suffix(".json") else {
                continue;
            };
            if !is_workflow_id(id) {
                continue;
            }
            let entry = match self.read(id)? {
                OnDisk::Missing => continue,
                OnDisk::NotAFile | OnDisk::Damaged => Entry::Damaged,
                OnDisk::TooNew(found) => Entry::TooNew(found),
                OnDisk::Current(workflow) => Entry::Ok(workflow),
            };
            out.push(Listed {
                id: id.to_owned(),
                file_name,
                entry,
            });
        }
        out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(out)
    }

    pub fn get(&self, id: &str) -> Result<Workflow, WorkflowError> {
        check_id(id)?;
        match self.read(id)? {
            OnDisk::Current(workflow) => Ok(workflow),
            other => Err(other.into_error()),
        }
    }

    /// Writes a new workflow at revision 1. Refuses to replace any existing file.
    pub fn create(&self, workflow: Workflow) -> Result<Workflow, WorkflowError> {
        check_id(&workflow.id)?;
        let _guard = self.lock();
        match self.read(&workflow.id)? {
            OnDisk::Missing => {}
            OnDisk::Current(existing) => {
                return Err(WorkflowError::Conflict {
                    on_disk: existing.revision,
                })
            }
            other => return Err(other.into_error()),
        }
        let workflow = Workflow {
            version: WORKFLOW_VERSION,
            revision: 1,
            ..workflow
        };
        self.write(&workflow)?;
        Ok(workflow)
    }

    /// Saves a change to an existing workflow, if it was made to the revision
    /// on disk. Turning a workflow on needs it to have no problems; otherwise
    /// nothing is written.
    pub fn save(
        &self,
        workflow: Workflow,
        models: &BTreeSet<String>,
    ) -> Result<SaveResult, WorkflowError> {
        check_id(&workflow.id)?;
        let _guard = self.lock();
        let on_disk = match self.read(&workflow.id)? {
            OnDisk::Current(existing) => existing.revision,
            other => return Err(other.into_error()),
        };
        if workflow.revision != on_disk {
            return Err(WorkflowError::Conflict { on_disk });
        }
        let problems = validate(&workflow, models);
        if workflow.enabled && !problems.is_empty() {
            return Err(WorkflowError::NotClean(problems));
        }
        let revision = on_disk
            .checked_add(1)
            .ok_or(WorkflowError::Conflict { on_disk })?;
        let workflow = Workflow {
            version: WORKFLOW_VERSION,
            revision,
            ..workflow
        };
        self.write(&workflow)?;
        Ok(SaveResult { workflow, problems })
    }

    /// Moves the workflow's file to `.trash/<id>-<unix time>.json` and returns
    /// where it went. A damaged file can be deleted too; a newer one can't.
    pub fn delete(&self, id: &str) -> Result<PathBuf, WorkflowError> {
        check_id(id)?;
        let _guard = self.lock();
        match self.read(id)? {
            OnDisk::Current(_) | OnDisk::Damaged => {}
            other => return Err(other.into_error()),
        }
        let trash = self.dir.join(".trash");
        fs::create_dir_all(&trash)?;
        fs::set_permissions(&trash, fs::Permissions::from_mode(0o700))?;

        let from = self.path(id);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut n = 1;
        let to = loop {
            let name = if n == 1 {
                format!("{id}-{stamp}.json")
            } else {
                format!("{id}-{stamp}-{n}.json")
            };
            let to = trash.join(name);
            // A hard link fails if the name is taken, so an earlier copy is
            // never replaced, even by another process.
            match fs::hard_link(&from, &to) {
                Ok(()) => break to,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(e.into()),
            }
        };
        fs::remove_file(&from)?;
        File::open(&self.dir)?.sync_all()?;
        Ok(to)
    }

    fn lock(&self) -> MutexGuard<'_, ()> {
        // Writes are atomic, so a panic mid-write leaves nothing half-done.
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn write(&self, workflow: &Workflow) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))?;
        let bytes = serde_json::to_vec_pretty(workflow).map_err(io::Error::other)?;
        write_atomic(&self.path(&workflow.id), &bytes)
    }

    /// What is at the workflow's path, without following links.
    fn read(&self, id: &str) -> io::Result<OnDisk> {
        let path = self.path(id);
        match fs::symlink_metadata(&path) {
            Ok(meta) if !meta.file_type().is_file() => return Ok(OnDisk::NotAFile),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(OnDisk::Missing),
            Err(e) => return Err(e),
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(OnDisk::Missing),
            Err(e) => return Err(e),
        };
        Ok(parse(&bytes, id))
    }
}

enum OnDisk {
    Missing,
    NotAFile,
    Damaged,
    TooNew(u32),
    Current(Workflow),
}

impl OnDisk {
    /// Why a command can't use this file.
    fn into_error(self) -> WorkflowError {
        match self {
            OnDisk::Missing => WorkflowError::NotFound,
            OnDisk::NotAFile => WorkflowError::NotAFile,
            OnDisk::Damaged => WorkflowError::Damaged,
            OnDisk::TooNew(found) => WorkflowError::TooNew { found },
            OnDisk::Current(w) => WorkflowError::Conflict {
                on_disk: w.revision,
            },
        }
    }
}

fn parse(bytes: &[u8], id: &str) -> OnDisk {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return OnDisk::Damaged;
    };
    // Like settings: a file without a version is read as the current format.
    let version = match value.get("version") {
        None => WORKFLOW_VERSION,
        Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => n,
            None => return OnDisk::Damaged,
        },
    };
    if version > WORKFLOW_VERSION {
        return OnDisk::TooNew(version);
    }
    match serde_json::from_value::<Workflow>(value) {
        Ok(workflow) if workflow.id == id => OnDisk::Current(workflow),
        _ => OnDisk::Damaged,
    }
}

/// Only a lowercase, hyphenated UUID can name a workflow file, so an id can
/// never reach outside the workflows folder.
pub fn is_workflow_id(id: &str) -> bool {
    uuid::Uuid::try_parse(id).is_ok_and(|u| u.hyphenated().to_string() == id)
}

fn check_id(id: &str) -> Result<(), WorkflowError> {
    if is_workflow_id(id) {
        Ok(())
    } else {
        Err(WorkflowError::InvalidId)
    }
}
