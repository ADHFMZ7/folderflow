//! The engine: runs the workflows. It follows each run's steps, records every
//! run, and tells the window when a run changes. See docs/engine.md.
//!
//! The clock, notifications and window events are behind traits, so tests
//! run the engine for real in a temp folder with fakes for those alone.

pub mod app;
pub mod files;
mod runner;
pub mod runs;
pub mod values;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use chrono::{DateTime, FixedOffset, SecondsFormat};
use serde::Serialize;
use tokio::sync::mpsc;
use ts_rs::TS;

use crate::api::pickers::expand_home;
use crate::api::types::{ApiError, ErrorCode};
use crate::storage::data_dir::DataDir;
use crate::storage::settings::SettingsStore;
use crate::storage::workflows::WorkflowStore;
use crate::workflow::{validate, StepKind};
use files::{Files, Grants, Trash};
use runs::{
    Run, RunFile, RunQuery, RunStatus, RunStore, RunSummary, RunTrigger, TriggerKind, UndoResult,
};

/// The time on the Mac, in its own time zone.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<FixedOffset>;
}

/// macOS notifications.
pub trait Notifier: Send + Sync {
    /// Fails with a reason when the notification can't be shown.
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
}

/// Tells the window something changed. Screens then ask for the details.
pub trait EngineEvents: Send + Sync {
    fn run_changed(&self, change: RunChanged);
}

/// The `run-changed` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunChanged {
    pub run_id: String,
    pub workflow_id: String,
    pub status: RunStatus,
}

pub struct Ports {
    pub clock: Arc<dyn Clock>,
    pub notifier: Arc<dyn Notifier>,
    pub events: Arc<dyn EngineEvents>,
    pub trash: Arc<dyn Trash>,
}

impl Ports {
    /// Now, as the run records write it.
    fn now(&self) -> String {
        self.clock
            .now()
            .to_rfc3339_opts(SecondsFormat::Millis, false)
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<FixedOffset> {
        chrono::Local::now().fixed_offset()
    }
}

/// How many runs `list_runs` returns when not told.
const DEFAULT_LIMIT: u32 = 100;

pub struct Engine {
    inner: Arc<Inner>,
}

struct Inner {
    runs: RunStore,
    workflows: WorkflowStore,
    settings: SettingsStore,
    journal: PathBuf,
    home: PathBuf,
    ports: Ports,
    /// One queue per workflow, so its runs go one at a time, in order.
    queues: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
}

impl Engine {
    /// Starts the engine on the app's data folder. File actions a crash left
    /// half-known are settled from the journal first. Runs left queued or
    /// running when the app last stopped are marked interrupted; none restarts
    /// on its own (decision 5).
    pub fn new(dir: &DataDir, home: PathBuf, ports: Ports) -> io::Result<Self> {
        files::recover(&dir.journal_path())?;
        let inner = Inner {
            runs: RunStore::new(dir),
            workflows: WorkflowStore::new(dir),
            settings: SettingsStore::new(dir),
            journal: dir.journal_path(),
            home,
            ports,
            queues: Mutex::new(HashMap::new()),
        };
        for mut run in inner.runs.all()? {
            if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
                run.status = RunStatus::Interrupted;
                inner.runs.save(&run)?;
            }
        }
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Queues one run of the saved workflow per file, and returns them queued.
    /// Every file is checked first, so either all of them run or none does.
    /// Must be called inside a Tokio runtime, which runs the queue.
    pub fn run_now(
        &self,
        workflow_id: &str,
        files: Vec<String>,
    ) -> Result<Vec<RunSummary>, ApiError> {
        let inner = &self.inner;
        let workflow = inner.workflows.get(workflow_id)?;

        let trigger = workflow.steps.iter().find(|s| s.kind.is_trigger());
        if let Some(StepKind::Schedule { .. }) = trigger.map(|s| &s.kind) {
            return Err(invalid("This workflow runs on its schedule, not on files."));
        }
        let working = inner
            .settings
            .load()
            .map_err(ApiError::from)?
            .settings
            .working_kinds();
        match validate(&workflow, &working).len() {
            0 => {}
            1 => {
                return Err(invalid(
                    "Fix the problem with this workflow before running it.",
                ))
            }
            n => {
                return Err(invalid(format!(
                    "Fix the {n} problems with this workflow before running it."
                )))
            }
        }
        if files.is_empty() {
            return Err(invalid("Choose at least one file to run on."));
        }
        let files = files
            .iter()
            .map(|f| chosen_file(f, &inner.home))
            .collect::<Result<Vec<_>, _>>()?;

        let mut queued = Vec::new();
        for file in files {
            let run = Run {
                id: uuid::Uuid::new_v4().to_string(),
                workflow_id: workflow.id.clone(),
                revision: workflow.revision,
                workflow: workflow.clone(),
                trigger: RunTrigger {
                    kind: TriggerKind::RunNow,
                    file: Some(file.clone()),
                },
                file: Some(file),
                undo: None,
                status: RunStatus::Queued,
                started_at: inner.ports.now(),
                ended_at: None,
                steps: Vec::new(),
                values: Default::default(),
                error: None,
            };
            inner.runs.save(&run).map_err(history_error)?;
            inner.announce(&run);
            queued.push(run);
        }
        for run in &queued {
            Inner::enqueue(inner, &run.workflow_id, &run.id);
        }
        Ok(queued.iter().map(Run::summary).collect())
    }

