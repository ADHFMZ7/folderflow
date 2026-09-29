pub mod api;
mod background;
pub mod engine;
pub mod storage;
pub mod updates;
pub mod workflow;

use std::sync::Arc;

use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use api::commands::{self, AppBackend};
use api::providers::HttpProviders;
use api::Backend;
use engine::app::{forward_changes, AppEvents, AppNotifier, AppTrash, AppWatcher};
use engine::{Engine, Ports, SystemClock};
use storage::data_dir::DataDir;
use storage::secrets::KeychainStore;
use updates::Updates;

/// The bundle id from before the app was named Vela.
const OLD_ID: &str = "com.adhfmz7.folderflow"; // rename:keep

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![background::AT_LOGIN]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            // The old bundle id's data folder moves over once.
            let old = data.with_file_name(OLD_ID);
            let dir = DataDir::open_moving(data, &old)?;
            // Keys go in the Keychain under the bundle id, com.adhfmz7.vela.
            let secrets = Arc::new(KeychainStore::new(app.config().identifier.clone()));
            let (changes, changed) = tokio::sync::mpsc::unbounded_channel();
            let engine = Engine::new(
                &dir,
                app.path().home_dir()?,
                Ports {
                    clock: Arc::new(SystemClock),
                    notifier: Arc::new(AppNotifier::new(
                        &app.config().identifier,
                        background::in_app_bundle(),
                        {
                            let handle = app.handle().clone();
                            move || {
                                let shown = handle.clone();
                                let _ = handle
                                    .run_on_main_thread(move || background::show_window(&shown));
                            }
                        },
                    )),
                    events: Arc::new(AppEvents(app.handle().clone())),
                    trash: Arc::new(AppTrash),
                    watcher: Arc::new(AppWatcher::new(changes)?),
                },
            )?;
            let started = engine.clone();
            tauri::async_runtime::spawn(async move {
                started.start();
                forward_changes(started, changed).await;
            });
            app.manage(engine);
            app.manage(Backend::new(dir, secrets, HttpProviders::default()));
            // Only Vela.app can replace itself.
            app.manage(if background::in_app_bundle() {
                let handle = app.handle().clone();
                Updates::new(updates::Plugin(handle.clone()), move |status| {
                    let _ = handle.emit("update-changed", status);
                    background::show_update(&handle);
                })
            } else {
                Updates::unavailable()
            });
            updates::schedule(app.handle().clone());

            background::install(app)?;
            match app.state::<AppBackend>().get_settings() {
                Ok(loaded) => {
                    background::open_at_login(app.handle(), loaded.settings.open_at_login)
                }
                Err(e) => eprintln!("vela: {}", e.message),
            }
            // Opened at login, Vela starts in the menu bar alone.
            if background::opened_at_login() {
                app.set_dock_visibility(false);
            } else {
                background::show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing hides: workflows keep running (decision 3).
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                background::hide_window(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::list_model_kinds,
            commands::list_providers,
            commands::detect,
            commands::connect,
            commands::remove_connection,
            commands::list_models,
            commands::list_workflows,
            commands::get_workflow,
            commands::create_workflow,
            commands::save_workflow,
            commands::delete_workflow,
            commands::validate_workflow,
            commands::get_draft,
            commands::save_draft,
            commands::apply_draft,
            commands::discard_draft,
            commands::list_templates,
            commands::choose_folder,
            commands::choose_csv,
            commands::choose_files,
            commands::run_now,
            commands::list_runs,
            commands::get_run,
            commands::list_needs_you,
            commands::answer,
            commands::retry_run,
            commands::resume_run,
            commands::undo_run,
            commands::dismiss_run,
            commands::list_notices,
            commands::mark_notices_read,
            commands::clear_notices,
            commands::get_activity,
            commands::pause_all,
            commands::try_on_file,
            commands::get_update_status,
            commands::check_for_updates,
            commands::restart_to_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // Opened again from Finder or Spotlight while in the menu bar.
            RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => background::show_window(app),
            // However Vela quits (the menu bar, ⌘Q, the Dock, logging
            // out), it ends here, and a downloaded update goes in place.
            RunEvent::Exit => {
                app.state::<Engine>().stop(background::QUIT_WAIT);
                app.state::<Updates>().install_as_quitting();
            }
            _ => {}
        });
}
