//! The Tauri commands: each one hands its arguments to `Backend` and nothing more.
//! They are async so none of them runs on, and blocks, the main thread.

use std::path::{Path, PathBuf};

use tauri::{Manager, State};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

use super::pickers::{expand_home, shorten_home};

use super::providers::HttpProviders;
use super::types::{
    ApiError, ConnectOutcome, Credentials, DetectResult, ErrorCode, LoadedSettings, Model,
    ModelKind, Provider, SettingsChange, Template, WorkflowSummary,
};
use super::Backend;
use crate::engine::notices::Notice;
use crate::engine::runs::{NeedsYouItem, Run, RunQuery, RunSummary, UndoResult};
use crate::engine::{Activity, Engine};
use crate::storage::settings::Settings;
use crate::workflow::{Problem, SaveResult, Workflow};

pub type AppBackend = Backend<HttpProviders>;

#[tauri::command]
pub async fn get_settings(backend: State<'_, AppBackend>) -> Result<LoadedSettings, ApiError> {
    backend.get_settings()
}

#[tauri::command]
pub async fn update_settings(
    app: tauri::AppHandle,
    backend: State<'_, AppBackend>,
    change: SettingsChange,
) -> Result<Settings, ApiError> {
    let login = change.open_at_login.is_some();
    let settings = backend.update_settings(change)?;
    if login {
        crate::background::open_at_login(&app, settings.open_at_login);
    }
    Ok(settings)
}

#[tauri::command]
pub async fn list_model_kinds(backend: State<'_, AppBackend>) -> Result<Vec<ModelKind>, ApiError> {
    Ok(backend.list_model_kinds())
}

#[tauri::command]
pub async fn list_providers(backend: State<'_, AppBackend>) -> Result<Vec<Provider>, ApiError> {
    Ok(backend.list_providers())
}

#[tauri::command]
pub async fn detect(
    backend: State<'_, AppBackend>,
    provider_id: String,
) -> Result<DetectResult, ApiError> {
    backend.detect(&provider_id).await
}

#[tauri::command]
pub async fn connect(
    backend: State<'_, AppBackend>,
    provider_id: String,
    credentials: Credentials,
) -> Result<ConnectOutcome, ApiError> {
    backend.connect(&provider_id, credentials).await
}

#[tauri::command]
pub async fn remove_connection(
    backend: State<'_, AppBackend>,
    id: String,
) -> Result<Settings, ApiError> {
    backend.remove_connection(&id)
}

#[tauri::command]
pub async fn list_models(
    backend: State<'_, AppBackend>,
    connection_id: String,
) -> Result<Vec<Model>, ApiError> {
    backend.list_models(&connection_id).await
}

#[tauri::command]
pub async fn list_workflows(
    backend: State<'_, AppBackend>,
    engine: State<'_, Engine>,
) -> Result<Vec<WorkflowSummary>, ApiError> {
    let mut list = backend.list_workflows()?;
    let runs = engine.workflow_runs();
    for w in &mut list {
        if let Some((needs_you, last)) = runs.get(&w.id) {
            w.needs_you = *needs_you;
            w.last_run = Some(last.clone());
        }
    }
    Ok(list)
}

#[tauri::command]
pub async fn get_workflow(
    backend: State<'_, AppBackend>,
    id: String,
) -> Result<Workflow, ApiError> {
    backend.get_workflow(&id)
}

#[tauri::command]
pub async fn create_workflow(
    backend: State<'_, AppBackend>,
    template_id: Option<String>,
) -> Result<Workflow, ApiError> {
    backend.create_workflow(template_id)
}

#[tauri::command]
pub async fn save_workflow(
    backend: State<'_, AppBackend>,
    engine: State<'_, Engine>,
    workflow: Workflow,
) -> Result<SaveResult, ApiError> {
    let saved = backend.save_workflow(workflow)?;
    engine.reload(&saved.workflow.id);
    Ok(saved)
}

#[tauri::command]
pub async fn delete_workflow(
    backend: State<'_, AppBackend>,
    engine: State<'_, Engine>,
    id: String,
) -> Result<(), ApiError> {
    backend.delete_workflow(&id)?;
    engine.reload(&id);
    Ok(())
}

#[tauri::command]
pub async fn get_draft(
    backend: State<'_, AppBackend>,
    id: String,
) -> Result<Option<Workflow>, ApiError> {
    backend.get_draft(&id)
}

#[tauri::command]
pub async fn save_draft(
    backend: State<'_, AppBackend>,
    workflow: Workflow,
) -> Result<SaveResult, ApiError> {
    backend.save_draft(workflow)
}

#[tauri::command]
pub async fn apply_draft(
    backend: State<'_, AppBackend>,
    engine: State<'_, Engine>,
    id: String,
) -> Result<SaveResult, ApiError> {
    let applied = backend.apply_draft(&id)?;
    engine.reload(&id);
    Ok(applied)
}

#[tauri::command]
pub async fn discard_draft(backend: State<'_, AppBackend>, id: String) -> Result<(), ApiError> {
    backend.discard_draft(&id)
}

#[tauri::command]
pub async fn validate_workflow(
    backend: State<'_, AppBackend>,
    workflow: Workflow,
) -> Result<Vec<Problem>, ApiError> {
    backend.validate_workflow(workflow)
}

#[tauri::command]
pub async fn list_templates(backend: State<'_, AppBackend>) -> Result<Vec<Template>, ApiError> {
    Ok(backend.list_templates())
}