    /// Reverses every file action of a finished run, newest first, and marks
    /// it undone. Files changed since are left alone and listed.
    pub fn undo_run(&self, id: &str) -> Result<UndoResult, ApiError> {
        let mut run = self.get_run(id)?;
        match run.status {
            RunStatus::Done | RunStatus::Failed | RunStatus::Interrupted => {}
            RunStatus::Undone => {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    "This run was already undone.",
                ))
            }
            RunStatus::Queued | RunStatus::Running | RunStatus::Waiting => {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    "This run hasn't finished, so it can't be undone yet.",
                ))
            }
        }
        let inner = &self.inner;
        let report = files::undo(&inner.journal, &run.id, inner.ports.trash.as_ref())
            .map_err(|e| ApiError::new(ErrorCode::Io, format!("Couldn't undo the run: {e}.")))?;
        run.status = RunStatus::Undone;
        run.undo = Some(report.clone());
        inner.runs.save(&run).map_err(history_error)?;
        inner.announce(&run);
        Ok(UndoResult { run, report })
    }

    pub fn get_run(&self, id: &str) -> Result<Run, ApiError> {
        self.inner
            .runs
            .get(id)
            .map_err(history_error)?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "That run no longer exists."))
    }

    /// Runs newest first, narrowed by the query.
    pub fn list_runs(&self, query: RunQuery) -> Result<Vec<RunSummary>, ApiError> {
        let mut runs = self.inner.runs.all().map_err(history_error)?;
        runs.sort_by_cached_key(|r| {
            std::cmp::Reverse((
                DateTime::parse_from_rfc3339(&r.started_at).ok(),
                r.id.clone(),
            ))
        });
        let listed = runs
            .iter()
            .filter(|r| {
                query
                    .workflow_id
                    .as_ref()
                    .is_none_or(|w| &r.workflow_id == w)
            })
            .filter(|r| query.status.is_none_or(|s| r.status == s));
        let after = match &query.before {
            Some(before) => listed
                .skip_while(|r| &r.id != before)
                .skip(1)
                .collect::<Vec<_>>(),
            None => listed.collect(),
        };
        Ok(after
            .into_iter()
            .take(query.limit.unwrap_or(DEFAULT_LIMIT) as usize)
            .map(Run::summary)
            .collect())
    }
}

impl Inner {
    fn announce(&self, run: &Run) {
        self.ports.events.run_changed(RunChanged {
            run_id: run.id.clone(),
            workflow_id: run.workflow_id.clone(),
            status: run.status,
        });
    }

