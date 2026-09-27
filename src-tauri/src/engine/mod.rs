//! The engine: runs the workflows. It watches their folders, wakes their
//! schedules, follows each run's steps, records every run, and tells the
//! window when a run changes. See docs/engine.md.
//!
//! The clock, notifications, window events and folder watching are behind
//! traits, so tests run the engine for real in a temp folder with fakes for
//! those alone.

pub mod app;
pub mod content;
pub mod files;
pub mod intake;
pub mod notices;
mod runner;
pub mod runs;
pub mod schedule;
pub mod values;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use serde::Serialize;
use tokio::sync::mpsc;
use ts_rs::TS;

use crate::api::pickers::expand_home;
use crate::api::types::{ApiError, ErrorCode};
use crate::storage::data_dir::DataDir;
use crate::storage::settings::SettingsStore;
use crate::storage::workflows::{WorkflowError, WorkflowStore};
use crate::workflow::{validate, StepKind, Workflow};
use files::{Files, Grants, Plan, Trash, Writes};
use intake::{Intake, Watch};
use notices::{Notice, NoticeKind, Notices};
use runs::{
    NeedsYouItem, Question, Run, RunError, RunFile, RunQuery, RunStatus, RunStore, RunSummary,
    RunTrigger, RunValue, StepOutcome, StepRun, TriggerKind, UndoResult,
};
use schedule::Schedules;

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
    /// The notification list changed; `unread` is how many are unread now.
    fn notices_changed(&self, _unread: u32) {}
    /// Pause all, the count of runs in progress, or the count waiting on the
    /// person changed.
    fn activity_changed(&self, _activity: Activity) {}
}

/// Watches folders, and tells the engine of changes in them through
/// `Engine::folders_changed`.
pub trait Watcher: Send + Sync {
    /// Watch exactly these folders from now on, each with its subfolders or not.
    fn watch(&self, folders: &[(PathBuf, bool)]);
}

/// What the title bar and the menu bar show: whether workflows are paused,
/// how many runs are queued or running, and how many are in Needs you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Activity {
    pub paused: bool,
    pub running: u32,
    pub needs_you: u32,
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

/// What a try on a file did: each step on its path and what it would do,
/// how it ended (`done`, `waiting` on a question, or `failed`), and the
/// values it had by then.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TryResult {
    pub status: RunStatus,
    pub steps: Vec<StepRun>,
    pub values: BTreeMap<String, RunValue>,
    /// The question it waits on, when `waiting`.
    pub question: Option<Question>,
    /// Why it stopped, when `failed`.
    pub error: Option<RunError>,
}

pub struct Ports {
    pub clock: Arc<dyn Clock>,
    pub notifier: Arc<dyn Notifier>,
    pub events: Arc<dyn EngineEvents>,
    pub trash: Arc<dyn Trash>,
    pub watcher: Arc<dyn Watcher>,
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

/// How many runs of each workflow the history keeps (decision 7).
pub const KEEP_RUNS: usize = 1000;

/// How often `stop` looks whether the runs in progress have stopped.
const STOP_POLL: Duration = Duration::from_millis(20);

/// The longest the schedule clock sleeps, so waking from sleep or a change
/// of time zone is noticed within a minute.
const SCHEDULE_NAP: Duration = Duration::from_secs(60);

#[derive(Clone)]
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
    intake: Arc<Intake>,
    schedules: Mutex<Schedules>,
    /// Where file actions a crash cut off put files, until `start` records
    /// them as Vela's.
    recovered: Mutex<Vec<PathBuf>>,
    /// Turning workflows on and off, one at a time.
    reloading: Mutex<()>,
    /// What the workflow list shows without reading every run.
    index: Mutex<Index>,
    notices: Notices,
    /// Pause all: no new runs from folders or schedules. Kept in `paused_path`.
    paused: AtomicBool,
    paused_path: PathBuf,
    /// The app is quitting: nothing new starts, and runs stop between steps.
    stopping: AtomicBool,
    /// One queue per workflow, so its runs go one at a time, in order.
    queues: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
}

