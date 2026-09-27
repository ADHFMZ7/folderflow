//! FolderFlow in the menu bar (decision 3). Closing the window hides it and
//! the Dock icon while workflows keep running; the menu bar icon shows what
//! the engine is doing, and opens the window, pauses or quits. Opened at
//! login, FolderFlow starts there with no window. See docs/engine.md,
//! "Background".

use std::path::Path;
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Window};
use tauri_plugin_autostart::ManagerExt as _;

use crate::engine::{Activity, Engine};

const TRAY: &str = "folderflow";

/// Passed to FolderFlow when macOS opens it at login.
pub const AT_LOGIN: &str = "--at-login";

/// How long quitting waits for runs in progress to finish the step they are on.
pub const QUIT_WAIT: Duration = Duration::from_secs(2);

/// One line of the menu bar icon's menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Item {
        id: &'static str,
        label: String,
        enabled: bool,
    },
    Separator,
}

fn item(id: &'static str, label: impl Into<String>) -> Entry {
    Entry::Item {
        id,
        label: label.into(),
        enabled: true,
    }
}

/// The menu for this activity: what the engine is doing, how many runs are
/// in Needs you, then Open, Pause all or Resume, and Quit.
pub fn entries(activity: Activity) -> Vec<Entry> {
    let mut out = vec![Entry::Item {
        id: "status",
        label: status(activity),
        enabled: false,
    }];
    match activity.needs_you {
        0 => {}
        1 => out.push(item("needs-you", "1 needs you")),
        n => out.push(item("needs-you", format!("{n} need you"))),
    }
    out.push(Entry::Separator);
    out.push(item("open", "Open FolderFlow"));
    out.push(item(
        "pause",
        if activity.paused {
            "Resume"
        } else {
            "Pause all"
        },
    ));
    out.push(Entry::Separator);
    out.push(item("quit", "Quit FolderFlow"));
    out
}

/// What the engine is doing, in a few words.
pub fn status(activity: Activity) -> String {
    match (activity.paused, activity.running) {
        (true, 0) => "Paused".into(),
        // Pausing lets runs already going finish.
        (true, n) => format!("Paused · {n} finishing"),
        (false, 0) => "Nothing running".into(),
        (false, n) => format!("{n} running"),
    }
}

/// Which drawing the menu bar icon shows: the boat, with a wake while runs
/// are going or its sails lowered while paused, and a dot when something
/// needs you. Drawn in `icons/tray/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Watching,
    Running,
    Paused,
}

pub fn look(activity: Activity) -> (Look, bool) {
    let look = if activity.paused {
        Look::Paused
    } else if activity.running > 0 {
        Look::Running
    } else {
        Look::Watching
    };
    (look, activity.needs_you > 0)
}

fn icon(activity: Activity) -> Image<'static> {
    match look(activity) {
        (Look::Watching, false) => tauri::include_image!("icons/tray/watching.png"),
        (Look::Watching, true) => tauri::include_image!("icons/tray/watching-needs-you.png"),
        (Look::Running, false) => tauri::include_image!("icons/tray/running.png"),
        (Look::Running, true) => tauri::include_image!("icons/tray/running-needs-you.png"),
        (Look::Paused, false) => tauri::include_image!("icons/tray/paused.png"),
        (Look::Paused, true) => tauri::include_image!("icons/tray/paused-needs-you.png"),
    }
}

fn menu(app: &AppHandle, activity: Activity) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    for entry in entries(activity) {
        match entry {
            Entry::Item { id, label, enabled } => {
                let accelerator = (id == "quit").then_some("CmdOrCtrl+Q");
                menu.append(&MenuItem::with_id(app, id, label, enabled, accelerator)?)?;
            }
            Entry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(menu)
}

/// Puts the icon in the menu bar. The engine must be managed already.
pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let activity = app.state::<Engine>().activity();
    TrayIconBuilder::with_id(TRAY)
        .icon(icon(activity))
        .icon_as_template(true)
        .tooltip(format!("FolderFlow: {}", status(activity)))
        .menu(&menu(app.handle(), activity)?)
        .on_menu_event(on_menu)
        .build(app)?;
    Ok(())
}

