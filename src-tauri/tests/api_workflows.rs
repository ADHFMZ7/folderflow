//! The workflow commands: summaries, create from a template, save with revision
//! checks, delete to the trash, and validation against the models in settings.

mod common;

use std::collections::BTreeMap;
use std::fs;

use folderflow_lib::api::types::{ErrorCode, WorkflowStatus, WorkflowSummary};
use folderflow_lib::storage::data_dir::DataDir;
use folderflow_lib::storage::settings::{Connection, ModelRef, Settings, SettingsStore};
use folderflow_lib::workflow::{Every, ProblemCode, Schedule, StepKind, Workflow};
use serde_json::json;

use common::api::{offline_harness, Harness};

const ID: &str = "0f8c2b1e-6b0a-4c1e-9d6a-2f1f6c0f7a11";

fn workflows_dir(h: &Harness) -> std::path::PathBuf {
    h.root.join("workflows")
}

/// Settings where every kind in `kinds` has a default on a live connection.
fn with_models(h: &Harness, kinds: &[&str]) {
    let defaults = kinds
        .iter()
        .map(|k| {
            (
                k.to_string(),
                Some(ModelRef {
                    connection_id: "c1".into(),
                    model_id: "m".into(),
                }),
            )
        })
        .collect();
    let settings = Settings {
        connections: vec![Connection {
            id: "c1".into(),
            provider_id: "ollama".into(),
            endpoint: None,
        }],
        defaults,
        ..Settings::default()
    };
    SettingsStore::new(&DataDir::open(&h.root).unwrap())
        .save(&settings)
        .unwrap();
}

fn summary_of(h: &Harness, id: &str) -> WorkflowSummary {
    h.backend
        .list_workflows()
        .unwrap()
        .into_iter()
        .find(|s| s.id == id)
        .unwrap()
}

fn set_trigger(h: &Harness, id: &str, kind: StepKind) -> Workflow {
    let mut w = h.backend.get_workflow(id).unwrap();
    w.steps[0].kind = kind;
    h.backend.save_workflow(w).unwrap().workflow
}

#[test]
fn no_workflows_lists_nothing() {
    let h = offline_harness();
    assert_eq!(h.backend.list_workflows().unwrap(), []);
}

#[test]
fn create_with_no_template_saves_a_blank_workflow() {
    let h = offline_harness();

    let w = h.backend.create_workflow(None).unwrap();

    assert_eq!(w.name, "New workflow");
    assert_eq!(w.revision, 1);
    assert_eq!(h.backend.get_workflow(&w.id).unwrap(), w);
    assert_eq!(
        summary_of(&h, &w.id),
        WorkflowSummary {
            id: w.id.clone(),
            name: "New workflow".into(),
            trigger: "File added · ~/Downloads".into(),
            enabled: false,
            last_run: None,
            needs_you: 0,
            kinds_needed: vec![],
            status: WorkflowStatus::Ok,
            has_draft: false,
        }
    );
}

#[test]
fn create_from_a_template_copies_it_with_a_new_id() {
    let h = offline_harness();

    let a = h.backend.create_workflow(Some("receipts".into())).unwrap();
    let b = h.backend.create_workflow(Some("receipts".into())).unwrap();

    assert_ne!(a.id, b.id);
    assert_eq!(a.name, "Sort receipts");
    assert_eq!(a.revision, 1);
    let s = summary_of(&h, &a.id);
    assert_eq!(s.kinds_needed, ["llm", "system1"]);
    assert_eq!(s.trigger, "File added · ~/Downloads");
}

#[test]
fn create_from_an_unknown_template_is_not_found() {
    let h = offline_harness();

    let err = h.backend.create_workflow(Some("nope".into())).unwrap_err();

    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(!workflows_dir(&h).exists());
}