/// Runs the saved workflow once on each file. See docs/engine.md, "Run now".
#[tauri::command]
pub async fn run_now(
    engine: State<'_, Engine>,
    workflow_id: String,
    files: Vec<String>,
) -> Result<Vec<RunSummary>, ApiError> {
    engine.run_now(&workflow_id, files)
}

#[tauri::command]
pub async fn list_runs(
    engine: State<'_, Engine>,
    query: RunQuery,
) -> Result<Vec<RunSummary>, ApiError> {
    engine.list_runs(query)
}

#[tauri::command]
pub async fn get_run(engine: State<'_, Engine>, id: String) -> Result<Run, ApiError> {
    engine.get_run(&id)
}

#[tauri::command]
pub async fn list_needs_you(engine: State<'_, Engine>) -> Result<Vec<NeedsYouItem>, ApiError> {
    engine.list_needs_you()
}

#[tauri::command]
pub async fn answer(
    engine: State<'_, Engine>,
    run_id: String,
    branch_id: String,
) -> Result<Run, ApiError> {
    engine.answer(&run_id, &branch_id)
}

#[tauri::command]
pub async fn retry_run(engine: State<'_, Engine>, run_id: String) -> Result<Run, ApiError> {
    engine.retry_run(&run_id)
}

#[tauri::command]
pub async fn resume_run(engine: State<'_, Engine>, run_id: String) -> Result<Run, ApiError> {
    engine.resume_run(&run_id)
}

#[tauri::command]
pub async fn undo_run(engine: State<'_, Engine>, run_id: String) -> Result<UndoResult, ApiError> {
    engine.undo_run(&run_id)
}

#[tauri::command]
pub async fn list_notices(engine: State<'_, Engine>) -> Result<Vec<Notice>, ApiError> {
    Ok(engine.list_notices())
}

#[tauri::command]
pub async fn mark_notices_read(
    engine: State<'_, Engine>,
    ids: Option<Vec<String>>,
) -> Result<(), ApiError> {
    engine.mark_notices_read(ids);
    Ok(())
}

#[tauri::command]
pub async fn clear_notices(engine: State<'_, Engine>) -> Result<(), ApiError> {
    engine.clear_notices();
    Ok(())
}

#[tauri::command]
pub async fn get_activity(engine: State<'_, Engine>) -> Result<Activity, ApiError> {
    Ok(engine.activity())
}

#[tauri::command]
pub async fn pause_all(engine: State<'_, Engine>, paused: bool) -> Result<Activity, ApiError> {
    engine.pause_all(paused)
}

#[tauri::command]
pub async fn dismiss_run(engine: State<'_, Engine>, run_id: String) -> Result<(), ApiError> {
    engine.dismiss_run(&run_id)
}

/// The native folder picker. Opened from here rather than from the webview, so
/// the window needs no dialog permission and paths come back with `~`.
#[tauri::command]
pub async fn choose_folder(
    window: tauri::WebviewWindow,
    start: Option<String>,
) -> Result<Option<String>, ApiError> {
    pick(&window, start, |dialog| {
        dialog.set_title("Choose a folder").blocking_pick_folder()
    })
}

/// The native file picker, for an existing .csv file. A new file's path is typed.
#[tauri::command]
pub async fn choose_csv(
    window: tauri::WebviewWindow,
    start: Option<String>,
) -> Result<Option<String>, ApiError> {
    pick(&window, start, |dialog| {
        dialog
            .set_title("Choose a spreadsheet")
            .add_filter("CSV spreadsheet", &["csv"])
            .blocking_pick_file()
    })
}

/// The native file picker, for files to run a workflow on. Answers with none
/// when cancelled.
#[tauri::command]
pub async fn choose_files(
    window: tauri::WebviewWindow,
    start: Option<String>,
) -> Result<Vec<String>, ApiError> {
    let (dialog, home) = dialog_at(&window, start)?;
    Ok(dialog
        .set_title("Choose files")
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| path.into_path().ok())
        .map(|path| shorten_home(&path, &home))
        .collect())
}

/// Opens a picker in front of `window`, and answers with the path or `None`.
fn pick(
    window: &tauri::WebviewWindow,
    start: Option<String>,
    open: impl FnOnce(FileDialogBuilder<tauri::Wry>) -> Option<FilePath>,
) -> Result<Option<String>, ApiError> {
    let (dialog, home) = dialog_at(window, start)?;
    Ok(open(dialog)
        .and_then(|path| path.into_path().ok())
        .map(|path| shorten_home(&path, &home)))
}

/// A picker in front of `window`, starting where `start` points if it's a
/// real folder (or a file in one), and the home folder to shorten paths with.
fn dialog_at(
    window: &tauri::WebviewWindow,
    start: Option<String>,
) -> Result<(FileDialogBuilder<tauri::Wry>, PathBuf), ApiError> {
    let home = window.path().home_dir().map_err(|e| {
        ApiError::new(
            ErrorCode::Io,
            format!("Couldn't find your home folder: {e}"),
        )
    })?;
    let mut dialog = window.dialog().file().set_parent(window);
    // A folder, or the folder a file is in; a path that doesn't exist yet opens the default.
    let start_dir = start
        .as_deref()
        .and_then(|s| expand_home(s, &home))
        .and_then(|p| {
            if p.is_dir() {
                Some(p)
            } else {
                p.parent().map(Path::to_path_buf)
            }
        })
        .filter(|dir| dir.is_dir());
    if let Some(dir) = start_dir {
        dialog = dialog.set_directory(dir);
    }
    Ok((dialog, home))
}