    /// Hands the run to its workflow's queue, starting the queue if needed.
    fn enqueue(this: &Arc<Self>, workflow_id: &str, run_id: &str) {
        let mut queues = this.queues.lock().unwrap_or_else(|e| e.into_inner());
        let queue = queues.entry(workflow_id.to_owned()).or_insert_with(|| {
            let (tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(work_through(Arc::downgrade(this), rx));
            tx
        });
        let _ = queue.send(run_id.to_owned());
    }

    async fn execute(&self, run_id: &str) {
        let Ok(Some(mut run)) = self.runs.get(run_id) else {
            return;
        };
        if run.status != RunStatus::Queued {
            return;
        }
        run.status = RunStatus::Running;
        self.save(&run);
        self.announce(&run);

        let grants = Grants::for_run(
            &run.workflow,
            run.file.as_ref().map(|f| f.path.as_path()),
            &self.home,
        );
        let result = match Files::open(&self.journal, &run.id, grants) {
            Ok(files) => {
                let mut ctx = runner::Ctx {
                    home: &self.home,
                    ports: &self.ports,
                    files,
                };
                runner::run(&mut run, &mut ctx, |r| self.save(r)).await
            }
            Err(e) => Err(runs::RunError {
                step_id: None,
                message: format!("Couldn't open the run's journal: {e}."),
            }),
        };

        run.ended_at = Some(self.ports.now());
        run.status = match result {
            Ok(()) => RunStatus::Done,
            Err(error) => {
                self.tell_failed(&run, &error.step_id, &error.message);
                run.error = Some(error);
                RunStatus::Failed
            }
        };
        self.save(&run);
        self.announce(&run);
    }

    /// A save that fails mid-run can't stop the run; the next save retries.
    fn save(&self, run: &Run) {
        if let Err(e) = self.runs.save(run) {
            eprintln!("folderflow: couldn't save run {}: {e}", run.id);
        }
    }

    fn tell_failed(&self, run: &Run, step_id: &Option<String>, message: &str) {
        let step = step_id
            .as_ref()
            .and_then(|id| run.workflow.steps.iter().find(|s| &s.id == id))
            .map(|s| s.title.as_str())
            .unwrap_or("A step");
        let on = run
            .summary()
            .file
            .map(|f| format!(" on {f}"))
            .unwrap_or_default();
        let _ = self
            .ports
            .notifier
            .notify(&run.workflow.name, &format!("{step} failed{on}: {message}"));
    }
}

/// Runs a workflow's queued runs one at a time, until the engine is gone.
async fn work_through(engine: Weak<Inner>, mut queue: mpsc::UnboundedReceiver<String>) {
    while let Some(run_id) = queue.recv().await {
        let Some(inner) = engine.upgrade() else {
            return;
        };
        inner.execute(&run_id).await;
    }
}

/// A file the person chose: `~` expanded, and a regular file, not a folder or a link.
fn chosen_file(chosen: &str, home: &Path) -> Result<RunFile, ApiError> {
    let path = expand_home(chosen, home)
        .ok_or_else(|| invalid(format!("{chosen} isn't a full path to a file.")))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| chosen.to_owned());
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(invalid(format!("{name} no longer exists.")))
        }
        Err(e) => {
            return Err(ApiError::new(
                ErrorCode::Io,
                format!("Couldn't open {name}: {e}"),
            ))
        }
    };
    if meta.file_type().is_symlink() {
        return Err(invalid(format!(
            "{name} is a link. Choose the file it points to."
        )));
    }
    if meta.is_dir() {
        return Err(invalid(format!(
            "{name} is a folder. Choose files to run on."
        )));
    }
    if !meta.is_file() {
        return Err(invalid(format!(
            "{name} isn't a file FolderFlow can run on."
        )));
    }
    Ok(RunFile {
        path,
        inode: meta.ino(),
    })
}

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Invalid, message)
}

fn history_error(e: io::Error) -> ApiError {
    ApiError::new(
        ErrorCode::Io,
        format!("Couldn't read or save the run history: {e}"),
    )
}