#[test]
fn summaries_describe_the_trigger() {
    let h = offline_harness();
    let id = h.backend.create_workflow(None).unwrap().id;
    let schedule = |every, time: &str, weekday| StepKind::Schedule {
        schedule: Schedule {
            every,
            time: time.into(),
            weekday,
        },
        next: None,
    };

    let cases = [
        (
            schedule(Every::Weekday, "09:00", None),
            "Schedule · Weekdays 09:00",
        ),
        (
            schedule(Every::Day, "07:30", None),
            "Schedule · Every day 07:30",
        ),
        (
            schedule(Every::Week, "17:00", Some(5)),
            "Schedule · Fridays 17:00",
        ),
        (
            schedule(Every::Week, "08:00", Some(0)),
            "Schedule · Sundays 08:00",
        ),
        (
            schedule(Every::Week, "08:00", None),
            "Schedule · Every week 08:00",
        ),
        (StepKind::RunNow { next: None }, "Run now"),
        (
            StepKind::FileAdded {
                folder: "~/Desktop".into(),
                file_types: vec!["png".into()],
                subfolders: true,
                next: None,
            },
            "File added · ~/Desktop",
        ),
    ];
    for (kind, text) in cases {
        set_trigger(&h, &id, kind);
        assert_eq!(summary_of(&h, &id).trigger, text);
    }
}

#[test]
fn a_workflow_with_no_trigger_says_so() {
    let h = offline_harness();
    let mut w = h.backend.create_workflow(None).unwrap();
    w.steps.clear();
    h.backend.save_workflow(w.clone()).unwrap();

    assert_eq!(summary_of(&h, &w.id).trigger, "No trigger");
}

#[test]
fn kinds_needed_lists_each_kind_once() {
    let h = offline_harness();
    let summaries = h.backend.create_workflow(Some("summaries".into())).unwrap();
    let invoices = h.backend.create_workflow(Some("invoices".into())).unwrap();
    let cleanup = h.backend.create_workflow(Some("cleanup".into())).unwrap();

    assert_eq!(summary_of(&h, &summaries.id).kinds_needed, ["llm"]);
    assert_eq!(summary_of(&h, &invoices.id).kinds_needed, ["llm"]);
    assert_eq!(
        summary_of(&h, &cleanup.id).kinds_needed,
        Vec::<String>::new()
    );
    assert_eq!(
        summary_of(&h, &cleanup.id).trigger,
        "Schedule · Fridays 17:00"
    );
}

#[test]
fn summaries_are_sorted_by_name() {
    let h = offline_harness();
    for t in ["summaries", "cleanup", "receipts"] {
        h.backend.create_workflow(Some(t.into())).unwrap();
    }

    let names: Vec<_> = h
        .backend
        .list_workflows()
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();

    assert_eq!(
        names,
        ["Sort receipts", "Summarise PDFs", "Weekly clean-up"]
    );
}

#[test]
fn damaged_and_newer_files_are_listed_by_file_name_and_left_alone() {
    let h = offline_harness();
    let ok = h.backend.create_workflow(None).unwrap();
    let damaged_id = "11111111-2222-4333-8444-555555555555";
    let damaged = workflows_dir(&h).join(format!("{damaged_id}.json"));
    fs::write(&damaged, "not json").unwrap();
    let newer = workflows_dir(&h).join(format!("{ID}.json"));
    let newer_bytes = json!({ "version": 9, "id": ID }).to_string();
    fs::write(&newer, &newer_bytes).unwrap();

    let list = h.backend.list_workflows().unwrap();

    assert_eq!(list.len(), 3);
    let empty = |id: &str, name: String, status| WorkflowSummary {
        id: id.into(),
        name,
        trigger: String::new(),
        enabled: false,
        last_run: None,
        needs_you: 0,
        kinds_needed: vec![],
        status,
        has_draft: false,
    };
    assert!(list.contains(&empty(
        damaged_id,
        format!("{damaged_id}.json"),
        WorkflowStatus::Damaged
    )));
    assert!(list.contains(&empty(ID, format!("{ID}.json"), WorkflowStatus::TooNew)));
    assert!(list
        .iter()
        .any(|s| s.id == ok.id && s.status == WorkflowStatus::Ok));
    assert_eq!(fs::read_to_string(&damaged).unwrap(), "not json");
    assert_eq!(fs::read_to_string(&newer).unwrap(), newer_bytes);
}

