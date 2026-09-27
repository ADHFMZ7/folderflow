//! The engine's ports in the running app: the Mac's notifications, the Trash,
//! folder watching with FSEvents, and Tauri events to the window and the
//! menu bar icon.
//!
//! Notifications use UserNotifications in Vela.app. That framework works only
//! inside an app bundle, and `tauri dev` runs a bare binary, so development
//! builds keep the deprecated NSUserNotificationCenter.
#![allow(deprecated)]

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_foundation::{
    NSError, NSFileManager, NSString, NSUserNotification, NSUserNotificationCenter,
    NSUserNotificationCenterDelegate, NSURL,
};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

/// Notifications from Vela. They show as banners even while Vela is the
/// front app, which macOS otherwise skips: a person who just chose Run is
/// usually looking at Vela. Clicking one calls `on_open`.
pub struct AppNotifier {
    route: Route,
}

enum Route {
    /// Vela.app: UserNotifications. `denied` is whether the person turned
    /// Vela's notifications off, as last heard.
    Bundle { denied: Arc<AtomicBool> },
    /// A development build isn't an app bundle, which UserNotifications
    /// needs: the older NSUserNotificationCenter, borrowing the bundle id.
    /// `unavailable` is why it can't show them, if it can't.
    Dev { unavailable: Option<String> },
}

impl AppNotifier {
    /// For Vela.app (`in_bundle`), asks macOS for permission to notify the
    /// first time; for a development build, borrows `identifier`.
    pub fn new(
        identifier: &str,
        in_bundle: bool,
        on_open: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let route = if in_bundle {
            Self::bundle(on_open)
        } else {
            Self::dev(identifier)
        };
        Self { route }
    }

    fn bundle(on_open: impl Fn() + Send + Sync + 'static) -> Route {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let delegate = Delegate::new(Box::new(on_open));
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The center holds its delegate weakly: keep it for the life of the app.
        let _ = Retained::into_raw(delegate);
        let denied = Arc::new(AtomicBool::new(false));
        let heard = denied.clone();
        let answered = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            heard.store(!granted.as_bool(), Ordering::SeqCst);
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &answered,
        );
        Route::Bundle { denied }
    }

    fn dev(identifier: &str) -> Route {
        let unavailable = mac_notification_sys::set_application(identifier)
            .err()
            .map(|_| {
                "macOS doesn't know Vela yet. Open Vela.app once, then try again.".to_string()
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
        Route::Dev { unavailable }
    }
}

impl Notifier for AppNotifier {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        match &self.route {
            Route::Bundle { denied } => {
                let center = UNUserNotificationCenter::currentNotificationCenter();
                // The person can turn notifications off at any time: look again
                // for next time.
                let heard = denied.clone();
                let settings = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
                    // SAFETY: the center passes settings that live for the call.
                    let status = unsafe { settings.as_ref() }.authorizationStatus();
                    heard.store(status == UNAuthorizationStatus::Denied, Ordering::SeqCst);
                });
                center.getNotificationSettingsWithCompletionHandler(&settings);
                if denied.load(Ordering::SeqCst) {
                    return Err(
                        "Notifications are off for Vela. Turn them on in System Settings › Notifications › Vela."
                            .into(),
                    );
                }
                let content = UNMutableNotificationContent::new();
                content.setTitle(&NSString::from_str(title));
                content.setBody(&NSString::from_str(body));
                let id = NSString::from_str(&uuid::Uuid::new_v4().to_string());
                let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                    &id, &content, None,
                );
                center.addNotificationRequest_withCompletionHandler(&request, None);
                Ok(())
            }
            Route::Dev { unavailable } => {
                if let Some(why) = unavailable {
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
    }
}

/// Called when the person clicks one of Vela's notifications.
struct OnOpen(Box<dyn Fn() + Send + Sync>);

define_class!(
    /// Shows Vela's notifications while it's the front app, and opens Vela
    /// when one is clicked (UserNotifications).
    #[unsafe(super(NSObject))]
    #[name = "VelaNotificationDelegate"]
    #[ivars = OnOpen]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion
                .call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            _response: &UNNotificationResponse,
            completion: &DynBlock<dyn Fn()>,
        ) {
            (self.ivars().0)();
            completion.call(());
        }
    }
);

impl Delegate {
    fn new(on_open: Box<dyn Fn() + Send + Sync>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(OnOpen(on_open));
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    /// Tells macOS to show every Vela notification, front app or not
    /// (NSUserNotificationCenter, for development builds).
    #[unsafe(super(NSObject))]
    #[name = "VelaNotificationPresenter"]
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
                Err(e) => eprintln!("vela: folder watching: {e}"),
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
                eprintln!("vela: couldn't watch {}: {e}", new.0.display());
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
