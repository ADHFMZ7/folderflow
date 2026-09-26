//! workflows/<id>.json: one file per workflow, and workflows/<id>.draft.json for
//! edits to a running workflow that aren't live yet. See docs/workflow-format.md.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
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
    /// Whether there are changes waiting in a draft.
    pub has_draft: bool,
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
            let has_draft = fs::symlink_metadata(self.draft_path(id)).is_ok();
            out.push(Listed {
                id: id.to_owned(),
                file_name,
                entry,
                has_draft,
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
    /// where it went. A damaged file can be deleted too; a newer one can't. A
    /// draft goes to the trash with it.
    pub fn delete(&self, id: &str) -> Result<PathBuf, WorkflowError> {
        check_id(id)?;
        let _guard = self.lock();
        match self.read(id)? {
            OnDisk::Current(_) | OnDisk::Damaged => {}
            other => return Err(other.into_error()),
        }
        let to = self.trash(&self.path(id), id)?;
        if let OnDisk::Current(_) | OnDisk::Damaged = self.read_draft(id)? {
            self.trash(&self.draft_path(id), &format!("{id}.draft"))?;
        }
        Ok(to)
    }

    /// The draft of a workflow, if it has one. A draft left behind by an older
    /// revision (say, after a crash right after applying) moves to the trash.
    pub fn get_draft(&self, id: &str) -> Result<Option<Workflow>, WorkflowError> {
        check_id(id)?;
        let _guard = self.lock();
        let live = self.current(id)?;
        match self.read_draft(id)? {
            OnDisk::Missing => Ok(None),
            OnDisk::Current(draft) if draft.revision == live.revision => Ok(Some(draft)),
            OnDisk::Current(_) => {
                self.trash(&self.draft_path(id), &format!("{id}.draft"))?;
                Ok(None)
            }
            other => Err(other.into_error()),
        }
    }

    /// Saves edits to a draft, never to the running file. The draft keeps the
    /// revision it started from and the running workflow's on/off state.
    /// Problems don't stop a draft from being saved.
    pub fn save_draft(
        &self,
        draft: Workflow,
        models: &BTreeSet<String>,
    ) -> Result<SaveResult, WorkflowError> {
        check_id(&draft.id)?;
        let _guard = self.lock();
        let live = self.current(&draft.id)?;
        match self.read_draft(&draft.id)? {
            OnDisk::Missing | OnDisk::Current(_) => {}
            // Unreadable: keep a copy before replacing it.
            OnDisk::Damaged => {
                self.trash(&self.draft_path(&draft.id), &format!("{}.draft", draft.id))?;
            }
            other => return Err(other.into_error()),
        }
        if draft.revision != live.revision {
            return Err(WorkflowError::Conflict {
                on_disk: live.revision,
            });
        }
        let problems = validate(&draft, models);
        let draft = Workflow {
            version: WORKFLOW_VERSION,
            revision: live.revision,
            enabled: live.enabled,
            ..draft
        };
        self.write_at(&self.draft_path(&draft.id), &draft)?;
        Ok(SaveResult {
            workflow: draft,
            problems,
        })
    }

    /// Makes the draft the running workflow, at the next revision, and removes
    /// it. A running workflow only takes a draft with no problems; if anything
    /// is refused, nothing is written.
    pub fn apply_draft(
        &self,
        id: &str,
        models: &BTreeSet<String>,
    ) -> Result<SaveResult, WorkflowError> {
        check_id(id)?;
        let _guard = self.lock();
        let live = self.current(id)?;
        let draft = match self.read_draft(id)? {
            OnDisk::Current(draft) => draft,
            other => return Err(other.into_error()),
        };
        if draft.revision != live.revision {
            return Err(WorkflowError::Conflict {
                on_disk: live.revision,
            });
        }
        let problems = validate(&draft, models);
        if live.enabled && !problems.is_empty() {
            return Err(WorkflowError::NotClean(problems));
        }
        let revision = live
            .revision
            .checked_add(1)
            .ok_or(WorkflowError::Conflict {
                on_disk: live.revision,
            })?;
        let workflow = Workflow {
            version: WORKFLOW_VERSION,
            revision,
            enabled: live.enabled,
            ..draft
        };
        self.write(&workflow)?;
        // Its content now lives in the running file.
        fs::remove_file(self.draft_path(id))?;
        File::open(&self.dir)?.sync_all()?;
        Ok(SaveResult { workflow, problems })
    }

    /// Moves the draft to the trash. With no draft, does nothing.
    pub fn discard_draft(&self, id: &str) -> Result<(), WorkflowError> {
        check_id(id)?;
        let _guard = self.lock();
        self.current(id)?;
        match self.read_draft(id)? {
            OnDisk::Missing => Ok(()),
            OnDisk::Current(_) | OnDisk::Damaged => {
                self.trash(&self.draft_path(id), &format!("{id}.draft"))?;
                Ok(())
            }
            other => Err(other.into_error()),
        }
    }

    /// The running workflow, or why it can't be used.
    fn current(&self, id: &str) -> Result<Workflow, WorkflowError> {
        match self.read(id)? {
            OnDisk::Current(workflow) => Ok(workflow),
            other => Err(other.into_error()),
        }
    }

    /// Moves a file to `.trash/<stem>-<unix time>.json` and returns where it
    /// went. A hard link fails if the name is taken, so an earlier copy is
    /// never replaced, even by another process.
    fn trash(&self, from: &Path, stem: &str) -> io::Result<PathBuf> {
        let trash = self.dir.join(".trash");
        fs::create_dir_all(&trash)?;
        fs::set_permissions(&trash, fs::Permissions::from_mode(0o700))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut n = 1;
        let to = loop {
            let name = if n == 1 {
                format!("{stem}-{stamp}.json")
            } else {
                format!("{stem}-{stamp}-{n}.json")
            };
            let to = trash.join(name);
            match fs::hard_link(from, &to) {
                Ok(()) => break to,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(e),
            }
        };
        fs::remove_file(from)?;
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

    fn draft_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.draft.json"))
    }

    fn write(&self, workflow: &Workflow) -> io::Result<()> {
        self.write_at(&self.path(&workflow.id), workflow)
    }

    fn write_at(&self, path: &Path, workflow: &Workflow) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))?;
        let bytes = serde_json::to_vec_pretty(workflow).map_err(io::Error::other)?;
        write_atomic(path, &bytes)
    }

    /// What is at the workflow's path, without following links.
    fn read(&self, id: &str) -> io::Result<OnDisk> {
        self.read_at(&self.path(id), id)
    }

    /// What is at the workflow's draft path, without following links.
    fn read_draft(&self, id: &str) -> io::Result<OnDisk> {
        self.read_at(&self.draft_path(id), id)
    }

    fn read_at(&self, path: &Path, id: &str) -> io::Result<OnDisk> {
        match fs::symlink_metadata(path) {
            Ok(meta) if !meta.file_type().is_file() => return Ok(OnDisk::NotAFile),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(OnDisk::Missing),
            Err(e) => return Err(e),
        }
        let bytes = match fs::read(path) {
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
