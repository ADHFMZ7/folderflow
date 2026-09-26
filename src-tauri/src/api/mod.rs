//! What the front end can ask of the core. `Backend` does the work; the Tauri
//! commands in `commands` are thin wrappers, so tests call `Backend` directly
//! with an in-memory key store, a temp data folder and fake providers.

pub mod catalog;
pub mod commands;
pub mod pickers;
pub mod providers;
mod summary;
pub mod types;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::storage::connections::Connections;
use crate::storage::data_dir::DataDir;
use crate::storage::secrets::{Secret, SecretStore};
use crate::storage::settings::{LoadOutcome, ModelRef, Settings, SettingsStore};
use crate::storage::workflows::{WorkflowError, WorkflowStore};
use crate::workflow::{templates, validate, Problem, SaveResult, Workflow};

use providers::{CheckError, ModelSource, ProviderClient, RemoteModel};
use types::{
    ApiError, ConnectMethod, ConnectOutcome, Credentials, DetectResult, ErrorCode, LoadedSettings,
    Model, ModelKind, Provider, SettingsChange, SettingsNotice, Template, WorkflowSummary,
};

const KEY_REJECTED: &str = "That key was rejected. Check it and try again.";
const ENTER_KEY: &str = "Enter an API key.";
const ENTER_ADDRESS: &str = "Enter an address starting with http:// or https://.";

pub struct Backend<P> {
    settings: SettingsStore,
    secrets: Arc<dyn SecretStore>,
    providers: P,
    /// Serialises its own writes, so workflow saves can't interleave.
    workflows: WorkflowStore,
    /// Every settings read-modify-write happens while holding this, so commands
    /// running at the same time can't overwrite each other's changes.
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Kept for the life of the app once a damaged settings file is set aside,
    /// so every `get_settings` reports it, not only the first.
    notice: Option<SettingsNotice>,
}

impl<P: ProviderClient> Backend<P> {
    pub fn new(dir: DataDir, secrets: Arc<dyn SecretStore>, providers: P) -> Self {
        Self {
            settings: SettingsStore::new(&dir),
            workflows: WorkflowStore::new(&dir),
            secrets,
            providers,
            state: Mutex::new(State::default()),
        }
    }

    pub fn get_settings(&self) -> Result<LoadedSettings, ApiError> {
        let mut state = self.lock();
        let settings = self.load(&mut state)?;
        Ok(LoadedSettings {
            settings,
            notice: state.notice.clone(),
        })
    }

