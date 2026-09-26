//! settings.json: loads what was saved, survives damage, never destroys newer data.

mod common;

use std::collections::BTreeMap;
use std::fs;

use folderflow_lib::storage::settings::{
    Appearance, Connection, LoadOutcome, ModelRef, Settings, SettingsError, SettingsStore,
    SETTINGS_VERSION,
};

fn sample() -> Settings {
    Settings {
        setup_complete: true,
        open_at_login: false,
        appearance: Appearance::Dark,
        connections: vec![Connection {
            id: "c1".into(),
            provider_id: "ollama".into(),
            endpoint: None,
        }],
        defaults: BTreeMap::from([
            (
                "llm".into(),
                Some(ModelRef {
                    connection_id: "c1".into(),
                    model_id: "qwen3.5:9b".into(),
                }),
            ),
            ("system1".into(), None),
        ]),
    }
}

#[test]
fn first_launch_loads_defaults_without_creating_a_file() {
    let (_tmp, dir) = common::data_dir();
    let store = SettingsStore::new(&dir);

    let loaded = store.load().unwrap();

    assert_eq!(loaded.outcome, LoadOutcome::Fresh);
    assert_eq!(loaded.settings, Settings::default());
    assert!(loaded.settings.open_at_login);
    assert!(!dir.settings_path().exists());
}

#[test]
fn saved_settings_load_back_unchanged() {
    let (_tmp, dir) = common::data_dir();
    let store = SettingsStore::new(&dir);

    store.save(&sample()).unwrap();
    let loaded = store.load().unwrap();

    assert_eq!(loaded.outcome, LoadOutcome::Loaded);
    assert_eq!(loaded.settings, sample());
}

#[test]
fn the_file_uses_the_field_names_the_front_end_expects() {
    let (_tmp, dir) = common::data_dir();
    SettingsStore::new(&dir).save(&sample()).unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.settings_path()).unwrap()).unwrap();

    assert_eq!(json["version"], SETTINGS_VERSION);
    assert_eq!(json["setupComplete"], true);
    assert_eq!(json["openAtLogin"], false);
    assert_eq!(json["appearance"], "dark");
    assert_eq!(json["connections"][0]["providerId"], "ollama");
    assert_eq!(json["defaults"]["llm"]["connectionId"], "c1");
    assert_eq!(json["defaults"]["llm"]["modelId"], "qwen3.5:9b");
    assert!(json["defaults"]["system1"].is_null());
}

#[test]
fn fields_missing_from_the_file_get_their_defaults() {
    let (_tmp, dir) = common::data_dir();
    fs::write(
        dir.settings_path(),
        r#"{ "version": 1, "setupComplete": true }"#,
    )
    .unwrap();

    let loaded = SettingsStore::new(&dir).load().unwrap();

    assert_eq!(loaded.outcome, LoadOutcome::Loaded);
    assert!(loaded.settings.setup_complete);
    assert!(loaded.settings.open_at_login);
    assert!(loaded.settings.connections.is_empty());
}

#[test]
fn settings_saved_before_appearance_existed_follow_the_system() {
    let (_tmp, dir) = common::data_dir();
    fs::write(
        dir.settings_path(),
        r#"{ "version": 1, "setupComplete": true, "openAtLogin": false }"#,
    )
    .unwrap();

    let loaded = SettingsStore::new(&dir).load().unwrap();

    assert_eq!(loaded.outcome, LoadOutcome::Loaded);
    assert_eq!(loaded.settings.appearance, Appearance::System);
}

#[test]
fn a_damaged_file_is_moved_aside_intact_and_defaults_load() {
    let (_tmp, dir) = common::data_dir();
    let damaged = r#"{ "version": 1, "setupComplete": tr"#;
    fs::write(dir.settings_path(), damaged).unwrap();

    let loaded = SettingsStore::new(&dir).load().unwrap();

    let LoadOutcome::Recovered { backup } = loaded.outcome else {
        panic!("expected Recovered, got {:?}", loaded.outcome);
    };
    assert_eq!(fs::read_to_string(&backup).unwrap(), damaged);
    assert!(backup.starts_with(dir.root()));
    assert_eq!(loaded.settings, Settings::default());
    assert!(!dir.settings_path().exists());
}

#[test]
fn an_empty_file_counts_as_damaged() {
    let (_tmp, dir) = common::data_dir();
    fs::write(dir.settings_path(), "").unwrap();

    let loaded = SettingsStore::new(&dir).load().unwrap();

    assert!(matches!(loaded.outcome, LoadOutcome::Recovered { .. }));
}

#[test]
fn recovering_twice_keeps_both_damaged_files() {
    let (_tmp, dir) = common::data_dir();
    let store = SettingsStore::new(&dir);

    fs::write(dir.settings_path(), "first damage").unwrap();
    let LoadOutcome::Recovered { backup: first } = store.load().unwrap().outcome else {
        panic!()
    };
    fs::write(dir.settings_path(), "second damage").unwrap();
    let LoadOutcome::Recovered { backup: second } = store.load().unwrap().outcome else {
        panic!()
    };

    assert_ne!(first, second);
    assert_eq!(fs::read_to_string(first).unwrap(), "first damage");
    assert_eq!(fs::read_to_string(second).unwrap(), "second damage");
}

#[test]
fn a_file_from_a_newer_version_is_refused_and_never_overwritten() {
    let (_tmp, dir) = common::data_dir();
    let newer = r#"{ "version": 99, "setupComplete": true, "somethingNew": [1, 2, 3] }"#;
    fs::write(dir.settings_path(), newer).unwrap();
    let store = SettingsStore::new(&dir);

    assert!(matches!(
        store.load(),
        Err(SettingsError::TooNew { found: 99 })
    ));
    assert!(matches!(
        store.save(&Settings::default()),
        Err(SettingsError::TooNew { found: 99 })
    ));
    assert_eq!(fs::read_to_string(dir.settings_path()).unwrap(), newer);
}

#[test]
fn the_saved_file_is_readable_only_by_its_owner() {
    let (_tmp, dir) = common::data_dir();
    SettingsStore::new(&dir).save(&sample()).unwrap();

    assert_eq!(common::mode(&dir.settings_path()), 0o600);
}