#[test]
fn errors_map_to_the_contract_codes() {
    let h = offline_harness();
    assert_eq!(
        h.backend.get_workflow("../settings").unwrap_err().code,
        ErrorCode::Invalid
    );
    assert_eq!(
        h.backend.get_workflow(ID).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        h.backend.delete_workflow(ID).unwrap_err().code,
        ErrorCode::NotFound
    );

    fs::create_dir_all(workflows_dir(&h)).unwrap();
    let path = workflows_dir(&h).join(format!("{ID}.json"));
    fs::write(&path, json!({ "version": 2 }).to_string()).unwrap();
    assert_eq!(
        h.backend.get_workflow(ID).unwrap_err().code,
        ErrorCode::TooNew
    );
    assert_eq!(
        h.backend.delete_workflow(ID).unwrap_err().code,
        ErrorCode::TooNew
    );

    fs::write(&path, "garbage").unwrap();
    let err = h.backend.get_workflow(ID).unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.ends_with('.'));
}

#[test]
fn a_stale_save_is_a_conflict() {
    let h = offline_harness();
    let w = h.backend.create_workflow(None).unwrap();
    let saved = h.backend.save_workflow(w.clone()).unwrap();
    assert_eq!(saved.workflow.revision, 2);

    let err = h.backend.save_workflow(w).unwrap_err();

    assert_eq!(err.code, ErrorCode::Conflict);
}

#[test]
fn turning_on_needs_a_default_model_for_every_ai_step() {
    let h = offline_harness();
    let w = h.backend.create_workflow(Some("receipts".into())).unwrap();
    let on = Workflow {
        enabled: true,
        ..w.clone()
    };

    let err = h.backend.save_workflow(on.clone()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(h.backend.get_workflow(&w.id).unwrap(), w);

    with_models(&h, &["llm"]);
    assert_eq!(
        h.backend.save_workflow(on.clone()).unwrap_err().code,
        ErrorCode::Invalid
    );

    with_models(&h, &["llm", "system1"]);
    let saved = h.backend.save_workflow(on).unwrap();
    assert!(saved.workflow.enabled);
    assert_eq!(saved.problems, []);
    assert!(summary_of(&h, &w.id).enabled);
}

#[test]
fn a_default_on_a_removed_connection_is_no_model() {
    let h = offline_harness();
    let settings = Settings {
        defaults: BTreeMap::from([(
            "llm".to_string(),
            Some(ModelRef {
                connection_id: "gone".into(),
                model_id: "m".into(),
            }),
        )]),
        ..Settings::default()
    };
    SettingsStore::new(&DataDir::open(&h.root).unwrap())
        .save(&settings)
        .unwrap();
    let w = h.backend.create_workflow(Some("summaries".into())).unwrap();

    let problems = h.backend.validate_workflow(w).unwrap();

    let codes: Vec<_> = problems.iter().map(|p| p.code).collect();
    assert_eq!(codes, [ProblemCode::NoModel]);
}

#[test]
fn validate_uses_the_current_settings_and_writes_nothing() {
    let h = offline_harness();
    let w = build_unsaved();

    let without = h.backend.validate_workflow(w.clone()).unwrap();
    with_models(&h, &["llm", "system1"]);
    let with = h.backend.validate_workflow(w).unwrap();

    assert_eq!(without.len(), 2);
    assert!(without.iter().all(|p| p.code == ProblemCode::NoModel));
    assert_eq!(with, []);
    assert!(!workflows_dir(&h).exists());
}

fn build_unsaved() -> Workflow {
    folderflow_lib::workflow::templates::build("receipts").unwrap()
}

#[test]
fn saving_with_settings_from_a_newer_version_is_refused() {
    let h = offline_harness();
    let w = h.backend.create_workflow(None).unwrap();
    fs::write(h.root.join("settings.json"), r#"{"version": 99}"#).unwrap();

    assert_eq!(
        h.backend.save_workflow(w.clone()).unwrap_err().code,
        ErrorCode::TooNew
    );
    assert_eq!(
        h.backend.validate_workflow(w).unwrap_err().code,
        ErrorCode::TooNew
    );
}

#[test]
fn delete_moves_the_workflow_to_the_trash() {
    let h = offline_harness();
    let w = h.backend.create_workflow(None).unwrap();

    h.backend.delete_workflow(&w.id).unwrap();

    assert_eq!(h.backend.list_workflows().unwrap(), []);
    let trashed: Vec<_> = fs::read_dir(workflows_dir(&h).join(".trash"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(trashed.len(), 1);
    assert!(trashed[0].starts_with(&w.id));
}