    pub fn update_settings(&self, change: SettingsChange) -> Result<Settings, ApiError> {
        let mut state = self.lock();
        let mut settings = self.load(&mut state)?;

        if let Some(v) = change.setup_complete {
            settings.setup_complete = v;
        }
        if let Some(v) = change.open_at_login {
            settings.open_at_login = v;
        }
        if let Some(v) = change.appearance {
            settings.appearance = v;
        }
        if let Some(defaults) = change.defaults {
            let unknown = defaults
                .values()
                .flatten()
                .any(|m| !settings.connections.iter().any(|c| c.id == m.connection_id));
            if unknown {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "That model's connection no longer exists.",
                ));
            }
            settings.defaults = defaults;
        }

        self.settings.save(&settings)?;
        Ok(settings)
    }

    pub fn list_model_kinds(&self) -> Vec<ModelKind> {
        catalog::model_kinds()
    }

    pub fn list_providers(&self) -> Vec<Provider> {
        catalog::providers()
    }

    /// For providers connected by "detect": is it running on this Mac?
    pub async fn detect(&self, provider_id: &str) -> Result<DetectResult, ApiError> {
        let provider = find_provider(provider_id)?;
        let source = match (provider.connect, provider.id.as_str()) {
            (ConnectMethod::Detect, "ollama") => ModelSource::Ollama,
            _ => {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    format!("{} can't be found automatically.", provider.name),
                ))
            }
        };
        Ok(match self.providers.list_models(source).await {
            Ok(_) => DetectResult::found(),
            Err(_) => {
                DetectResult::not_found(format!("{} isn't running on this Mac.", provider.name))
            }
        })
    }

    /// Checks the credentials with the provider, then saves the connection and
    /// its key. A refused or failed check stores nothing.
    pub async fn connect(
        &self,
        provider_id: &str,
        credentials: Credentials,
    ) -> Result<ConnectOutcome, ApiError> {
        let provider = find_provider(provider_id)?;
        if provider.id == "jev" {
            return Ok(ConnectOutcome::failed("Jev can't be connected yet."));
        }

        let key = credentials
            .api_key
            .map(|k| Secret::new(k.expose().trim()))
            .filter(|k| !k.expose().is_empty());
        let (key, endpoint) = match provider.connect {
            ConnectMethod::Detect => (None, None),
            ConnectMethod::ApiKey if key.is_none() => {
                return Ok(ConnectOutcome::failed(ENTER_KEY));
            }
            ConnectMethod::ApiKey => (key, None),
            ConnectMethod::Endpoint => {
                match credentials.endpoint.as_deref().and_then(web_address) {
                    Some(endpoint) => (key, Some(endpoint)),
                    None => return Ok(ConnectOutcome::failed(ENTER_ADDRESS)),
                }
            }
        };

        let Some(source) = source_for(&provider.id, endpoint.as_deref(), key.as_ref()) else {
            return Err(unsupported(&provider));
        };
        let models = match self.providers.list_models(source).await {
            Ok(models) => models,
            Err(CheckError::Rejected) => return Ok(ConnectOutcome::failed(KEY_REJECTED)),
            Err(CheckError::Unreachable) => {
                return Ok(ConnectOutcome::failed(unreachable(&provider)));
            }
            Err(CheckError::Unexpected) => return Err(unexpected_answer(&provider)),
        };

        let mut state = self.lock();
        self.load(&mut state)?;
        let connection = Connections::new(&self.settings, self.secrets.as_ref())
            .add_with_endpoint(&provider.id, endpoint, key)?;
        let mut settings = self.load(&mut state)?;
        fill_missing_defaults(&mut settings, &provider, &connection.id, &models);
        self.settings.save(&settings)?;
        Ok(ConnectOutcome::ok(connection, settings))
    }

    /// Removes a connection, its key, and any defaults that used it.
    pub fn remove_connection(&self, id: &str) -> Result<Settings, ApiError> {
        let mut state = self.lock();
        self.load(&mut state)?;
        Connections::new(&self.settings, self.secrets.as_ref()).remove(id)?;
        self.load(&mut state)
    }

    /// The models a connection offers, asked for with that connection's own key.
    pub async fn list_models(&self, connection_id: &str) -> Result<Vec<Model>, ApiError> {
        let connection = {
            let mut state = self.lock();
            self.load(&mut state)?
                .connections
                .into_iter()
                .find(|c| c.id == connection_id)
                .ok_or_else(|| {
                    ApiError::new(ErrorCode::NotFound, "That connection no longer exists.")
                })?
        };
        let provider = catalog::provider_by_id(&connection.provider_id).ok_or_else(|| {
            ApiError::new(
                ErrorCode::Invalid,
                "This connection's provider isn't supported by this version of FolderFlow.",
            )
        })?;
        let key = self.secrets.get(&connection.id)?;

        let Some(source) = source_for(&provider.id, connection.endpoint.as_deref(), key.as_ref())
        else {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                format!(
                    "This {} connection is incomplete. Remove it and connect again.",
                    provider.name
                ),
            ));
        };
        let models = self
            .providers
            .list_models(source)
            .await
            .map_err(|e| match e {
                CheckError::Rejected => ApiError::new(ErrorCode::Provider, KEY_REJECTED),
                CheckError::Unreachable => {
                    ApiError::new(ErrorCode::Provider, unreachable(&provider))
                }
                CheckError::Unexpected => unexpected_answer(&provider),
            })?;

        Ok(to_models(&provider, &connection.id, models))
    }

    /// Every workflow file, sorted by name. A damaged or newer file is listed
    /// by its file name and never changed.
    pub fn list_workflows(&self) -> Result<Vec<WorkflowSummary>, ApiError> {
        let listed = self.workflows.list().map_err(WorkflowError::from)?;
        let mut out: Vec<WorkflowSummary> = listed.into_iter().map(summary::of).collect();
        out.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(out)
    }

    pub fn get_workflow(&self, id: &str) -> Result<Workflow, ApiError> {
        Ok(self.workflows.get(id)?)
    }

    /// A blank workflow, or a copy of a template, saved at revision 1.
    pub fn create_workflow(&self, template_id: Option<String>) -> Result<Workflow, ApiError> {
        let workflow = match template_id.as_deref() {
            None => templates::blank(),
            Some(id) => templates::build(id).ok_or_else(|| {
                ApiError::new(
                    ErrorCode::NotFound,
                    "FolderFlow doesn't know that template.",
                )
            })?,
        };
        Ok(self.workflows.create(workflow)?)
    }

    pub fn save_workflow(&self, workflow: Workflow) -> Result<SaveResult, ApiError> {
        let models = self.working_models()?;
        Ok(self.workflows.save(workflow, &models)?)
    }

    /// Moves the workflow's file to the trash.
    pub fn delete_workflow(&self, id: &str) -> Result<(), ApiError> {
        self.workflows.delete(id)?;
        Ok(())
    }

    /// The changes to a running workflow that aren't live yet, if any.
    pub fn get_draft(&self, id: &str) -> Result<Option<Workflow>, ApiError> {
        Ok(self.workflows.get_draft(id)?)
    }

    /// Autosaves edits to a draft, never to the running workflow.
    pub fn save_draft(&self, draft: Workflow) -> Result<SaveResult, ApiError> {
        let models = self.working_models()?;
        Ok(self.workflows.save_draft(draft, &models)?)
    }

    /// Makes the draft the running workflow.
    pub fn apply_draft(&self, id: &str) -> Result<SaveResult, ApiError> {
        let models = self.working_models()?;
        Ok(self.workflows.apply_draft(id, &models)?)
    }

    /// Moves the draft to the trash.
    pub fn discard_draft(&self, id: &str) -> Result<(), ApiError> {
        Ok(self.workflows.discard_draft(id)?)
    }

    pub fn validate_workflow(&self, workflow: Workflow) -> Result<Vec<Problem>, ApiError> {
        let models = self.working_models()?;
        Ok(validate(&workflow, &models))
    }

    pub fn list_templates(&self) -> Vec<Template> {
        catalog::templates()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic mid-command leaves nothing half-written (saves are atomic).
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The model kinds whose default model is on a connection that exists.
    fn working_models(&self) -> Result<BTreeSet<String>, ApiError> {
        let mut state = self.lock();
        let settings = self.load(&mut state)?;
        Ok(settings
            .defaults
            .iter()
            .filter(|(_, model)| {
                model
                    .as_ref()
                    .is_some_and(|m| settings.connections.iter().any(|c| c.id == m.connection_id))
            })
            .map(|(kind, _)| kind.clone())
            .collect())
    }

    /// Loads the settings, remembering a recovery so it keeps being reported.
    fn load(&self, state: &mut State) -> Result<Settings, ApiError> {
        let loaded = self.settings.load()?;
        if let LoadOutcome::Recovered { backup } = loaded.outcome {
            state.notice = Some(SettingsNotice::Recovered {
                backup: backup.display().to_string(),
            });
        }
        Ok(loaded.settings)
    }
}

