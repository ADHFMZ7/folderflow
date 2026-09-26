//! The JSON the front end sees: exact camelCase field names and tagged shapes.
//! If one of these changes, src/api/generated and the front end must change too.

mod common;

use std::collections::BTreeMap;

use folderflow_lib::api::catalog;
use folderflow_lib::api::types::{
    ApiError, ConnectOutcome, Credentials, DetectResult, ErrorCode, LoadedSettings, Model,
    SettingsChange, SettingsNotice, WorkflowStatus, WorkflowSummary,
};
use folderflow_lib::storage::settings::{Connection, ModelRef, Settings};
use serde_json::{json, Value};

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    keys
}

fn connection() -> Connection {
    Connection {
        id: "c1".into(),
        provider_id: "ollama".into(),
        endpoint: None,
    }
}

#[test]
fn loaded_settings_has_settings_and_a_null_notice() {
    let loaded = LoadedSettings {
        settings: Settings::default(),
        notice: None,
    };

    let json = to_json(&loaded);

    assert_eq!(keys(&json), ["notice", "settings"]);
    assert!(json["notice"].is_null());
    assert_eq!(
        keys(&json["settings"]),
        ["connections", "defaults", "openAtLogin", "setupComplete"]
    );
}

#[test]
fn a_recovered_notice_names_the_backup() {
    let notice = SettingsNotice::Recovered {
        backup: "/data/settings.damaged-1.json".into(),
    };

    assert_eq!(
        to_json(&notice),
        json!({ "kind": "recovered", "backup": "/data/settings.damaged-1.json" })
    );
}

#[test]
fn a_connection_is_its_id_and_provider() {
    assert_eq!(
        to_json(&connection()),
        json!({ "id": "c1", "providerId": "ollama" })
    );
}

#[test]
fn a_custom_connection_also_carries_its_address() {
    let conn = Connection {
        endpoint: Some("http://studio.local:8080".into()),
        ..connection()
    };

    assert_eq!(
        to_json(&conn),
        json!({ "id": "c1", "providerId": "ollama", "endpoint": "http://studio.local:8080" })
    );
}

#[test]
fn settings_change_fields_are_all_optional() {
    let empty: SettingsChange = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, SettingsChange::default());

    let change: SettingsChange = serde_json::from_value(json!({
        "setupComplete": true,
        "openAtLogin": false,
        "defaults": { "llm": { "connectionId": "c1", "modelId": "m" }, "system1": null }
    }))
    .unwrap();

    assert_eq!(change.setup_complete, Some(true));
    assert_eq!(change.open_at_login, Some(false));
    assert_eq!(
        change.defaults,
        Some(BTreeMap::from([
            (
                "llm".to_string(),
                Some(ModelRef {
                    connection_id: "c1".into(),
                    model_id: "m".into()
                })
            ),
            ("system1".to_string(), None),
        ]))
    );
}

#[test]
fn a_model_kind_lists_the_steps_that_use_it() {
    let json = to_json(&catalog::model_kinds());

    assert_eq!(
        json,
        json!([
            {
                "id": "llm",
                "name": "LLM",
                "description": "Reads and writes text: pulling out details, writing summaries, open-ended steps.",
                "usedBy": ["Extract", "Write", "Agent step"]
            },
            {
                "id": "system1",
                "name": "System 1",
                "description": "Fast, lightweight models that pick one option from a list.",
                "usedBy": ["Classify"]
            }
        ])
    );
}

#[test]
fn providers_match_the_catalog_and_omit_missing_links() {
    let json = to_json(&catalog::providers());

    assert_eq!(
        json,
        json!([
            {
                "id": "ollama", "name": "Ollama", "location": "local", "connect": "detect",
                "kinds": ["llm"], "privacy": "Runs on this Mac. Files never leave it.",
                "helpUrl": "https://ollama.com"
            },
            {
                "id": "custom", "name": "Custom server", "location": "local", "connect": "endpoint",
                "kinds": ["llm"], "privacy": "Files go to the server address you enter."
            },
            {
                "id": "anthropic", "name": "Anthropic", "location": "cloud", "connect": "apiKey",
                "kinds": ["llm"], "privacy": "Files your workflows run on are sent to Anthropic.",
                "keyUrl": "https://platform.claude.com/settings/keys"
            },
            {
                "id": "openai", "name": "OpenAI", "location": "cloud", "connect": "apiKey",
                "kinds": ["llm"], "privacy": "Files your workflows run on are sent to OpenAI.",
                "keyUrl": "https://platform.openai.com/api-keys"
            },
            {
                "id": "groq", "name": "Groq", "location": "cloud", "connect": "apiKey",
                "kinds": ["llm"], "privacy": "Files your workflows run on are sent to Groq.",
                "keyUrl": "https://console.groq.com/keys"
            },
            {
                "id": "jev", "name": "Jev", "location": "cloud", "connect": "apiKey",
                "kinds": ["system1"], "privacy": "Files your workflows run on are sent to Jev."
            }
        ])
    );
}

