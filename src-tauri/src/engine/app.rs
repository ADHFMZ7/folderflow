//! The engine's ports in the running app: the Mac's notifications, and Tauri
//! events to the window.
//!
//! Notifications use NSUserNotificationCenter, which Apple deprecates in favour
//! of UserNotifications. That framework works only inside an app bundle, and
//! `tauri dev` runs a bare binary, so it would leave development builds silent.
//! Revisit when the menu bar app lands (docs/engine.md, pull request 9).
#![allow(deprecated)]

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread};
use objc2_foundation::{
    NSString, NSUserNotification, NSUserNotificationCenter, NSUserNotificationCenterDelegate,
};
use tauri::{AppHandle, Emitter};

use super::{EngineEvents, Notifier, RunChanged};

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
}