impl Engine {
    /// Opens the engine on the app's data folder. File actions a crash left
    /// half-known are settled from the journal first. Runs left queued or
    /// running when the app last stopped are marked interrupted; none restarts
    /// on its own (decision 5). Nothing is watched until `start`.
    pub fn new(dir: &DataDir, home: PathBuf, ports: Ports) -> io::Result<Self> {
        let recovered = files::recover_all(&dir.journal_path())?;
        let engine_dir = dir.root().join("engine");
        let notices = Notices::load(
            engine_dir.join("notifications.json"),
            ports.notifier.clone(),
            ports.events.clone(),
        );
        let inner = Inner {
            runs: RunStore::new(dir),
            workflows: WorkflowStore::new(dir),
            settings: SettingsStore::new(dir),
            journal: dir.journal_path(),
            home,
            ports,
            intake: Arc::new(Intake::new(engine_dir.join("seen"))),
            schedules: Mutex::new(Schedules::load(engine_dir.join("schedules.json"))),
            recovered: Mutex::new(recovered.touched),
            reloading: Mutex::new(()),
            index: Mutex::new(Index::default()),
            notices,
            paused: AtomicBool::new(engine_dir.join("paused").exists()),
            paused_path: engine_dir.join("paused"),
            stopping: AtomicBool::new(false),
            queues: Mutex::new(HashMap::new()),
        };
        for mut run in inner.runs.all()? {
            if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
                run.status = RunStatus::Interrupted;
                inner.runs.save(&run)?;
            }
            lock(&inner.index).note(&run);
        }
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Turns on every workflow that is on, runs the files that arrived in
    /// their folders while the app was quit (oldest first), and starts the
    /// schedule clock. Must be called inside a Tokio runtime.
    pub fn start(&self) {
        let inner = &self.inner;
        let ids = match inner.workflows.list() {
            Ok(listed) => listed.into_iter().map(|l| l.id).collect(),
            Err(e) => {
                eprintln!("vela: couldn't list the workflows: {e}");
                Vec::new()
            }
        };
        let _reloading = inner.reloading();
        let mut carrying_on = Vec::new();
        for id in &ids {
            if let Some(false) = inner.turn_on_or_off(id) {
                carrying_on.push(id.clone());
            }
        }
        // Files a crash cut off mid-write are Vela's, not new.
        for path in std::mem::take(&mut *lock(&inner.recovered)) {
            inner.intake.ours(&path);
        }
        inner.ports.watcher.watch(&inner.intake.folders());
        for id in carrying_on {
            Inner::take(inner, &id);
        }
        inner.check_schedules();
        tokio::spawn(keep_time(Arc::downgrade(inner)));
    }

    /// Picks up a change to a workflow: saved, applied, turned on or off, or
    /// deleted. The running version is whatever is saved now.
    pub fn reload(&self, workflow_id: &str) {
        let inner = &self.inner;
        let _reloading = inner.reloading();
        if inner.is_stopping() {
            return;
        }
        let carrying_on = inner.turn_on_or_off(workflow_id) == Some(false);
        inner.ports.watcher.watch(&inner.intake.folders());
        if carrying_on {
            Inner::take(inner, workflow_id);
        }
    }

    /// Something changed at these paths: looks for new files in every
    /// watched folder they touch, and queues a run for each.
    pub fn folders_changed(&self, paths: &[PathBuf]) {
        let inner = &self.inner;
        for id in inner.intake.touched_by(paths) {
            Inner::take(inner, &id);
        }
    }