#[test]
fn templates_match_the_catalog() {
    let json = to_json(&catalog::templates());

    assert_eq!(
        json,
        json!([
            { "id": "receipts", "name": "Sort receipts", "blurb": "Rename receipts by date and vendor, and file them by year", "trigger": "File added" },
            { "id": "screenshots", "name": "Tidy screenshots", "blurb": "Move screenshots off the Desktop into a dated folder", "trigger": "File added" },
            { "id": "summaries", "name": "Summarise PDFs", "blurb": "Write a one-paragraph summary next to each new PDF", "trigger": "File added" },
            { "id": "invoices", "name": "Log invoices", "blurb": "Add each invoice to a spreadsheet, and ask before big ones", "trigger": "File added" },
            { "id": "cleanup", "name": "Weekly clean-up", "blurb": "Every Friday at 17:00, a reminder to tidy Downloads", "trigger": "Schedule" },
            { "id": "paperwork", "name": "Paperwork inbox", "blurb": "Sort receipts, invoices and contracts from Downloads: rename, file and log them, and ask before big invoices", "trigger": "File added" }
        ])
    );
}

#[test]
fn credentials_read_camel_case_and_both_fields_are_optional() {
    let full: Credentials =
        serde_json::from_value(json!({ "apiKey": "sk-1", "endpoint": "http://x" })).unwrap();
    assert_eq!(full.api_key.as_ref().unwrap().expose(), "sk-1");
    assert_eq!(full.endpoint.as_deref(), Some("http://x"));

    let empty: Credentials = serde_json::from_value(json!({})).unwrap();
    assert!(empty.api_key.is_none() && empty.endpoint.is_none());

    let nulls: Credentials =
        serde_json::from_value(json!({ "apiKey": null, "endpoint": null })).unwrap();
    assert!(nulls.api_key.is_none() && nulls.endpoint.is_none());
}

#[test]
fn detect_result_is_tagged_by_found() {
    assert_eq!(to_json(&DetectResult::found()), json!({ "found": true }));
    assert_eq!(
        to_json(&DetectResult::not_found("Not here.")),
        json!({ "found": false, "reason": "Not here." })
    );
}

#[test]
fn connect_outcome_is_tagged_by_ok() {
    let ok = ConnectOutcome::ok(connection(), Settings::default());
    let json = to_json(&ok);
    assert_eq!(keys(&json), ["connection", "ok", "settings"]);
    assert_eq!(json["ok"], true);
    assert_eq!(
        json["connection"],
        json!({ "id": "c1", "providerId": "ollama" })
    );

    assert_eq!(
        to_json(&ConnectOutcome::failed("Enter an API key.")),
        json!({ "ok": false, "error": "Enter an API key." })
    );
}

#[test]
fn a_model_names_its_connection_and_kind() {
    let model = Model {
        id: "claude-x".into(),
        connection_id: "c1".into(),
        name: "Claude X".into(),
        kind: "llm".into(),
    };

    assert_eq!(
        to_json(&model),
        json!({ "id": "claude-x", "connectionId": "c1", "name": "Claude X", "kind": "llm" })
    );
}

#[test]
fn a_workflow_summary_uses_camel_case_and_null_for_never_run() {
    let summary = WorkflowSummary {
        id: "w1".into(),
        name: "Receipts".into(),
        trigger: "File added".into(),
        enabled: true,
        last_run: None,
        needs_you: 2,
        kinds_needed: vec!["llm".into()],
        status: WorkflowStatus::Ok,
        has_draft: true,
    };

    assert_eq!(
        to_json(&summary),
        json!({
            "id": "w1", "name": "Receipts", "trigger": "File added", "enabled": true,
            "lastRun": null, "needsYou": 2, "kindsNeeded": ["llm"], "status": "ok",
            "hasDraft": true
        })
    );
}

#[test]
fn a_workflow_summary_status_is_ok_damaged_or_too_new() {
    let cases = [
        (WorkflowStatus::Ok, "ok"),
        (WorkflowStatus::Damaged, "damaged"),
        (WorkflowStatus::TooNew, "tooNew"),
    ];
    for (status, text) in cases {
        assert_eq!(to_json(&status), json!(text));
    }
}

#[test]
fn api_errors_carry_a_snake_case_code_and_a_message() {
    let cases = [
        (ErrorCode::TooNew, "too_new"),
        (ErrorCode::NotFound, "not_found"),
        (ErrorCode::Invalid, "invalid"),
        (ErrorCode::Keychain, "keychain"),
        (ErrorCode::Provider, "provider"),
        (ErrorCode::Io, "io"),
        (ErrorCode::Conflict, "conflict"),
    ];

    for (code, text) in cases {
        let err = ApiError::new(code, "Something went wrong.");
        assert_eq!(
            to_json(&err),
            json!({ "code": text, "message": "Something went wrong." })
        );
    }
}
