//! The engine's ports in the running app: the Mac's notifications, and Tauri
//! events to the window.

use tauri::{AppHandle, Emitter};

use super::{EngineEvents, Notifier, RunChanged};

/// Notifications from FolderFlow, sent and waited on here, so one macOS refuses
/// is recorded on its step instead of lost.
pub struct AppNotifier {
    /// Why notifications can't be shown at all, if they can't.
    unavailable: Option<String>,
}

impl AppNotifier {
    /// Sends as the app with this bundle id. A development build isn't a bundle,
    /// so it borrows the id, which macOS knows only once FolderFlow.app has
    /// been opened; until then every notification fails, and says why.
    pub fn new(identifier: &str) -> Self {
        let unavailable = notify_rust::set_application(identifier).err().map(|_| {
            "macOS doesn't know FolderFlow yet. Open FolderFlow.app once, then try again."
                .to_string()
        });
        Self { unavailable }
    }
}

impl Notifier for AppNotifier {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        if let Some(why) = &self.unavailable {
            return Err(why.clone());
        }
        notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

pub struct AppEvents(pub AppHandle);

impl EngineEvents for AppEvents {
    fn run_changed(&self, change: RunChanged) {
        let _ = self.0.emit("run-changed", change);
    }
}
