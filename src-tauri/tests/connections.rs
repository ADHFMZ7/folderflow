//! Connections and their keys stay in step, and keys never land in a file.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::Ordering;

use folderflow_lib::storage::connections::{ConnectionError, Connections};
use folderflow_lib::storage::secrets::{MemorySecretStore, Secret, SecretStore};
use folderflow_lib::storage::settings::{ModelRef, Settings, SettingsStore};

use common::FlakySecrets;

const KEY: &str = "sk-live-1234567890abcdef";

#[test]
fn a_key_is_stored_under_its_connection_and_never_in_settings() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();

    let conn = Connections::new(&settings, &secrets)
        .add("anthropic", Some(Secret::new(KEY)))
        .unwrap();

    assert_eq!(conn.provider_id, "anthropic");
    assert_eq!(
        settings.load().unwrap().settings.connections,
        vec![conn.clone()]
    );
    assert_eq!(secrets.get(&conn.id).unwrap().unwrap().expose(), KEY);
    common::assert_no_file_contains(dir.root(), KEY);
}

#[test]
fn no_file_in_the_data_folder_ever_contains_a_key() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();
    let conns = Connections::new(&settings, &secrets);

    let a = conns.add("anthropic", Some(Secret::new(KEY))).unwrap();
    conns
        .add("groq", Some(Secret::new("gsk-other-key-0987654321")))
        .unwrap();
    conns.add("ollama", None).unwrap();
    conns.remove(&a.id).unwrap();

    common::assert_no_file_contains(dir.root(), KEY);
    common::assert_no_file_contains(dir.root(), "gsk-other-key-0987654321");
}

#[test]
fn a_local_provider_needs_no_key() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();

    let conn = Connections::new(&settings, &secrets)
        .add("ollama", None)
        .unwrap();

    assert!(secrets.get(&conn.id).unwrap().is_none());
    assert_eq!(settings.load().unwrap().settings.connections.len(), 1);
}

#[test]
fn every_connection_gets_its_own_id() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();
    let conns = Connections::new(&settings, &secrets);

    let a = conns
        .add("openai", Some(Secret::new("key-one-123456")))
        .unwrap();
    let b = conns
        .add("openai", Some(Secret::new("key-two-123456")))
        .unwrap();

    assert_ne!(a.id, b.id);
    assert_eq!(
        conns.key_for(&a.id).unwrap().unwrap().expose(),
        "key-one-123456"
    );
    assert_eq!(
        conns.key_for(&b.id).unwrap().unwrap().expose(),
        "key-two-123456"
    );
}

#[test]
fn if_the_key_cant_be_stored_no_connection_is_added() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = FlakySecrets::failing_set();

    let result = Connections::new(&settings, &secrets).add("anthropic", Some(Secret::new(KEY)));

    assert!(matches!(result, Err(ConnectionError::Secret(_))));
    assert!(settings.load().unwrap().settings.connections.is_empty());
}

#[test]
fn if_settings_cant_be_saved_the_key_is_taken_back_out() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();
    // A read-only data folder: loading works, saving fails after the key is stored.
    fs::set_permissions(dir.root(), fs::Permissions::from_mode(0o500)).unwrap();

    let result = Connections::new(&settings, &secrets).add("anthropic", Some(Secret::new(KEY)));
    fs::set_permissions(dir.root(), fs::Permissions::from_mode(0o700)).unwrap();

    assert!(matches!(result, Err(ConnectionError::Settings(_))));
    assert!(
        secrets.is_empty(),
        "the Keychain still holds a key no connection refers to"
    );
}

#[test]
fn error_messages_never_contain_the_key() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = FlakySecrets::failing_set();

    let err = Connections::new(&settings, &secrets)
        .add("anthropic", Some(Secret::new(KEY)))
        .unwrap_err();

    let shown = format!("{err} {err:?}");
    assert!(!shown.contains(KEY), "{shown}");
}

#[test]
fn removing_a_connection_deletes_its_key() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();
    let conns = Connections::new(&settings, &secrets);
    let conn = conns.add("anthropic", Some(Secret::new(KEY))).unwrap();

    conns.remove(&conn.id).unwrap();

    assert!(secrets.get(&conn.id).unwrap().is_none());
    assert!(settings.load().unwrap().settings.connections.is_empty());
}

#[test]
fn if_the_key_cant_be_deleted_the_connection_stays() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = FlakySecrets::default();
    let conns = Connections::new(&settings, &secrets);
    let conn = conns.add("anthropic", Some(Secret::new(KEY))).unwrap();
    secrets.fail_delete.store(true, Ordering::SeqCst);

    assert!(conns.remove(&conn.id).is_err());

    assert_eq!(
        settings.load().unwrap().settings.connections,
        vec![conn.clone()]
    );
    assert_eq!(secrets.get(&conn.id).unwrap().unwrap().expose(), KEY);
}

#[test]
fn removing_a_connection_clears_defaults_that_used_it() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();
    let conns = Connections::new(&settings, &secrets);
    let gone = conns
        .add("groq", Some(Secret::new("gsk-1234567890ab")))
        .unwrap();
    let kept = conns.add("ollama", None).unwrap();
    let mut s = settings.load().unwrap().settings;
    s.defaults = BTreeMap::from([
        (
            "llm".into(),
            Some(ModelRef {
                connection_id: gone.id.clone(),
                model_id: "gpt-oss".into(),
            }),
        ),
        (
            "system1".into(),
            Some(ModelRef {
                connection_id: kept.id.clone(),
                model_id: "tiny".into(),
            }),
        ),
    ]);
    settings.save(&s).unwrap();

    conns.remove(&gone.id).unwrap();

    let after: Settings = settings.load().unwrap().settings;
    assert_eq!(after.defaults["llm"], None);
    assert_eq!(
        after.defaults["system1"].as_ref().unwrap().connection_id,
        kept.id
    );
}

#[test]
fn removing_an_unknown_connection_is_an_error() {
    let (_tmp, dir) = common::data_dir();
    let settings = SettingsStore::new(&dir);
    let secrets = MemorySecretStore::default();

    let result = Connections::new(&settings, &secrets).remove("no-such-id");

    assert!(matches!(result, Err(ConnectionError::NotFound(id)) if id == "no-such-id"));
}
