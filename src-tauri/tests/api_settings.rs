//! get_settings and update_settings: notices, partial changes, validation, and no
//! lost updates when commands run at the same time.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::sync::Barrier;

use folderflow_lib::api::catalog;
use folderflow_lib::api::types::{ErrorCode, SettingsChange, SettingsNotice};
use folderflow_lib::storage::connections::Connections;
use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::storage::settings::{Connection, ModelRef, Settings, SettingsStore};

use common::api::{offline_harness, Harness};

fn store(h: &Harness) -> SettingsStore {
    SettingsStore::new(&DataDir::open(&h.root).unwrap())
}

fn model(connection_id: &str, model_id: &str) -> Option<ModelRef> {
    Some(ModelRef {
        connection_id: connection_id.into(),
        model_id: model_id.into(),
    })
}

fn with_connection(h: &Harness, id: &str) -> Settings {
    let settings = Settings {
        connections: vec![Connection {
            id: id.into(),
            provider_id: "ollama".into(),
            endpoint: None,
        }],
        ..Settings::default()
    };
    store(h).save(&settings).unwrap();
    settings
}

#[test]
fn first_launch_returns_defaults_and_no_notice() {
    let h = offline_harness();

    let loaded = h.backend.get_settings().unwrap();

    assert_eq!(loaded.settings, Settings::default());
    assert_eq!(loaded.notice, None);
}

#[test]
fn a_damaged_file_is_reported_with_its_backup() {
    let h = offline_harness();
    fs::write(h.root.join("settings.json"), "{ not json").unwrap();

    let loaded = h.backend.get_settings().unwrap();

    let Some(SettingsNotice::Recovered { backup }) = loaded.notice else {
        panic!("expected a recovered notice, got {:?}", loaded.notice);
    };
    assert_eq!(fs::read_to_string(&backup).unwrap(), "{ not json");
    assert_eq!(loaded.settings, Settings::default());
}

#[test]
fn the_recovered_notice_is_reported_on_every_call_for_the_life_of_the_app() {
    let h = offline_harness();
    fs::write(h.root.join("settings.json"), "{ not json").unwrap();

    let first = h.backend.get_settings().unwrap().notice;
    let second = h.backend.get_settings().unwrap().notice;
    h.backend
        .update_settings(SettingsChange {
            setup_complete: Some(true),
            ..SettingsChange::default()
        })
        .unwrap();
    let third = h.backend.get_settings().unwrap().notice;

    assert!(matches!(first, Some(SettingsNotice::Recovered { .. })));
    assert_eq!(second, first);
    assert_eq!(third, first);
}

#[test]
fn a_recovery_during_another_command_is_still_reported() {
    let h = offline_harness();
    fs::write(h.root.join("settings.json"), "{ not json").unwrap();

    h.backend
        .update_settings(SettingsChange {
            open_at_login: Some(false),
            ..SettingsChange::default()
        })
        .unwrap();
    let loaded = h.backend.get_settings().unwrap();

    assert!(matches!(
        loaded.notice,
        Some(SettingsNotice::Recovered { .. })
    ));
    assert!(!loaded.settings.open_at_login);
}

