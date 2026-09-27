//! What FolderFlow tells the person: questions, failed runs, Notify steps and
//! undos that left files alone. Each goes to macOS's notifications and to the
//! list under the bell, kept in `engine/notifications.json`, so nothing is
//! missed when macOS doesn't show it.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::runs::Run;
use super::{EngineEvents, Notifier};
use crate::storage::atomic::write_atomic;

/// How many notifications the list keeps; older ones drop off.
pub const KEEP: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Notice {
    pub id: String,
    pub kind: NoticeKind,
    pub workflow_id: String,
    pub workflow_name: String,
    /// The run it's about; clicking it opens the run.
    pub run_id: Option<String>,
    pub message: String,
    /// RFC 3339.
    pub at: String,
    pub read: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoticeKind {
    /// An Ask me step waiting for an answer.
    Question,
    Failed,
    /// A Notify step's message.
    Message,
    /// An undo that left files alone.
    Undo,
}

pub struct Notices {
    path: PathBuf,
    list: Mutex<Vec<Notice>>,
    notifier: Arc<dyn Notifier>,
    events: Arc<dyn EngineEvents>,
}

impl Notices {
    /// The list at `path`; empty if it's missing or damaged.
    pub fn load(
        path: PathBuf,
        notifier: Arc<dyn Notifier>,
        events: Arc<dyn EngineEvents>,
    ) -> Notices {
        let list = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Notices {
            path,
            list: Mutex::new(list),
            notifier,
            events,
        }
    }

    /// Adds a notification about `run` and shows it in macOS. Returns why
    /// macOS didn't show it, if it didn't; it's in the list either way.
    pub fn tell(
        &self,
        kind: NoticeKind,
        run: &Run,
        message: &str,
        at: String,
    ) -> Result<(), String> {
        let notice = Notice {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            workflow_id: run.workflow_id.clone(),
            workflow_name: run.workflow.name.clone(),
            run_id: Some(run.id.clone()),
            message: message.to_owned(),
            at,
            read: false,
        };
        {
            let mut list = self.lock();
            list.insert(0, notice);
            list.truncate(KEEP);
            self.save(&list);
            self.events.notices_changed(unread(&list));
        }
        self.notifier.notify(&run.workflow.name, message)
    }

    /// Newest first.
    pub fn list(&self) -> Vec<Notice> {
        self.lock().clone()
    }

    /// Marks these notifications read, or all of them.
    pub fn mark_read(&self, ids: Option<&[String]>) {
        let mut list = self.lock();
        for n in list.iter_mut() {
            if ids.is_none_or(|ids| ids.contains(&n.id)) {
                n.read = true;
            }
        }
        self.save(&list);
        self.events.notices_changed(unread(&list));
    }

    /// Empties the list. Runs and their history are untouched.
    pub fn clear(&self) {
        let mut list = self.lock();
        list.clear();
        self.save(&list);
        self.events.notices_changed(0);
    }

    fn save(&self, list: &[Notice]) {
        let result = (|| -> io::Result<()> {
            if let Some(folder) = self.path.parent() {
                fs::create_dir_all(folder)?;
            }
            let bytes = serde_json::to_vec_pretty(list).map_err(io::Error::other)?;
            write_atomic(&self.path, &bytes)
        })();
        if let Err(e) = result {
            eprintln!("folderflow: couldn't save the notifications: {e}");
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Notice>> {
        self.list.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn unread(list: &[Notice]) -> u32 {
    list.iter().filter(|n| !n.read).count() as u32
}