/// Brings the menu bar icon up to date. Menus are changed on the main
/// thread; updates run there in the order they were told.
pub fn show_activity(app: &AppHandle, activity: Activity) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(tray) = handle.tray_by_id(TRAY) else {
            return;
        };
        if let Ok(menu) = menu(&handle, activity) {
            let _ = tray.set_menu(Some(menu));
        }
        let _ = tray.set_icon(Some(icon(activity)));
        let _ = tray.set_icon_as_template(true);
        let _ = tray.set_tooltip(Some(format!("FolderFlow: {}", status(activity))));
    });
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open" => show_window(app),
        "needs-you" => {
            show_window(app);
            // Needs you is at the top of the Workflows page.
            let _ = app.emit("navigate", "#/workflows");
        }
        "pause" => {
            let engine = app.state::<Engine>().inner().clone();
            // Resuming looks in every watched folder: not on the main thread.
            tauri::async_runtime::spawn_blocking(move || {
                let paused = engine.activity().paused;
                if let Err(e) = engine.pause_all(!paused) {
                    eprintln!("folderflow: {}", e.message);
                }
            });
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Shows the window, with FolderFlow back in the Dock.
pub fn show_window(app: &AppHandle) {
    let _ = app.set_dock_visibility(true);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Hides the window and the Dock icon; FolderFlow stays in the menu bar.
pub fn hide_window(window: &Window) {
    let _ = window.hide();
    let _ = window.app_handle().set_dock_visibility(false);
}

/// Whether macOS opened FolderFlow at login, rather than the person.
pub fn opened_at_login() -> bool {
    std::env::args().any(|a| a == AT_LOGIN)
}

/// Adds or removes FolderFlow's login item to match the setting. Only
/// FolderFlow.app does: a development build would add its bare binary.
pub fn open_at_login(app: &AppHandle, on: bool) {
    match std::env::current_exe() {
        Ok(exe) if in_app_bundle(&exe) => {}
        _ => return,
    }
    let launcher = app.autolaunch();
    if launcher.is_enabled().is_ok_and(|now| now == on) {
        return;
    }
    let result = if on {
        launcher.enable()
    } else {
        launcher.disable()
    };
    if let Err(e) = result {
        eprintln!("folderflow: couldn't change the login item: {e}");
    }
}

fn in_app_bundle(exe: &Path) -> bool {
    exe.to_string_lossy().contains(".app/Contents/MacOS/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(paused: bool, running: u32, needs_you: u32) -> Activity {
        Activity {
            paused,
            running,
            needs_you,
        }
    }

    /// The menu as it reads, `-` for a separator and `(…)` for a line that
    /// can't be clicked.
    fn read(activity: Activity) -> Vec<String> {
        entries(activity)
            .into_iter()
            .map(|e| match e {
                Entry::Item {
                    label,
                    enabled: false,
                    ..
                } => format!("({label})"),
                Entry::Item { label, .. } => label,
                Entry::Separator => "-".into(),
            })
            .collect()
    }

    #[test]
    fn the_menu_says_what_is_running_and_offers_pause_all() {
        assert_eq!(
            read(activity(false, 0, 0)),
            [
                "(Nothing running)",
                "-",
                "Open FolderFlow",
                "Pause all",
                "-",
                "Quit FolderFlow"
            ]
        );
        assert_eq!(read(activity(false, 2, 0))[0], "(2 running)");
    }

    #[test]
    fn runs_waiting_on_the_person_get_a_line_that_opens_needs_you() {
        let menu = entries(activity(false, 0, 1));
        assert_eq!(menu[1], item("needs-you", "1 needs you"));
        assert_eq!(read(activity(false, 0, 3))[1], "3 need you");
    }

    #[test]
    fn paused_it_offers_resume_and_says_what_is_finishing() {
        let menu = read(activity(true, 0, 0));
        assert_eq!(menu[0], "(Paused)");
        assert!(menu.contains(&"Resume".to_string()));
        assert!(!menu.contains(&"Pause all".to_string()));
        assert_eq!(read(activity(true, 1, 0))[0], "(Paused · 1 finishing)");
    }

    #[test]
    fn the_icon_shows_the_pause_first_then_runs_and_a_dot_for_needs_you() {
        assert_eq!(look(activity(false, 0, 0)), (Look::Watching, false));
        assert_eq!(look(activity(false, 2, 0)), (Look::Running, false));
        assert_eq!(look(activity(false, 0, 1)), (Look::Watching, true));
        // Paused with runs still finishing: the sails are down all the same.
        assert_eq!(look(activity(true, 1, 3)), (Look::Paused, true));
    }

    #[test]
    fn only_an_app_bundle_changes_the_login_item() {
        assert!(in_app_bundle(Path::new(
            "/Applications/FolderFlow.app/Contents/MacOS/folderflow"
        )));
        assert!(!in_app_bundle(Path::new(
            "/Users/me/folderflow/src-tauri/target/debug/folderflow"
        )));
    }
}