    /// Queues a run of each scheduled workflow that is due.
    pub fn check_schedules(&self) {
        self.inner.check_schedules();
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
            let run = inner
                .queued(&workflow, TriggerKind::RunNow, Some(file))
                .map_err(history_error)?;
            queued.push(run);
        }
        for run in &queued {
            Inner::enqueue(inner, &run.workflow_id, &run.id);
        }
        Ok(queued.iter().map(Run::summary).collect())
    }

    /// Try on a file: runs `workflow` as the editor has it on one file, with
    /// every file action worked out, not done. It writes nothing, records no
    /// run, tells nobody, and leaves the file unseen. `answers` are the
    /// person's answers so far, by Ask me step; a try without one for the
    /// next question waits on it. A try stops at the first step on its path
    /// with a problem.
    pub async fn try_on_file(
        &self,
        workflow: Workflow,
        file: String,
        answers: BTreeMap<String, String>,
    ) -> Result<TryResult, ApiError> {
        let inner = &self.inner;
        let kind =
            match workflow
                .steps
                .iter()
                .find(|s| s.kind.is_trigger())
                .map(|s| &s.kind)
            {
                Some(StepKind::FileAdded { .. }) => TriggerKind::FileAdded,
                Some(StepKind::RunNow { .. }) => TriggerKind::RunNow,
                Some(_) => return Err(invalid(
                    "A scheduled workflow runs without a file, so there's no file to try it on.",
                )),
                None => return Err(invalid("Add a trigger before trying this workflow.")),
            };
        let file = chosen_file(&file, &inner.home)?;
        let working = inner
            .settings
            .load()
            .map_err(ApiError::from)?
            .settings
            .working_kinds();
        let mut problems = HashMap::new();
        for problem in validate(&workflow, &working) {
            match problem.step_id {
                Some(id) => {
                    problems.entry(id).or_insert(problem.message);
                }
                None => return Err(invalid(problem.message)),
            }
        }

        let mut run = Run {
            id: "try".into(),
            workflow_id: workflow.id.clone(),
            revision: workflow.revision,
            workflow: workflow.clone(),
            trigger: RunTrigger {
                kind,
                file: Some(file.clone()),
            },
            file: Some(file.clone()),
            undo: None,
            waiting_for: None,
            continue_at: None,
            dismissed: false,
            status: RunStatus::Running,
            started_at: inner.ports.now(),
            ended_at: None,
            steps: Vec::new(),
            values: Default::default(),
            error: None,
        };
        let grants = Grants::for_run(&workflow, Some(&file.path), &inner.home);
        let mut ctx = runner::Ctx {
            home: &inner.home,
            ports: &inner.ports,
            notices: &inner.notices,
            stopping: &inner.stopping,
            files: Box::new(Plan::new(grants)),
            trying: Some(runner::Trying {
                answers: &answers,
                problems: &problems,
            }),
        };
        let ended = runner::run(&mut run, &mut ctx, |_| {}).await;
        let (status, error) = match ended {
            Ok(runner::Ended::Done) => (RunStatus::Done, None),
            Ok(runner::Ended::Waiting) => (RunStatus::Waiting, None),
            Ok(runner::Ended::Interrupted) => (RunStatus::Interrupted, None),
            Err(error) => (RunStatus::Failed, Some(error)),
        };
        Ok(TryResult {
            status,
            steps: run.steps,
            values: run.values,
            question: run.waiting_for,
            error,
        })
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
        let report = files::undo(
            &inner.journal,
            &run.id,
            inner.ports.trash.as_ref(),
            inner.intake.as_ref(),
        )
        .map_err(|e| ApiError::new(ErrorCode::Io, format!("Couldn't undo the run: {e}.")))?;
        run.status = RunStatus::Undone;
        run.undo = Some(report.clone());
        inner.runs.save(&run).map_err(history_error)?;
        inner.announce(&run);
        if !report.left_alone.is_empty() {
            let n = report.left_alone.len();
            let message = format!(
                "Undo put back {} of this run's changes. {n} file{} changed since, so {} left alone.",
                report.restored,
                if n == 1 { "" } else { "s" },
                if n == 1 { "it was" } else { "they were" },
            );
            let _ = inner
                .notices
                .tell(NoticeKind::Undo, &run, &message, inner.ports.now());
        }
        Ok(UndoResult { run, report })
    }

    /// The notifications under the bell, newest first.
    pub fn list_notices(&self) -> Vec<Notice> {
        self.inner.notices.list()
    }

    /// Marks these notifications read, or all of them when `ids` is `None`.
    pub fn mark_notices_read(&self, ids: Option<Vec<String>>) {
        self.inner.notices.mark_read(ids.as_deref());
    }

    /// Empties the notification list; runs and their history are untouched.
    pub fn clear_notices(&self) {
        self.inner.notices.clear();
    }

    pub fn activity(&self) -> Activity {
        self.inner.activity(&lock(&self.inner.index))
    }

    /// Pause all: while paused, new files wait and scheduled times are passed
    /// over; runs already queued or running finish, and Run now still works.
    /// Resuming looks in every watched folder at once, so files that arrived
    /// meanwhile run.
    pub fn pause_all(&self, paused: bool) -> Result<Activity, ApiError> {
        let inner = &self.inner;
        let saved = if paused {
            fs::create_dir_all(inner.paused_path.parent().expect("in engine/"))
                .and_then(|()| fs::write(&inner.paused_path, b""))
        } else {
            match fs::remove_file(&inner.paused_path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        };
        saved.map_err(|e| ApiError::new(ErrorCode::Io, format!("Couldn't save the pause: {e}")))?;
        let was = {
            let index = lock(&inner.index);
            let was = inner.paused.swap(paused, Ordering::SeqCst);
            if was != paused {
                inner.ports.events.activity_changed(inner.activity(&index));
            }
            was
        };
        if was && !paused {
            let folders: Vec<PathBuf> =
                inner.intake.folders().into_iter().map(|(f, _)| f).collect();
            self.folders_changed(&folders);
        }
        Ok(self.activity())
    }

    /// Stops for the app quitting (decision 3): folders are no longer
    /// watched, schedules no longer fire, and nothing new starts. Runs still
    /// in the queue are marked interrupted at once. Runs in progress stop
    /// after the step they are on; any still going after `wait` are marked
    /// interrupted as they stand, and the next start settles their journals.
    /// Files that arrive from now on aren't recorded, so the next start runs them.
    pub fn stop(&self, wait: Duration) {
        let inner = &self.inner;
        {
            let _reloading = inner.reloading();
            inner.stopping.store(true, Ordering::SeqCst);
            inner.ports.watcher.watch(&[]);
        }
        let busy: Vec<String> = lock(&inner.index).busy.iter().cloned().collect();
        for id in busy {
            if let Ok(Some(run)) = inner.runs.get(&id) {
                if run.status == RunStatus::Queued {
                    inner.interrupt(run);
                }
            }
        }
        let until = std::time::Instant::now() + wait;
        while !lock(&inner.index).busy.is_empty() && std::time::Instant::now() < until {
            std::thread::sleep(STOP_POLL);
        }
        let left: Vec<String> = lock(&inner.index).busy.iter().cloned().collect();
        for id in left {
            if let Ok(Some(run)) = inner.runs.get(&id) {
                if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
                    inner.interrupt(run);
                }
            }
        }
    }

    /// Everything waiting on the person, newest first.
    pub fn list_needs_you(&self) -> Result<Vec<NeedsYouItem>, ApiError> {
        let ids: Vec<String> = lock(&self.inner.index).needs.keys().cloned().collect();
        let mut items: Vec<NeedsYouItem> = ids
            .iter()
            .filter_map(|id| self.inner.runs.get(id).ok().flatten())
            .filter_map(|run| run.needs_you())
            .collect();
        items.sort_by_cached_key(|i| std::cmp::Reverse(newness(&i.run)));
        Ok(items)
    }

    /// For each workflow with runs: how many need the person, and its newest run.
    pub fn workflow_runs(&self) -> HashMap<String, (u32, RunSummary)> {
        let index = lock(&self.inner.index);
        index
            .last
            .iter()
            .map(|(wf, last)| {
                let needs = index.needs.values().filter(|w| *w == wf).count() as u32;
                (wf.clone(), (needs, last.clone()))
            })
            .collect()
    }

    /// Answers a waiting run's question; the run carries on down that branch.
    pub fn answer(&self, run_id: &str, branch_id: &str) -> Result<Run, ApiError> {
        let inner = &self.inner;
        let mut run = self.get_run(run_id)?;
        let question = match (&run.status, &run.waiting_for) {
            (RunStatus::Waiting, Some(q)) => q.clone(),
            _ => {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    "This question was already answered.",
                ))
            }
        };
        let Some(answer) = question.answers.iter().find(|a| a.id == branch_id) else {
            return Err(invalid("That isn't one of the answers to this question."));
        };
        if let Some(entry) = run
            .steps
            .iter_mut()
            .rev()
            .find(|s| s.step_id == question.step_id && s.outcome == StepOutcome::Waiting)
        {
            entry.outcome = StepOutcome::Done;
            entry.branch = Some(answer.id.clone());
            entry.ended_at = Some(inner.ports.now());
            entry.message = Some(format!("You answered {}.", answer.label));
        }
        run.waiting_for = None;
        run.continue_at = runner::after(&run, &question.step_id, Some(branch_id));
        if run.continue_at.is_some() {
            run.status = RunStatus::Queued;
        } else {
            run.status = RunStatus::Done;
            run.ended_at = Some(inner.ports.now());
        }
        inner.runs.save(&run).map_err(history_error)?;
        inner.announce(&run);
        if run.status == RunStatus::Queued {
            Inner::enqueue(inner, &run.workflow_id, &run.id);
        }
        Ok(run)
    }

    /// Runs a failed run again from the step that failed, with the values it had.
    pub fn retry_run(&self, run_id: &str) -> Result<Run, ApiError> {
        let run = self.get_run(run_id)?;
        if run.status != RunStatus::Failed {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "Only a failed run can be retried.",
            ));
        }
        let at = run.error.as_ref().and_then(|e| e.step_id.clone());
        self.requeue(run, at)
    }

    /// Carries on an interrupted run from where it stopped. A step that was
    /// cut off runs again.
    pub fn resume_run(&self, run_id: &str) -> Result<Run, ApiError> {
        let run = self.get_run(run_id)?;
        if run.status != RunStatus::Interrupted {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "Only a run Vela stopped by quitting can be resumed.",
            ));
        }
        let at = match run.steps.last() {
            None => None,
            Some(last) if last.outcome == StepOutcome::Done => {
                match runner::after(&run, &last.step_id, last.branch.as_deref()) {
                    Some(next) => Some(next),
                    // It had finished its last step: nothing is left to do.
                    None => return self.finish_done(run),
                }
            }
            Some(last) => Some(last.step_id.clone()),
        };
        self.requeue(run, at)
    }

    /// Takes a failed or interrupted run out of Needs you, changing nothing else.
    pub fn dismiss_run(&self, run_id: &str) -> Result<(), ApiError> {
        let mut run = self.get_run(run_id)?;
        if !matches!(run.status, RunStatus::Failed | RunStatus::Interrupted) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "Only a failed or stopped run can be dismissed.",
            ));
        }
        run.dismissed = true;
        self.inner.runs.save(&run).map_err(history_error)?;
        self.inner.announce(&run);
        Ok(())
    }

    fn requeue(&self, mut run: Run, at: Option<String>) -> Result<Run, ApiError> {
        let inner = &self.inner;
        run.continue_at = at;
        run.error = None;
        run.ended_at = None;
        run.dismissed = false;
        run.status = RunStatus::Queued;
        inner.runs.save(&run).map_err(history_error)?;
        inner.announce(&run);
        Inner::enqueue(inner, &run.workflow_id, &run.id);
        Ok(run)
    }

    fn finish_done(&self, mut run: Run) -> Result<Run, ApiError> {
        run.status = RunStatus::Done;
        run.ended_at = Some(self.inner.ports.now());
        self.inner.runs.save(&run).map_err(history_error)?;
        self.inner.announce(&run);
        Ok(run)
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
    fn reloading(&self) -> MutexGuard<'_, ()> {
        lock(&self.reloading)
    }

    /// Starts or stops watching and scheduling for a workflow, to match its
    /// saved file. Returns whether a File added workflow is on and carries on
    /// from its record (`Some(false)`), is newly on with the files there now
    /// left alone (`Some(true)`), or isn't watching (`None`).
    fn turn_on_or_off(&self, id: &str) -> Option<bool> {
        let workflow = match self.workflows.get(id) {
            Ok(workflow) => Some(workflow),
            Err(WorkflowError::NotFound | WorkflowError::InvalidId) => {
                self.report(id, self.intake.forget(id));
                self.report(id, lock(&self.schedules).turn_off(id));
                return None;
            }
            Err(_) => None,
        };
        let trigger = workflow
            .as_ref()
            .filter(|w| w.enabled)
            .and_then(|w| w.steps.iter().find(|s| s.kind.is_trigger()))
            .map(|s| &s.kind);

        let watch = match trigger {
            Some(StepKind::FileAdded {
                folder,
                subfolders,
                file_types,
                ..
            }) => expand_home(folder, &self.home).map(|folder| Watch {
                folder: fs::canonicalize(&folder).unwrap_or(folder),
                subfolders: *subfolders,
                file_types: file_types.iter().map(|t| t.to_lowercase()).collect(),
            }),
            _ => None,
        };
        let mut fresh = None;
        match watch {
            Some(watch) => match self.intake.turn_on(id, watch) {
                Ok(f) => fresh = Some(f),
                Err(e) => self.report(id, Err(e)),
            },
            None => self.report(id, self.intake.turn_off(id)),
        }

        let mut schedules = lock(&self.schedules);
        match trigger {
            Some(StepKind::Schedule { schedule, .. }) => {
                let now = self.ports.clock.now().with_timezone(&Local);
                self.report(id, schedules.turn_on(id, schedule, &now));
            }
            _ => self.report(id, schedules.turn_off(id)),
        }
        fresh
    }

    fn report(&self, id: &str, result: io::Result<()>) {
        if let Err(e) = result {
            eprintln!("vela: couldn't update the records of workflow {id}: {e}");
        }
    }

    /// Queues a run for each new file in a watched workflow's folder. While
    /// paused, the files wait: they aren't recorded, so resuming takes them.
    fn take(this: &Arc<Self>, id: &str) {
        if this.is_paused() || this.is_stopping() {
            return;
        }
        let files = match this.intake.take(id) {
            Ok(files) => files,
            Err(e) => {
                eprintln!("vela: couldn't look for new files for workflow {id}: {e}");
                return;
            }
        };
        if files.is_empty() {
            return;
        }
        let Ok(workflow) = this.workflows.get(id) else {
            return;
        };
        for file in files {
            match this.queued(&workflow, TriggerKind::FileAdded, Some(file)) {
                Ok(run) => Inner::enqueue(this, id, &run.id),
                Err(e) => eprintln!("vela: couldn't queue a run: {e}"),
            }
        }
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    fn activity(&self, index: &Index) -> Activity {
        Activity {
            paused: self.is_paused(),
            running: index.busy.len() as u32,
            needs_you: index.needs.len() as u32,
        }
    }

    fn interrupt(&self, mut run: Run) {
        run.status = RunStatus::Interrupted;
        self.save(&run);
        self.announce(&run);
    }

    fn check_schedules(self: &Arc<Self>) {
        if self.is_stopping() {
            return;
        }
        let now = self.ports.clock.now().with_timezone(&Local);
        let due = match lock(&self.schedules).due(&now) {
            Ok(due) => due,
            Err(e) => {
                eprintln!("vela: couldn't update the schedules: {e}");
                return;
            }
        };
        // Paused: the times count as checked, and pass by.
        if self.is_paused() {
            return;
        }
        for id in due {
            let Ok(workflow) = self.workflows.get(&id) else {
                continue;
            };
            if !workflow.enabled {
                continue;
            }
            match self.queued(&workflow, TriggerKind::Schedule, None) {
                Ok(run) => Inner::enqueue(self, &id, &run.id),
                Err(e) => eprintln!("vela: couldn't queue a run: {e}"),
            }
        }
    }

    /// Saves a new queued run of `workflow` and tells the window. The caller
    /// hands it to the queue.
    fn queued(
        &self,
        workflow: &Workflow,
        kind: TriggerKind,
        file: Option<RunFile>,
    ) -> io::Result<Run> {
        let run = Run {
            id: uuid::Uuid::new_v4().to_string(),
            workflow_id: workflow.id.clone(),
            revision: workflow.revision,
            workflow: workflow.clone(),
            trigger: RunTrigger {
                kind,
                file: file.clone(),
            },
            file,
            undo: None,
            waiting_for: None,
            continue_at: None,
            dismissed: false,
            status: RunStatus::Queued,
            started_at: self.ports.now(),
            ended_at: None,
            steps: Vec::new(),
            values: Default::default(),
            error: None,
        };
        self.runs.save(&run)?;
        self.announce(&run);
        Ok(run)
    }

    fn announce(&self, run: &Run) {
        {
            // Told under the lock, so the last told is the latest.
            let mut index = lock(&self.index);
            let before = self.activity(&index);
            index.note(run);
            let after = self.activity(&index);
            if after != before {
                self.ports.events.activity_changed(after);
            }
        }
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
        if run.status != RunStatus::Queued || self.is_stopping() {
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
        let writes: Arc<dyn Writes> = self.intake.clone();
        let result = match Files::open(&self.journal, &run.id, grants) {
            Ok(files) => {
                let mut ctx = runner::Ctx {
                    home: &self.home,
                    ports: &self.ports,
                    notices: &self.notices,
                    stopping: &self.stopping,
                    files: Box::new(files.telling(writes)),
                    trying: None,
                };
                runner::run(&mut run, &mut ctx, |r| self.save(r)).await
            }
            Err(e) => Err(runs::RunError {
                step_id: None,
                message: format!("Couldn't open the run's journal: {e}."),
            }),
        };

        match result {
            Ok(runner::Ended::Waiting) => {
                run.status = RunStatus::Waiting;
                if let Some(q) = &run.waiting_for {
                    let _ = self.notices.tell(
                        NoticeKind::Question,
                        &run,
                        &q.question,
                        self.ports.now(),
                    );
                }
            }
            Ok(runner::Ended::Interrupted) => {
                run.status = RunStatus::Interrupted;
            }
            Ok(runner::Ended::Done) => {
                run.ended_at = Some(self.ports.now());
                run.status = RunStatus::Done;
            }
            Err(error) => {
                run.ended_at = Some(self.ports.now());
                self.tell_failed(&run, &error.step_id, &error.message);
                run.error = Some(error);
                run.status = RunStatus::Failed;
            }
        }
        self.save(&run);
        self.announce(&run);
        if run.status == RunStatus::Done {
            self.prune(&run.workflow_id);
        }
    }

    /// Keeps the history to the newest runs; see `KEEP_RUNS`.
    fn prune(&self, workflow_id: &str) {
        let result = self.runs.prune(workflow_id, KEEP_RUNS, |id| {
            if let Err(e) = files::forget(&self.journal, id) {
                eprintln!("vela: couldn't remove the journal of run {id}: {e}");
            }
            lock(&self.index).forget(id);
        });
        if let Err(e) = result {
            eprintln!("vela: couldn't tidy the run history: {e}");
        }
    }

    /// A save that fails mid-run can't stop the run; the next save retries.
    fn save(&self, run: &Run) {
        if let Err(e) = self.runs.save(run) {
            eprintln!("vela: couldn't save run {}: {e}", run.id);
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
        let _ = self.notices.tell(
            NoticeKind::Failed,
            run,
            &format!("{step} failed{on}: {message}"),
            self.ports.now(),
        );
    }
}

/// Wakes the scheduled workflows at their times, until the engine is gone.
async fn keep_time(engine: Weak<Inner>) {
    loop {
        let nap = {
            let Some(inner) = engine.upgrade() else {
                return;
            };
            let now = inner.ports.clock.now().with_timezone(&Local);
            let next = lock(&inner.schedules).next(&now);
            next.and_then(|next| (next - now).to_std().ok())
                .map_or(SCHEDULE_NAP, |until| until.min(SCHEDULE_NAP))
        };
        tokio::time::sleep(nap.max(Duration::from_millis(500))).await;
        let Some(inner) = engine.upgrade() else {
            return;
        };
        inner.check_schedules();
    }
}

/// The runs that need the person, and each workflow's newest run.
#[derive(Default)]
struct Index {
    /// Run id → workflow id.
    needs: std::collections::BTreeMap<String, String>,
    last: HashMap<String, RunSummary>,
    /// Runs queued or running.
    busy: std::collections::BTreeSet<String>,
}

impl Index {
    fn note(&mut self, run: &Run) {
        if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
            self.busy.insert(run.id.clone());
        } else {
            self.busy.remove(&run.id);
        }
        if run.needs_you().is_some() {
            self.needs.insert(run.id.clone(), run.workflow_id.clone());
        } else {
            self.needs.remove(&run.id);
        }
        let summary = run.summary();
        let newer = self
            .last
            .get(&run.workflow_id)
            .is_none_or(|last| last.id == run.id || newness(last) < newness(&summary));
        if newer {
            self.last.insert(run.workflow_id.clone(), summary);
        }
    }

    fn forget(&mut self, run_id: &str) {
        self.needs.remove(run_id);
    }
}

/// Orders runs by when they were queued, whatever the offset they were written with.
fn newness(run: &RunSummary) -> (Option<DateTime<FixedOffset>>, String) {
    (
        DateTime::parse_from_rfc3339(&run.started_at).ok(),
        run.id.clone(),
    )
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
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
        return Err(invalid(format!("{name} isn't a file Vela can run on.")));
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
