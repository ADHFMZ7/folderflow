//! The engine's ports in the running app: the Mac's notifications, the Trash,
//! folder watching with FSEvents, and Tauri events to the window and the
//! menu bar icon.
//!
//! Notifications use NSUserNotificationCenter, which Apple deprecates in favour
//! of UserNotifications. That framework works only inside an app bundle, and
//! `tauri dev` runs a bare binary, so it would leave development builds silent.
//! Revisit with the signed release build, where UserNotifications can be tested.
#![allow(deprecated)]

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread};
use objc2_foundation::{
    NSFileManager, NSString, NSUserNotification, NSUserNotificationCenter,
    NSUserNotificationCenterDelegate, NSURL,
};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use super::files::Trash;
use super::{Activity, Engine, EngineEvents, Notifier, RunChanged, Watcher};

/// The Mac's Trash, where Finder's "Put Back" can restore from.
pub struct AppTrash;

impl Trash for AppTrash {
    fn trash(&self, path: &std::path::Path) -> std::io::Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, None)
            .map_err(|e| std::io::Error::other(e.localizedDescription().to_string()))
    }
}

/// Notifications from FolderFlow. They show as banners even while FolderFlow
/// is the front app, which macOS otherwise skips: a person who just chose Run
/// is usually looking at FolderFlow.
pub struct AppNotifier {
    /// Why notifications can't be shown at all, if they can't.
    unavailable: Option<String>,
}

impl AppNotifier {
    /// Sends as the app with this bundle id. A development build isn't a bundle,
    /// so it borrows the id, which macOS knows only once FolderFlow.app has
    /// been opened; until then every notification fails, and says why.
    pub fn new(identifier: &str) -> Self {
        let unavailable = mac_notification_sys::set_application(identifier)
            .err()
            .map(|_| {
                "macOS doesn't know FolderFlow yet. Open FolderFlow.app once, then try again."
                    .to_string()
            });
        if unavailable.is_none() {
            let presenter = Presenter::new();
            // SAFETY: the center holds its delegate weakly, so the presenter is
            // kept for the life of the app by leaking it below.
            unsafe {
                NSUserNotificationCenter::defaultUserNotificationCenter()
                    .setDelegate(Some(ProtocolObject::from_ref(&*presenter)));
            }
            let _ = Retained::into_raw(presenter);
        }
        Self { unavailable }
    }
}

impl Notifier for AppNotifier {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        if let Some(why) = &self.unavailable {
            return Err(why.clone());
        }
        let notification = NSUserNotification::new();
        notification.setTitle(Some(&NSString::from_str(title)));
        notification.setInformativeText(Some(&NSString::from_str(body)));
        NSUserNotificationCenter::defaultUserNotificationCenter()
            .deliverNotification(&notification);
        Ok(())
    }
}

define_class!(
    /// Tells macOS to show every FolderFlow notification, front app or not.
    #[unsafe(super(NSObject))]
    #[name = "FolderFlowNotificationPresenter"]
    struct Presenter;

    unsafe impl NSObjectProtocol for Presenter {}

    unsafe impl NSUserNotificationCenterDelegate for Presenter {
        #[unsafe(method(userNotificationCenter:shouldPresentNotification:))]
        fn should_present(
            &self,
            _center: &NSUserNotificationCenter,
            _notification: &NSUserNotification,
        ) -> bool {
            true
        }
    }
);

impl Presenter {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

pub struct AppEvents(pub AppHandle);

impl EngineEvents for AppEvents {
    fn run_changed(&self, change: RunChanged) {
        let _ = self.0.emit("run-changed", change);
    }

    fn notices_changed(&self, unread: u32) {
        let _ = self.0.emit("notices-changed", unread);
    }

    fn activity_changed(&self, activity: Activity) {
        let _ = self.0.emit("activity-changed", activity);
        crate::background::show_activity(&self.0, activity);
    }
}

/// Watches folders with FSEvents, sending each changed path down a channel
/// for `forward_changes`.
pub struct AppWatcher {
    watching: Mutex<(RecommendedWatcher, Vec<(PathBuf, bool)>)>,
}

impl AppWatcher {
    pub fn new(changes: mpsc::UnboundedSender<PathBuf>) -> notify::Result<Self> {
        let watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) => {
                    for path in event.paths {
                        let _ = changes.send(path);
                    }
                }
                Err(e) => eprintln!("folderflow: folder watching: {e}"),
            })?;
        Ok(Self {
            watching: Mutex::new((watcher, Vec::new())),
        })
    }
}

impl Watcher for AppWatcher {
    fn watch(&self, folders: &[(PathBuf, bool)]) {
        let mut guard = self.watching.lock().unwrap_or_else(|e| e.into_inner());
        let (watcher, watching) = &mut *guard;
        for old in watching.iter().filter(|w| !folders.contains(w)) {
            let _ = watcher.unwatch(&old.0);
        }
        for new in folders.iter().filter(|f| !watching.contains(f)) {
            let mode = if new.1 {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            if let Err(e) = watcher.watch(&new.0, mode) {
                eprintln!("folderflow: couldn't watch {}: {e}", new.0.display());
            }
        }
        *watching = folders.to_vec();
    }
}

/// How long to gather changes before looking: a download or a copy of many
/// files comes as a burst.
const GATHER: Duration = Duration::from_secs(1);

/// Hands folder changes to the engine: a second after the first of a burst,
/// all together.
pub async fn forward_changes(engine: Engine, mut changes: mpsc::UnboundedReceiver<PathBuf>) {
    while let Some(first) = changes.recv().await {
        let mut paths = vec![first];
        let until = tokio::time::Instant::now() + GATHER;
        while let Ok(Some(path)) = tokio::time::timeout_at(until, changes.recv()).await {
            paths.push(path);
        }
        paths.sort();
        paths.dedup();
        let engine = engine.clone();
        let _ = tokio::task::spawn_blocking(move || engine.folders_changed(&paths)).await;
    }
}
