pub mod api;
pub mod storage;
pub mod workflow;

use std::sync::Arc;

use tauri::Manager;

use api::commands;
use api::providers::HttpProviders;
use api::Backend;
use storage::data_dir::DataDir;
use storage::secrets::KeychainStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = DataDir::open(app.path().app_data_dir()?)?;
            // Keys go in the Keychain under the bundle id, com.adhfmz7.folderflow.
            let secrets = Arc::new(KeychainStore::new(app.config().identifier.clone()));
            app.manage(Backend::new(dir, secrets, HttpProviders::default()));
            Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
