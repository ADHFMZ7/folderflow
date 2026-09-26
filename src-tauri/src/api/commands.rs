//! The Tauri commands: each one hands its arguments to `Backend` and nothing more.
//! They are async so none of them runs on, and blocks, the main thread.

use tauri::State;

use super::providers::HttpProviders;
use super::types::{
    ApiError, ConnectOutcome, Credentials, DetectResult, LoadedSettings, Model, ModelKind,
    Provider, SettingsChange, Template, WorkflowSummary,
};
use super::Backend;
use crate::storage::settings::Settings;
use crate::workflow::{Problem, SaveResult, Workflow};

pub type AppBackend = Backend<HttpProviders>;

#[tauri::command]
pub async fn get_settings(backend: State<'_, AppBackend>) -> Result<LoadedSettings, ApiError> {
    backend.get_settings()
}

#[tauri::command]
pub async fn update_settings(
    backend: State<'_, AppBackend>,
    change: SettingsChange,
) -> Result<Settings, ApiError> {
    backend.update_settings(change)
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
) -> Result<Vec<WorkflowSummary>, ApiError> {
    backend.list_workflows()
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
    workflow: Workflow,
) -> Result<SaveResult, ApiError> {
    backend.save_workflow(workflow)
}

#[tauri::command]
pub async fn delete_workflow(backend: State<'_, AppBackend>, id: String) -> Result<(), ApiError> {
    backend.delete_workflow(&id)
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
