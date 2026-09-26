//! The engine's ports in the running app: the Mac's notifications, and Tauri
//! events to the window.

use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use super::{EngineEvents, Notifier, RunChanged};

pub struct AppNotifier(pub AppHandle);

impl Notifier for AppNotifier {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| e.to_string())
    }
}

pub struct AppEvents(pub AppHandle);

impl EngineEvents for AppEvents {
    fn run_changed(&self, change: RunChanged) {
        let _ = self.0.emit("run-changed", change);
    }
}