fn find_provider(id: &str) -> Result<Provider, ApiError> {
    catalog::provider_by_id(id).ok_or_else(|| {
        ApiError::new(
            ErrorCode::NotFound,
            "FolderFlow doesn't know that provider.",
        )
    })
}

/// Which provider API to ask, with only this provider's key. `None` when the
/// provider can't be asked or a key or address it needs is missing.
fn source_for<'a>(
    provider_id: &str,
    endpoint: Option<&'a str>,
    key: Option<&'a Secret>,
) -> Option<ModelSource<'a>> {
    match provider_id {
        "ollama" => Some(ModelSource::Ollama),
        "anthropic" => key.map(|key| ModelSource::Anthropic { key }),
        "openai" => key.map(|key| ModelSource::OpenAi { key }),
        "groq" => key.map(|key| ModelSource::Groq { key }),
        "custom" => endpoint.map(|endpoint| ModelSource::Custom { endpoint, key }),
        _ => None,
    }
}

/// The address with no trailing slash, if it is a usable http(s) address.
fn web_address(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('/');
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return None;
    }
    let url = reqwest::Url::parse(trimmed).ok()?;
    url.host_str()?;
    Some(trimmed.to_owned())
}

fn to_models(provider: &Provider, connection_id: &str, models: Vec<RemoteModel>) -> Vec<Model> {
    let kind = provider.kinds.first().cloned().unwrap_or_default();
    models
        .into_iter()
        .map(|m| Model {
            id: m.id,
            connection_id: connection_id.to_owned(),
            name: m.name,
            kind: kind.clone(),
        })
        .collect()
}

/// Gives each kind with no working default the new connection's first model of
/// that kind. A default that points at an existing connection is left alone.
fn fill_missing_defaults(
    settings: &mut Settings,
    provider: &Provider,
    connection_id: &str,
    models: &[RemoteModel],
) {
    let models = to_models(provider, connection_id, models.to_vec());
    for kind in catalog::model_kinds() {
        let working = settings
            .defaults
            .get(&kind.id)
            .and_then(Option::as_ref)
            .is_some_and(|m| settings.connections.iter().any(|c| c.id == m.connection_id));
        if working {
            continue;
        }
        if let Some(model) = models.iter().find(|m| m.kind == kind.id) {
            settings.defaults.insert(
                kind.id,
                Some(ModelRef {
                    connection_id: connection_id.to_owned(),
                    model_id: model.id.clone(),
                }),
            );
        }
    }
}

fn unreachable(provider: &Provider) -> String {
    format!("Couldn't reach {}.", provider.name)
}

fn unexpected_answer(provider: &Provider) -> ApiError {
    ApiError::new(
        ErrorCode::Provider,
        format!(
            "{} sent an answer FolderFlow couldn't read. Try again later.",
            provider.name
        ),
    )
}

fn unsupported(provider: &Provider) -> ApiError {
    ApiError::new(
        ErrorCode::Invalid,
        format!("{} can't be connected yet.", provider.name),
    )
}
