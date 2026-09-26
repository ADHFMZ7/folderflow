//! Helpers for the engine tests: an engine on a temp data folder, with a fixed
//! clock, and notifications and window events recorded instead of shown.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use folderflow_lib::engine::runs::{Run, RunStatus};
use folderflow_lib::engine::{Clock, Engine, EngineEvents, Notifier, Ports, RunChanged};
use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::storage::workflows::WorkflowStore;
use folderflow_lib::workflow::Workflow;
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

/// Sends every event down a channel the test reads.
pub struct Events(pub mpsc::UnboundedSender<RunChanged>);

impl EngineEvents for Events {
    fn run_changed(&self, change: RunChanged) {
        let _ = self.0.send(change);
    }
}

pub struct EngineHarness {
    pub _tmp: tempfile::TempDir,
    /// The data folder.
    pub root: PathBuf,
    /// Stands in for the home folder; `~/` paths resolve here.
    pub home: PathBuf,
    pub notes: Arc<Notes>,
    pub events: mpsc::UnboundedReceiver<RunChanged>,
    pub engine: Engine,
}

pub fn engine() -> EngineHarness {
    engine_with(Notes::default())
}

pub fn engine_with(notes: Notes) -> EngineHarness {
    let (tmp, dir) = super::data_dir();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    start(tmp, dir, home, notes)
}

/// Starts an engine on a data folder, as the app does at launch.
pub fn start(tmp: tempfile::TempDir, dir: DataDir, home: PathBuf, notes: Notes) -> EngineHarness {
    let root = dir.root().to_path_buf();
    let notes = Arc::new(notes);
    let (tx, rx) = mpsc::unbounded_channel();
    let engine = Engine::new(
        &dir,
        home.clone(),
        Ports {
            clock: Arc::new(TickingClock::new()),
            notifier: notes.clone(),
            events: Arc::new(Events(tx)),
        },
    )
    .unwrap();
    EngineHarness {
        _tmp: tmp,
        root,
        home,
        notes,
        events: rx,
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
            ..
        } = self;
        drop(engine);
        start(_tmp, DataDir::open(&root).unwrap(), home, Notes::default())
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }
}

pub fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
