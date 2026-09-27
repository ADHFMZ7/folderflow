//! Helpers for the engine tests: an engine on a temp data folder, with a clock
//! the test sets, notifications and window events recorded instead of shown,
//! and folder changes told to the engine by the test.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use folderflow_lib::engine::runs::{Run, RunStatus};
use folderflow_lib::engine::{
    Activity, Clock, Engine, EngineEvents, Notifier, Ports, RunChanged, Watcher,
};
use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::storage::workflows::WorkflowStore;
use folderflow_lib::workflow::Workflow;

use super::files::FolderTrash;
use serde_json::Value;
use tokio::sync::mpsc;

/// 2026-09-14 10:00 in a UTC+2 time zone.
pub const NOW: &str = "2026-09-14T10:00:00+02:00";

/// Starts at `NOW` and moves on a second each time it's read, so every time
/// the engine records is distinct and in order.
pub struct TickingClock(Mutex<DateTime<FixedOffset>>);

impl TickingClock {
    pub fn new() -> Self {
        Self(Mutex::new(DateTime::parse_from_rfc3339(NOW).unwrap()))
    }

    /// Jumps to `time`, as the Mac waking from sleep does.
    pub fn set(&self, time: DateTime<FixedOffset>) {
        *self.0.lock().unwrap() = time;
    }
}

impl Clock for TickingClock {
    fn now(&self) -> DateTime<FixedOffset> {
        let mut now = self.0.lock().unwrap();
        let this = *now;
        *now += chrono::Duration::seconds(1);
        this
    }
}

/// Every notification shown, as (title, body).
#[derive(Default)]
pub struct Notes {
    pub shown: Mutex<Vec<(String, String)>>,
    pub refuse: bool,
    /// Called as each notification is shown, for a test to act mid-run.
    pub then: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl Notifier for Notes {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        if self.refuse {
            return Err("Notifications are off for FolderFlow".into());
        }
        self.shown
            .lock()
            .unwrap()
            .push((title.to_owned(), body.to_owned()));
        if let Some(then) = &*self.then.lock().unwrap() {
            then();
        }
        Ok(())
    }
}

impl Notes {
    pub fn bodies(&self) -> Vec<String> {
        self.shown
            .lock()
            .unwrap()
            .iter()
            .map(|(_, b)| b.clone())
            .collect()
    }
}

/// Sends every run event down a channel the test reads, and keeps every
/// activity event.
pub struct Events {
    pub runs: mpsc::UnboundedSender<RunChanged>,
    pub activity: Arc<Mutex<Vec<Activity>>>,
}

impl EngineEvents for Events {
    fn run_changed(&self, change: RunChanged) {
        let _ = self.runs.send(change);
    }

    fn activity_changed(&self, activity: Activity) {
        self.activity.lock().unwrap().push(activity);
    }
}

/// The folders the engine last asked to watch.
#[derive(Default)]
pub struct Watched(pub Mutex<Vec<(PathBuf, bool)>>);

impl Watcher for Watched {
    fn watch(&self, folders: &[(PathBuf, bool)]) {
        *self.0.lock().unwrap() = folders.to_vec();
    }
}

pub struct EngineHarness {
    pub _tmp: tempfile::TempDir,
    /// The data folder.
    pub root: PathBuf,
    /// Stands in for the home folder; `~/` paths resolve here.
    pub home: PathBuf,
    pub notes: Arc<Notes>,
    pub clock: Arc<TickingClock>,
    pub watched: Arc<Watched>,
    pub events: mpsc::UnboundedReceiver<RunChanged>,
    /// Every `activity-changed` event, in order.
    pub activity: Arc<Mutex<Vec<Activity>>>,
    pub engine: Engine,
}

pub fn engine() -> EngineHarness {
    engine_with(Notes::default())
}

pub fn engine_with(notes: Notes) -> EngineHarness {
    let (tmp, dir) = super::data_dir();
    // Resolved, so it matches the paths the engine records.
    let home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    fs::create_dir_all(&home).unwrap();
    start(tmp, dir, home, notes)
}

/// Starts an engine on a data folder, as the app does at launch.
pub fn start(tmp: tempfile::TempDir, dir: DataDir, home: PathBuf, notes: Notes) -> EngineHarness {
    start_at(tmp, dir, home, notes, Arc::new(TickingClock::new()))
}

fn start_at(
    tmp: tempfile::TempDir,
    dir: DataDir,
    home: PathBuf,
    notes: Notes,
    clock: Arc<TickingClock>,
) -> EngineHarness {
    let root = dir.root().to_path_buf();
    let notes = Arc::new(notes);
    let watched = Arc::new(Watched::default());
    let (tx, rx) = mpsc::unbounded_channel();
    let activity = Arc::new(Mutex::new(Vec::new()));
    let engine = Engine::new(
        &dir,
        home.clone(),
        Ports {
            clock: clock.clone(),
            notifier: notes.clone(),
            events: Arc::new(Events {
                runs: tx,
                activity: activity.clone(),
            }),
            trash: Arc::new(FolderTrash(home.parent().unwrap().join("trash"))),
            watcher: watched.clone(),
        },
    )
    .unwrap();
    engine.start();
    EngineHarness {
        _tmp: tmp,
        root,
        home,
        notes,
        clock,
        watched,
        events: rx,
        activity,
        engine,
    }
}