#[test]
fn a_file_from_a_newer_version_is_too_new() {
    let h = offline_harness();
    fs::write(h.root.join("settings.json"), r#"{ "version": 99 }"#).unwrap();

    let err = h.backend.get_settings().unwrap_err();

    assert_eq!(err.code, ErrorCode::TooNew);
}

#[test]
fn update_applies_only_the_fields_it_names() {
    let h = offline_harness();
    let before = with_connection(&h, "c1");

    let after = h
        .backend
        .update_settings(SettingsChange {
            setup_complete: Some(true),
            ..SettingsChange::default()
        })
        .unwrap();

    assert!(after.setup_complete);
    assert_eq!(after.open_at_login, before.open_at_login);
    assert_eq!(after.connections, before.connections);
    assert_eq!(store(&h).load().unwrap().settings, after);
}

#[test]
fn update_replaces_the_whole_defaults_map() {
    let h = offline_harness();
    let mut before = with_connection(&h, "c1");
    before.defaults = BTreeMap::from([("system1".to_string(), model("c1", "old"))]);
    store(&h).save(&before).unwrap();

    let after = h
        .backend
        .update_settings(SettingsChange {
            defaults: Some(BTreeMap::from([("llm".to_string(), model("c1", "qwen3"))])),
            ..SettingsChange::default()
        })
        .unwrap();

    assert_eq!(
        after.defaults,
        BTreeMap::from([("llm".to_string(), model("c1", "qwen3"))])
    );
    assert!(after.open_at_login);
}

#[test]
fn a_default_can_be_cleared() {
    let h = offline_harness();
    with_connection(&h, "c1");

    let after = h
        .backend
        .update_settings(SettingsChange {
            defaults: Some(BTreeMap::from([("llm".to_string(), None)])),
            ..SettingsChange::default()
        })
        .unwrap();

    assert_eq!(after.defaults["llm"], None);
}

#[test]
fn a_default_pointing_at_an_unknown_connection_is_refused_and_nothing_changes() {
    let h = offline_harness();
    let before = with_connection(&h, "c1");

    let err = h
        .backend
        .update_settings(SettingsChange {
            setup_complete: Some(true),
            defaults: Some(BTreeMap::from([("llm".to_string(), model("gone", "m"))])),
            ..SettingsChange::default()
        })
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(store(&h).load().unwrap().settings, before);
}

#[test]
fn update_refuses_to_overwrite_a_newer_file() {
    let h = offline_harness();
    let newer = r#"{ "version": 99, "setupComplete": false }"#;
    fs::write(h.root.join("settings.json"), newer).unwrap();

    let err = h
        .backend
        .update_settings(SettingsChange {
            setup_complete: Some(true),
            ..SettingsChange::default()
        })
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::TooNew);
    assert_eq!(
        fs::read_to_string(h.root.join("settings.json")).unwrap(),
        newer
    );
}

#[test]
fn concurrent_updates_to_different_fields_are_all_kept() {
    let h = offline_harness();

    for _ in 0..40 {
        store(&h).save(&Settings::default()).unwrap();
        let barrier = Barrier::new(2);

        std::thread::scope(|s| {
            s.spawn(|| {
                barrier.wait();
                h.backend
                    .update_settings(SettingsChange {
                        setup_complete: Some(true),
                        ..SettingsChange::default()
                    })
                    .unwrap();
            });
            s.spawn(|| {
                barrier.wait();
                h.backend
                    .update_settings(SettingsChange {
                        open_at_login: Some(false),
                        ..SettingsChange::default()
                    })
                    .unwrap();
            });
        });

        let settings = store(&h).load().unwrap().settings;
        assert!(settings.setup_complete, "the setupComplete change was lost");
        assert!(!settings.open_at_login, "the openAtLogin change was lost");
    }
}

#[test]
fn concurrent_removals_and_updates_lose_nothing() {
    let h = offline_harness();
    let ids: Vec<String> = {
        let settings = store(&h);
        let conns = Connections::new(&settings, h.secrets.as_ref());
        (0..8)
            .map(|_| conns.add("ollama", None).unwrap().id)
            .collect()
    };
    let barrier = Barrier::new(ids.len() + 1);

    std::thread::scope(|s| {
        for id in &ids {
            let (h, barrier) = (&h, &barrier);
            s.spawn(move || {
                barrier.wait();
                h.backend.remove_connection(id).unwrap();
            });
        }
        s.spawn(|| {
            barrier.wait();
            h.backend
                .update_settings(SettingsChange {
                    setup_complete: Some(true),
                    ..SettingsChange::default()
                })
                .unwrap();
        });
    });

    let settings = store(&h).load().unwrap().settings;
    assert!(
        settings.connections.is_empty(),
        "{:?}",
        settings.connections
    );
    assert!(settings.setup_complete);
}

#[test]
fn the_catalog_lists_come_from_the_backend() {
    let h = offline_harness();

    assert_eq!(h.backend.list_model_kinds(), catalog::model_kinds());
    assert_eq!(h.backend.list_providers(), catalog::providers());
    assert_eq!(h.backend.list_templates(), catalog::templates());
    assert!(h.backend.list_workflows().unwrap().is_empty());
}
