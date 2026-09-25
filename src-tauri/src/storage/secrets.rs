//! API keys. A `Secret` can't be printed, displayed or serialised, so it can't
//! reach a log, an error message, a file or the front end by accident. It can be
//! deserialised, so a key the user types arrives straight in a `Secret`.

use std::fmt;

/// An API key. Read it with `expose` at the one place it is sent to its provider.
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl<'de> serde::Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(redacted)")
    }
}

/// Failures from the secret store. Messages never include a key.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("the Keychain refused the request: {0}")]
    Backend(String),
}

/// Where keys are kept, one per account (a connection id).
pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, secret: &Secret) -> Result<(), SecretError>;
    fn get(&self, account: &str) -> Result<Option<Secret>, SecretError>;
    /// Deleting an account that has no key is not an error.
    fn delete(&self, account: &str) -> Result<(), SecretError>;
}

/// The macOS Keychain, under one service name for the whole app.
pub struct KeychainStore {
    service: String,
}

impl KeychainStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl KeychainStore {
    fn entry(&self, account: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(&self.service, account).map_err(|e| keychain_error(&e))
    }
}

impl SecretStore for KeychainStore {
    fn set(&self, account: &str, secret: &Secret) -> Result<(), SecretError> {
        self.entry(account)?
            .set_password(secret.expose())
            .map_err(|e| keychain_error(&e))
    }

    fn get(&self, account: &str) -> Result<Option<Secret>, SecretError> {
        match self.entry(account)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keychain_error(&e)),
        }
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        match self.entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keychain_error(&e)),
        }
    }
}

/// A fixed description per failure. Some keyring errors carry the stored bytes,
/// so their own text is never passed on.
fn keychain_error(e: &keyring::Error) -> SecretError {
    let reason = match e {
        keyring::Error::NoStorageAccess(_) => "FolderFlow isn't allowed to use the Keychain",
        keyring::Error::NoEntry => "no key is stored",
        keyring::Error::BadEncoding(_) => "a stored key isn't valid text",
        keyring::Error::TooLong(..) => "the key is too long for the Keychain",
        keyring::Error::Invalid(..) => "the Keychain rejected the request",
        keyring::Error::Ambiguous(_) => "more than one Keychain entry matches",
        _ => "the Keychain reported an error",
    };
    SecretError::Backend(reason.into())
}

/// An in-memory store for tests and previews.
#[derive(Default)]
pub struct MemorySecretStore {
    keys: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn is_empty(&self) -> bool {
        self.keys.lock().unwrap().is_empty()
    }
}

impl SecretStore for MemorySecretStore {
    fn set(&self, account: &str, secret: &Secret) -> Result<(), SecretError> {
        self.keys
            .lock()
            .unwrap()
            .insert(account.to_owned(), secret.expose().to_owned());
        Ok(())
    }

    fn get(&self, account: &str) -> Result<Option<Secret>, SecretError> {
        Ok(self
            .keys
            .lock()
            .unwrap()
            .get(account)
            .map(|v| Secret::new(v.as_str())))
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        self.keys.lock().unwrap().remove(account);
        Ok(())
    }
}