impl EngineHarness {
    /// Saves a workflow file with these steps, as if made in the editor.
    pub fn workflow(&self, name: &str, steps: Value) -> Workflow {
        let workflow: Workflow = serde_json::from_value(serde_json::json!({
            "version": 1,
            "id": uuid::Uuid::new_v4().to_string(),
            "name": name,
            "revision": 1,
            "enabled": false,
            "steps": steps,
        }))
        .unwrap();
        WorkflowStore::new(&DataDir::open(&self.root).unwrap())
            .create(workflow)
            .unwrap()
    }

    /// Saves the workflow on or off, as the switch in the app does.
    pub fn switch(&self, workflow: &Workflow, on: bool) -> Workflow {
        let store = WorkflowStore::new(&DataDir::open(&self.root).unwrap());
        let current = store.get(&workflow.id).unwrap();
        let saved = store
            .save(
                Workflow {
                    enabled: on,
                    ..current
                },
                &Default::default(),
            )
            .unwrap()
            .workflow;
        self.engine.reload(&saved.id);
        saved
    }

    /// Saves a changed workflow, as the editor does.
    pub fn save(&self, workflow: Workflow) -> Workflow {
        let store = WorkflowStore::new(&DataDir::open(&self.root).unwrap());
        let revision = store.get(&workflow.id).unwrap().revision;
        let saved = store
            .save(
                Workflow {
                    revision,
                    ..workflow
                },
                &Default::default(),
            )
            .unwrap()
            .workflow;
        self.engine.reload(&saved.id);
        saved
    }

    /// Tells the engine something changed in `rel`, as FSEvents would.
    pub fn changed(&self, rel: &str) {
        self.engine
            .folders_changed(&[self.home.join(rel.trim_end_matches('/'))]);
    }

    /// Every run so far, oldest first.
    pub fn runs(&self) -> Vec<Run> {
        let mut runs: Vec<Run> = self
            .engine
            .list_runs(Default::default())
            .unwrap()
            .into_iter()
            .map(|r| self.engine.get_run(&r.id).unwrap())
            .collect();
        runs.reverse();
        runs
    }

    /// Waits until every run has finished, and returns them all, oldest first.
    pub async fn settle(&mut self) -> Vec<Run> {
        let wait = async {
            loop {
                let runs = self.runs();
                if runs
                    .iter()
                    .all(|r| !matches!(r.status, RunStatus::Queued | RunStatus::Running))
                {
                    return runs;
                }
                self.events.recv().await.expect("the engine stopped");
            }
        };
        tokio::time::timeout(Duration::from_secs(10), wait)
            .await
            .expect("runs never finished")
    }

    /// A file in the home folder, returned as the `~/` path a picker gives.
    pub fn file(&self, rel: &str) -> String {
        let path = self.home.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"contents").unwrap();
        format!("~/{rel}")
    }

    /// Waits for the run to reach `status`, as the window would hear of it.
    pub async fn until(&mut self, run_id: &str, status: RunStatus) -> Run {
        let wait = async {
            loop {
                let change = self.events.recv().await.expect("the engine stopped");
                if change.run_id == run_id && change.status == status {
                    return;
                }
            }
        };
        // A safety net for a hang, never a wait for work to finish.
        tokio::time::timeout(Duration::from_secs(10), wait)
            .await
            .unwrap_or_else(|_| panic!("run {run_id} never became {status:?}"));
        self.engine.get_run(run_id).unwrap()
    }

    /// Stops this engine and starts a new one on the same data folder, as
    /// quitting and reopening the app does.
    pub fn restart(self) -> EngineHarness {
        let EngineHarness {
            _tmp,
            root,
            home,
            engine,
            clock,
            ..
        } = self;
        drop(engine);
        start_at(
            _tmp,
            DataDir::open(&root).unwrap(),
            home,
            Notes::default(),
            clock,
        )
    }

    /// Connects a model for every AI kind, so AI steps pass validation.
    pub fn set_models(&self) {
        use folderflow_lib::storage::data_dir::DataDir;
        use folderflow_lib::storage::settings::{Connection, ModelRef, Settings, SettingsStore};
        let model = Some(ModelRef {
            connection_id: "c1".into(),
            model_id: "m".into(),
        });
        let settings = Settings {
            connections: vec![Connection {
                id: "c1".into(),
                provider_id: "ollama".into(),
                endpoint: None,
            }],
            defaults: [
                ("llm".to_string(), model.clone()),
                ("system1".to_string(), model),
            ]
            .into(),
            ..Settings::default()
        };
        SettingsStore::new(&DataDir::open(&self.root).unwrap())
            .save(&settings)
            .unwrap();
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.home.parent().unwrap().join("trash")
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }
}

pub fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
