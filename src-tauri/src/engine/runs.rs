//! Run records: `runs/<workflow id>/<run id>.json` in the data folder, one per
//! run, rewritten atomically after every step. See docs/engine.md, "Runs".

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::files::UndoReport;
use crate::storage::atomic::write_atomic;
use crate::storage::data_dir::DataDir;
use crate::storage::workflows::is_workflow_id;
use crate::workflow::{Branch, Workflow};

/// One pass of one workflow over one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Run {
    pub id: String,
    pub workflow_id: String,
    /// The workflow's revision when the run was queued.
    pub revision: u32,
    /// The workflow as it was when the run was queued. The run follows this
    /// copy, so editing the workflow meanwhile doesn't change it.
    pub workflow: Workflow,
    pub trigger: RunTrigger,
    /// Where the run's file is now, after any Rename or Move.
    #[serde(default)]
    pub file: Option<RunFile>,
    pub status: RunStatus,
    /// When it was queued. RFC 3339, with the Mac's offset.
    pub started_at: String,
    /// When it became done, failed or undone.
    pub ended_at: Option<String>,
    /// One entry per step reached, in order.
    pub steps: Vec<StepRun>,
    /// Every `{variable}` so far.
    pub values: BTreeMap<String, RunValue>,
    pub error: Option<RunError>,
    /// What undoing the run did, once it's undone.
    #[serde(default)]
    pub undo: Option<UndoReport>,
    /// The question a waiting run is paused on.
    #[serde(default)]
    pub waiting_for: Option<Question>,
    /// The step a queued run carries on from, after an answer, a retry or a
    /// resume. `None` starts at the trigger.
    #[serde(default)]
    pub continue_at: Option<String>,
    /// A failed or interrupted run the person put aside: out of Needs you.
    #[serde(default)]
    pub dismissed: bool,
}

/// What an Ask me step asks, with its `{variables}` filled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Question {
    pub step_id: String,
    pub question: String,
    pub answers: Vec<Branch>,
}

/// Something that waits on the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NeedsYouItem {
    pub kind: NeedsYouKind,
    pub run: RunSummary,
    /// The title of the step it's about.
    pub step: Option<String>,
    /// The question, why the run failed, or where it stopped.
    pub message: String,
    /// For a question, one per button.
    pub answers: Vec<Branch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NeedsYouKind {
    Question,
    Failed,
    Interrupted,
}

/// What `undo_run` returns.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UndoResult {
    pub run: Run,
    #[serde(flatten)]
    #[ts(flatten)]
    pub report: UndoReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RunStatus {
    Queued,
    Running,
    /// Paused on Ask me or a review.
    Waiting,
    Done,
    Failed,
    /// Cut off by the app quitting or crashing.
    Interrupted,
    Undone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunTrigger {
    pub kind: TriggerKind,
    /// The file the run is for, as it was when the run was queued.
    pub file: Option<RunFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TriggerKind {
    FileAdded,
    Schedule,
    RunNow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunFile {
    #[ts(type = "string")]
    pub path: PathBuf,
    #[ts(type = "number")]
    pub inode: u64,
}

/// What one step did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StepRun {
    pub step_id: String,
    pub title: String,
    /// The step's `type`.
    #[serde(rename = "type")]
    pub kind: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub outcome: StepOutcome,
    /// The branch a branching step chose.
    pub branch: Option<String>,
    /// The values this step produced.
    pub values: BTreeMap<String, RunValue>,
    /// Something to tell the person about this step, in plain words.
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StepOutcome {
    Running,
    /// An Ask me step, waiting for the answer.
    Waiting,
    Done,
    Failed,
}

/// A `{variable}`'s value and the kind its step gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RunValue {
    pub kind: ValueKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ValueKind {
    Text,
    Number,
    /// `YYYY-MM-DD`.
    Date,
    /// `yes` or `no`.
    YesNo,
}

/// Why a run failed. The message never holds a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunError {
    /// The step that failed, if a step did.
    pub step_id: Option<String>,
    pub message: String,
}

/// A line in the run history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunSummary {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: RunStatus,
    /// The file's name, for runs on a file.
    pub file: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    /// Why it failed, for a failed run.
    pub error: Option<String>,
}

/// Which runs `list_runs` returns. Every field narrows the list.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct RunQuery {
    pub workflow_id: Option<String>,
    pub status: Option<RunStatus>,
    /// Only runs listed after this run id: the next page.
    pub before: Option<String>,
    /// At most this many; 100 when missing.
    pub limit: Option<u32>,
}

