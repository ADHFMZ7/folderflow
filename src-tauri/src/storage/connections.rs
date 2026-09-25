//! Adding and removing provider connections: the connection goes in settings,
//! its key in the secret store, and the two never disagree.

use super::secrets::{Secret, SecretError, SecretStore};
use super::settings::{Connection, SettingsError, SettingsStore};

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error("no connection with id {0}")]
    NotFound(String),
}

pub struct Connections<'a> {
    settings: &'a SettingsStore,
    secrets: &'a dyn SecretStore,
}

impl<'a> Connections<'a> {
    pub fn new(settings: &'a SettingsStore, secrets: &'a dyn SecretStore) -> Self {
        Self { settings, secrets }
    }

    /// Adds a connection. Its key, if any, is stored first; if that fails, nothing is added.
    pub fn add(
        &self,
        provider_id: &str,
        key: Option<Secret>,
    ) -> Result<Connection, ConnectionError> {
        self.add_with_endpoint(provider_id, None, key)
    }

    /// Adds a connection to a provider reached at a server address the user chose.
    pub fn add_with_endpoint(
        &self,
        provider_id: &str,
        endpoint: Option<String>,
        key: Option<Secret>,
    ) -> Result<Connection, ConnectionError> {
        let mut settings = self.settings.load()?.settings;
        let connection = Connection {
            id: uuid::Uuid::new_v4().to_string(),
            provider_id: provider_id.to_owned(),
            endpoint,
        };

        if let Some(key) = &key {
            self.secrets.set(&connection.id, key)?;
        }
        settings.connections.push(connection.clone());
        if let Err(e) = self.settings.save(&settings) {
            // Take the key back out, so the Keychain holds nothing no connection refers to.
            if key.is_some() {
                let _ = self.secrets.delete(&connection.id);
            }
            return Err(e.into());
        }
        Ok(connection)
    }

    /// Removes a connection, its key, and any defaults that pointed at it.
    /// The key goes first; if deleting it fails, the connection stays so the key isn't orphaned.
    pub fn remove(&self, id: &str) -> Result<(), ConnectionError> {
        let mut settings = self.settings.load()?.settings;
        if !settings.connections.iter().any(|c| c.id == id) {
            return Err(ConnectionError::NotFound(id.to_owned()));
        }

        self.secrets.delete(id)?;
        settings.connections.retain(|c| c.id != id);
        for default in settings.defaults.values_mut() {
            if default.as_ref().is_some_and(|m| m.connection_id == id) {
                *default = None;
            }
        }
        self.settings.save(&settings)?;
        Ok(())
    }

    /// The key for one connection, and only that connection.
    pub fn key_for(&self, id: &str) -> Result<Option<Secret>, ConnectionError> {
        Ok(self.secrets.get(id)?)
    }
}
