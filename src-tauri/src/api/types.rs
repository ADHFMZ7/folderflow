//! The types commands take and return. Their JSON is the contract with the front
//! end, and `cargo test` exports each one to src/api/generated/ with ts-rs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, Serializer};
use ts_rs::TS;

use crate::storage::connections::ConnectionError;
use crate::storage::secrets::{Secret, SecretError};
use crate::storage::settings::{Connection, ModelRef, Settings, SettingsError};
use crate::storage::workflows::WorkflowError;

/// What `get_settings` returns.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LoadedSettings {
    pub settings: Settings,
    pub notice: Option<SettingsNotice>,
}

/// Something the user should be told about their settings file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum SettingsNotice {
    /// The file was unreadable; it was kept at `backup` and defaults were used.
    Recovered { backup: String },
}

/// A partial update: each field present replaces the saved one.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct SettingsChange {
    pub setup_complete: Option<bool>,
    pub open_at_login: Option<bool>,
    /// Replaces the whole map when present.
    pub defaults: Option<BTreeMap<String, Option<ModelRef>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelKind {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Step types that need a model of this kind.
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Location {
    Local,
    Cloud,
}

/// How a provider is connected: found on this Mac, an API key, or a server address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConnectMethod {
    Detect,
    ApiKey,
    Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub location: Location,
    pub connect: ConnectMethod,
    pub kinds: Vec<String>,
    /// One sentence on where files go when this provider runs a step.
    pub privacy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub help_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub key_url: Option<String>,
}

/// What the user typed to connect a provider. The backend reads it and never
/// sends it back: it can't be serialised, and its Debug output hides the key.
#[derive(Debug, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Credentials {
    #[ts(optional, as = "Option<String>")]
    pub api_key: Option<Secret>,
    #[ts(optional)]
    pub endpoint: Option<String>,
}

/// Serialises as the literal `true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct True;

/// Serialises as the literal `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct False;

impl Serialize for True {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(true)
    }
}

impl Serialize for False {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(false)
    }
}

/// `{ found: true } | { found: false, reason }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum DetectResult {
    Found {
        #[ts(type = "true")]
        found: True,
    },
    NotFound {
        #[ts(type = "false")]
        found: False,
        reason: String,
    },
}

impl DetectResult {
    pub fn found() -> Self {
        Self::Found { found: True }
    }

    pub fn not_found(reason: impl Into<String>) -> Self {
        Self::NotFound {
            found: False,
            reason: reason.into(),
        }
    }
}

/// `{ ok: true, connection, settings } | { ok: false, error }`. A refusal the
/// user can fix (bad key, unreachable, bad input) is `ok: false`, not an error.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum ConnectOutcome {
    Ok {
        #[ts(type = "true")]
        ok: True,
        connection: Connection,
        settings: Settings,
    },
    Failed {
        #[ts(type = "false")]
        ok: False,
        error: String,
    },
}

impl ConnectOutcome {
    pub fn ok(connection: Connection, settings: Settings) -> Self {
        Self::Ok {
            ok: True,
            connection,
            settings,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self::Failed {
            ok: False,
            error: error.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Model {
    pub id: String,
    pub connection_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub needs_you: u32,
    pub kinds_needed: Vec<String>,
    pub status: WorkflowStatus,
}

/// Whether a workflow's file could be read. For `damaged` and `tooNew`, the
/// summary's `name` is the file name and its other fields are empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkflowStatus {
    Ok,
    /// The file can't be read. It is kept as it is.
    Damaged,
    /// Written by a newer FolderFlow.
    TooNew,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub blurb: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorCode {
    TooNew,
    NotFound,
    Invalid,
    Keychain,
    Provider,
    Io,
    /// A save was based on an older revision than the one on disk.
    Conflict,
}

/// How a command fails. The message is shown to the user and never holds a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[error("{message}")]
#[ts(export)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<SettingsError> for ApiError {
    fn from(e: SettingsError) -> Self {
        match e {
            SettingsError::TooNew { .. } => Self::new(
                ErrorCode::TooNew,
                "Your settings were saved by a newer version of FolderFlow. Update FolderFlow to use them.",
            ),
            SettingsError::Io(e) => Self::new(
                ErrorCode::Io,
                format!("Couldn't read or save your settings: {e}"),
            ),
        }
    }
}

impl From<SecretError> for ApiError {
    fn from(e: SecretError) -> Self {
        // SecretError's text is a fixed description that never holds a key.
        Self::new(
            ErrorCode::Keychain,
            format!("Couldn't use the Keychain: {e}"),
        )
    }
}

impl From<ConnectionError> for ApiError {
    fn from(e: ConnectionError) -> Self {
        match e {
            ConnectionError::Settings(e) => e.into(),
            ConnectionError::Secret(e) => e.into(),
            ConnectionError::NotFound(_) => {
                Self::new(ErrorCode::NotFound, "That connection no longer exists.")
            }
        }
    }
}

impl From<WorkflowError> for ApiError {
    fn from(e: WorkflowError) -> Self {
        match e {
            WorkflowError::InvalidId => Self::new(ErrorCode::Invalid, "That isn't a workflow id."),
            WorkflowError::NotFound => {
                Self::new(ErrorCode::NotFound, "That workflow no longer exists.")
            }
            WorkflowError::TooNew { .. } => Self::new(
                ErrorCode::TooNew,
                "This workflow was saved by a newer version of FolderFlow. Update FolderFlow to use it.",
            ),
            WorkflowError::NotAFile => Self::new(
                ErrorCode::Invalid,
                "This workflow's file is a link or a folder, so FolderFlow won't open it.",
            ),
            WorkflowError::Damaged => Self::new(
                ErrorCode::Io,
                "This workflow's file can't be read. It has been left as it is.",
            ),
            WorkflowError::Conflict { .. } => Self::new(
                ErrorCode::Conflict,
                "This workflow was changed somewhere else. Reopen it to see the latest version.",
            ),
            WorkflowError::NotClean(problems) => Self::new(
                ErrorCode::Invalid,
                match problems.len() {
                    1 => "Fix the problem with this workflow before turning it on.".to_string(),
                    n => format!("Fix the {n} problems with this workflow before turning it on."),
                },
            ),
            WorkflowError::Io(e) => Self::new(
                ErrorCode::Io,
                format!("Couldn't read or save the workflow: {e}"),
            ),
        }
    }
}