impl Run {
    /// What this run waits on the person for, if anything.
    pub fn needs_you(&self) -> Option<NeedsYouItem> {
        let step_title = |id: Option<&String>| {
            id.and_then(|id| self.steps.iter().rev().find(|s| &s.step_id == id))
                .map(|s| s.title.clone())
        };
        let (kind, step, message, answers) = match self.status {
            RunStatus::Waiting => {
                let q = self.waiting_for.as_ref()?;
                (
                    NeedsYouKind::Question,
                    step_title(Some(&q.step_id)),
                    q.question.clone(),
                    q.answers.clone(),
                )
            }
            RunStatus::Failed if !self.dismissed => {
                let error = self.error.as_ref();
                (
                    NeedsYouKind::Failed,
                    step_title(error.and_then(|e| e.step_id.as_ref())),
                    error.map_or_else(|| "The run failed.".into(), |e| e.message.clone()),
                    Vec::new(),
                )
            }
            RunStatus::Interrupted if !self.dismissed => {
                let at = self.steps.last().map(|s| s.title.clone());
                let message = match &at {
                    Some(step) => format!("Stopped at {step} when FolderFlow quit."),
                    None => "FolderFlow quit before this run started.".into(),
                };
                (NeedsYouKind::Interrupted, at, message, Vec::new())
            }
            _ => return None,
        };
        Some(NeedsYouItem {
            kind,
            run: self.summary(),
            step,
            message,
            answers,
        })
    }

    pub fn summary(&self) -> RunSummary {
        RunSummary {
            id: self.id.clone(),
            workflow_id: self.workflow_id.clone(),
            workflow_name: self.workflow.name.clone(),
            status: self.status,
            file: self.trigger.file.as_ref().and_then(|f| {
                f.path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            }),
            started_at: self.started_at.clone(),
            ended_at: self.ended_at.clone(),
            error: self.error.as_ref().map(|e| e.message.clone()),
        }
    }
}

/// Reads and writes run records. Unreadable records are skipped, never changed.
pub struct RunStore {
    root: PathBuf,
}

impl RunStore {
    pub fn new(dir: &DataDir) -> Self {
        Self {
            root: dir.root().join("runs"),
        }
    }

    pub fn save(&self, run: &Run) -> io::Result<()> {
        let dir = self.root.join(&run.workflow_id);
        fs::create_dir_all(&dir)?;
        let bytes = serde_json::to_vec_pretty(run).map_err(io::Error::other)?;
        write_atomic(&dir.join(format!("{}.json", run.id)), &bytes)
    }

    /// The run with this id, or `None` if there is none (or it's unreadable).
    pub fn get(&self, id: &str) -> io::Result<Option<Run>> {
        if !is_workflow_id(id) {
            return Ok(None);
        }
        for dir in self.workflow_dirs()? {
            let path = dir.join(format!("{id}.json"));
            if fs::symlink_metadata(&path).is_ok() {
                return Ok(read(&path));
            }
        }
        Ok(None)
    }

    /// Every readable run, in no particular order.
    pub fn all(&self) -> io::Result<Vec<Run>> {
        let mut out = Vec::new();
        for dir in self.workflow_dirs()? {
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "json") {
                    out.extend(read(&path));
                }
            }
        }
        Ok(out)
    }

    /// Removes a workflow's oldest finished runs beyond the newest `keep`,
    /// and hands each removed run's id to `removed`. Runs that are waiting,
    /// failed or interrupted are kept, however old.
    pub fn prune(
        &self,
        workflow_id: &str,
        keep: usize,
        mut removed: impl FnMut(&str),
    ) -> io::Result<()> {
        let dir = self.root.join(workflow_id);
        let count = match fs::read_dir(&dir) {
            Ok(listed) => listed.count(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        // Cheap to count; read them all only when there's something to remove.
        if count <= keep {
            return Ok(());
        }
        let mut runs: Vec<Run> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| read(&e.path()))
            .collect();
        runs.sort_by_cached_key(|r| {
            std::cmp::Reverse((
                chrono::DateTime::parse_from_rfc3339(&r.started_at).ok(),
                r.id.clone(),
            ))
        });
        for run in runs.iter().skip(keep) {
            let kept = matches!(
                run.status,
                RunStatus::Queued
                    | RunStatus::Running
                    | RunStatus::Waiting
                    | RunStatus::Failed
                    | RunStatus::Interrupted
            );
            if !kept {
                fs::remove_file(dir.join(format!("{}.json", run.id)))?;
                removed(&run.id);
            }
        }
        Ok(())
    }

    fn workflow_dirs(&self) -> io::Result<Vec<PathBuf>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            if entry.file_type()?.is_dir() && is_workflow_id(&name.to_string_lossy()) {
                out.push(entry.path());
            }
        }
        Ok(out)
    }
}

/// A regular file that parses as a run; links and damaged files are skipped.
fn read(path: &Path) -> Option<Run> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
