pub mod api;
mod background;
pub mod engine;
pub mod storage;
pub mod workflow;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use api::commands::{self, AppBackend};
use api::providers::HttpProviders;
use api::Backend;
use engine::app::{forward_changes, AppEvents, AppNotifier, AppTrash, AppWatcher};
use engine::{Engine, Ports, SystemClock};
use storage::data_dir::DataDir;
use storage::secrets::KeychainStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![background::AT_LOGIN]),
        ))
        .setup(|app| {
            let dir = DataDir::open(app.path().app_data_dir()?)?;
            // Keys go in the Keychain under the bundle id, com.adhfmz7.folderflow.
            let secrets = Arc::new(KeychainStore::new(app.config().identifier.clone()));
            let (changes, changed) = tokio::sync::mpsc::unbounded_channel();
            let engine = Engine::new(
                &dir,
                app.path().home_dir()?,
                Ports {
                    clock: Arc::new(SystemClock),
                    notifier: Arc::new(AppNotifier::new(&app.config().identifier)),
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

            background::install(app)?;
            match app.state::<AppBackend>().get_settings() {
                Ok(loaded) => {
                    background::open_at_login(app.handle(), loaded.settings.open_at_login)
                }
                Err(e) => eprintln!("folderflow: {}", e.message),
            }
            // Opened at login, FolderFlow starts in the menu bar alone.
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
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // Opened again from Finder or Spotlight while in the menu bar.
            RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => background::show_window(app),
            RunEvent::Exit => app.state::<Engine>().stop(background::QUIT_WAIT),
            _ => {}
        });
}
