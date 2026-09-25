//! connect, remove_connection, list_models and detect against fake providers.

mod common;

use folderflow_lib::api::types::{ConnectOutcome, DetectResult, ErrorCode, Model};
use folderflow_lib::storage::secrets::SecretStore;
use folderflow_lib::storage::settings::{Connection, ModelRef, Settings};
use serde_json::json;

use common::api::{
    creds, harness, key, mount_anthropic, mount_ollama, mount_open_models, mount_openai_style,
    mount_status, no_creds, offline_harness, Harness, Servers,
};

const ANTHROPIC_KEY: &str = "sk-ant-test-1111111111";
const OPENAI_KEY: &str = "sk-openai-test-2222222222";

fn model_ref(connection: &Connection, model_id: &str) -> Option<ModelRef> {
    Some(ModelRef {
        connection_id: connection.id.clone(),
        model_id: model_id.into(),
    })
}

/// The connection and settings of a successful connect; panics on a refusal.
fn connected(outcome: ConnectOutcome) -> (Connection, Settings) {
    match outcome {
        ConnectOutcome::Ok {
            connection,
            settings,
            ..
        } => (connection, settings),
        ConnectOutcome::Failed { error, .. } => panic!("connect was refused: {error}"),
    }
}

/// The message of a refused connect; panics on a success.
fn refused(outcome: ConnectOutcome) -> String {
    match outcome {
        ConnectOutcome::Failed { error, .. } => error,
        ConnectOutcome::Ok { .. } => panic!("expected connect to be refused"),
    }
}

fn saved(h: &Harness) -> Settings {
    h.backend.get_settings().unwrap().settings
}

fn assert_nothing_stored(h: &Harness) {
    assert!(h.secrets.is_empty(), "a key was stored");
    assert!(saved(h).connections.is_empty(), "a connection was added");
}

// ---- connect ----

#[tokio::test]
async fn connecting_with_a_good_key_adds_the_connection_stores_the_key_and_sets_the_default() {
    let servers = Servers::start().await;
    mount_anthropic(
        &servers.anthropic,
        ANTHROPIC_KEY,
        &[("claude-a", "Claude A"), ("claude-b", "Claude B")],
    )
    .await;
    let h = harness(servers.urls());

    let (conn, settings) = connected(
        h.backend
            .connect("anthropic", key(ANTHROPIC_KEY))
            .await
            .unwrap(),
    );

    assert_eq!(conn.provider_id, "anthropic");
    assert_eq!(settings.connections, vec![conn.clone()]);
    assert_eq!(settings.defaults["llm"], model_ref(&conn, "claude-a"));
    assert_eq!(settings.defaults.get("system1").cloned().flatten(), None);
    assert_eq!(saved(&h), settings);
    assert_eq!(
        h.secrets.get(&conn.id).unwrap().unwrap().expose(),
        ANTHROPIC_KEY
    );
}

#[tokio::test]
async fn a_second_connection_does_not_take_over_a_working_default() {
    let servers = Servers::start().await;
    mount_anthropic(
        &servers.anthropic,
        ANTHROPIC_KEY,
        &[("claude-a", "Claude A")],
    )
    .await;
    mount_openai_style(&servers.openai, "", OPENAI_KEY, &["gpt-x"]).await;
    let h = harness(servers.urls());

    let (first, _) = connected(
        h.backend
            .connect("anthropic", key(ANTHROPIC_KEY))
            .await
            .unwrap(),
    );
    let (second, settings) = connected(h.backend.connect("openai", key(OPENAI_KEY)).await.unwrap());

    assert_eq!(settings.defaults["llm"], model_ref(&first, "claude-a"));
    assert_eq!(settings.connections, vec![first, second]);
}

#[tokio::test]
async fn a_default_pointing_at_a_missing_connection_is_replaced() {
    let servers = Servers::start().await;
    mount_ollama(&servers.ollama, &["qwen3:8b"]).await;
    let h = harness(servers.urls());
    // A stale default, as an older build might have left behind.
    std::fs::write(
        h.root.join("settings.json"),
        json!({
            "version": 1,
            "defaults": { "llm": { "connectionId": "gone", "modelId": "old" }, "system1": null }
        })
        .to_string(),
    )
    .unwrap();

    let (conn, settings) = connected(h.backend.connect("ollama", no_creds()).await.unwrap());

    assert_eq!(settings.defaults["llm"], model_ref(&conn, "qwen3:8b"));
    assert_eq!(settings.defaults["system1"], None);
}

#[tokio::test]
async fn a_connection_with_no_models_sets_no_default() {
    let servers = Servers::start().await;
    mount_ollama(&servers.ollama, &[]).await;
    let h = harness(servers.urls());

    let (_, settings) = connected(h.backend.connect("ollama", no_creds()).await.unwrap());

    assert_eq!(settings.defaults.get("llm").cloned().flatten(), None);
}

#[tokio::test]
async fn a_missing_or_blank_key_is_refused_before_any_request() {
    let servers = Servers::start().await;
    let h = harness(servers.urls());

    for credentials in [no_creds(), key(""), key("   ")] {
        let error = refused(h.backend.connect("anthropic", credentials).await.unwrap());
        assert_eq!(error, "Enter an API key.");
    }

    assert!(Servers::requests(&servers.anthropic).await.is_empty());
    assert_nothing_stored(&h);
}

#[tokio::test]
async fn a_rejected_key_is_refused_and_nothing_is_stored() {
    for status in [401, 403] {
        let servers = Servers::start().await;
        mount_status(&servers.openai, status).await;
        let h = harness(servers.urls());

        let error = refused(h.backend.connect("openai", key(OPENAI_KEY)).await.unwrap());

        assert_eq!(error, "That key was rejected. Check it and try again.");
        assert_nothing_stored(&h);
    }
}

#[tokio::test]
async fn an_unreachable_provider_is_refused_by_name() {
    let h = offline_harness();

    let error = refused(
        h.backend
            .connect("groq", key("gsk-123456789"))
            .await
            .unwrap(),
    );

    assert_eq!(error, "Couldn't reach Groq.");
    assert_nothing_stored(&h);
}

#[tokio::test]
async fn an_unexpected_answer_is_a_provider_error_and_nothing_is_stored() {
    let servers = Servers::start().await;
    mount_status(&servers.anthropic, 500).await;
    let h = harness(servers.urls());

    let err = h
        .backend
        .connect("anthropic", key(ANTHROPIC_KEY))
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::Provider);
    assert_nothing_stored(&h);
}

#[tokio::test]
async fn an_unknown_provider_is_not_found() {
    let h = offline_harness();

    let err = h
        .backend
        .connect("no-such-provider", no_creds())
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn jev_cannot_be_connected_yet() {
    let h = offline_harness();

    let error = refused(
        h.backend
            .connect("jev", key("jev-key-12345"))
            .await
            .unwrap(),
    );

    assert_eq!(error, "Jev can't be connected yet.");
    assert_nothing_stored(&h);
}

#[tokio::test]
async fn a_custom_server_needs_an_http_address() {
    let h = offline_harness();

    for endpoint in [
        None,
        Some(""),
        Some("studio.local:8080"),
        Some("ftp://studio.local"),
    ] {
        let error = refused(
            h.backend
                .connect("custom", creds(json!({ "endpoint": endpoint })))
                .await
                .unwrap(),
        );
        assert_eq!(error, "Enter an address starting with http:// or https://.");
    }
    assert_nothing_stored(&h);
}

#[tokio::test]
async fn a_custom_server_without_a_key_gets_no_authorization_header() {
    let custom = wiremock::MockServer::start().await;
    mount_open_models(&custom, &["local-model"]).await;
    let h = offline_harness();

    let (conn, settings) = connected(
        h.backend
            .connect("custom", creds(json!({ "endpoint": custom.uri() })))
            .await
            .unwrap(),
    );

    assert_eq!(conn.endpoint.as_deref(), Some(custom.uri().as_str()));
    assert_eq!(settings.defaults["llm"], model_ref(&conn, "local-model"));
    assert!(h.secrets.is_empty());
    let requests = Servers::requests(&custom).await;
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("authorization"));
}

#[tokio::test]
async fn a_custom_server_with_a_key_gets_it_as_a_bearer_token() {
    let custom = wiremock::MockServer::start().await;
    mount_openai_style(&custom, "", "custom-key-12345", &["m"]).await;
    let h = offline_harness();

    let (conn, _) = connected(
        h.backend
            .connect(
                "custom",
                creds(json!({ "endpoint": format!("{}/", custom.uri()), "apiKey": "custom-key-12345" })),
            )
            .await
            .unwrap(),
    );

    assert_eq!(
        h.secrets.get(&conn.id).unwrap().unwrap().expose(),
        "custom-key-12345"
    );
}

#[tokio::test]
async fn ollama_needs_no_key_and_never_stores_one() {
    let servers = Servers::start().await;
    mount_ollama(&servers.ollama, &["qwen3:8b"]).await;
    let h = harness(servers.urls());

    let (conn, _) = connected(
        h.backend
            .connect("ollama", key("stray-key-123456"))
            .await
            .unwrap(),
    );

    assert!(h.secrets.is_empty());
    assert_eq!(conn.provider_id, "ollama");
    let requests = Servers::requests(&servers.ollama).await;
    assert!(requests
        .iter()
        .all(|r| !common::api::request_text(r).contains("stray-key")));
}

// ---- remove_connection ----

#[tokio::test]
async fn removing_a_connection_deletes_its_key_and_clears_its_defaults() {
    let servers = Servers::start().await;
    mount_anthropic(
        &servers.anthropic,
        ANTHROPIC_KEY,
        &[("claude-a", "Claude A")],
    )
    .await;
    let h = harness(servers.urls());
    let (conn, _) = connected(
        h.backend
            .connect("anthropic", key(ANTHROPIC_KEY))
            .await
            .unwrap(),
    );

    let settings = h.backend.remove_connection(&conn.id).unwrap();

    assert!(settings.connections.is_empty());
    assert_eq!(settings.defaults["llm"], None);
    assert!(h.secrets.get(&conn.id).unwrap().is_none());
    assert_eq!(saved(&h), settings);
}

#[test]
fn removing_an_unknown_connection_is_not_found() {
    let h = offline_harness();

    let err = h.backend.remove_connection("no-such-id").unwrap_err();

    assert_eq!(err.code, ErrorCode::NotFound);
}

// ---- list_models ----

#[tokio::test]
async fn models_are_listed_with_the_connections_own_key() {
    let servers = Servers::start().await;
    mount_anthropic(
        &servers.anthropic,
        ANTHROPIC_KEY,
        &[("claude-a", "Claude A"), ("claude-b", "Claude B")],
    )
    .await;
    let h = harness(servers.urls());
    let (conn, _) = connected(
        h.backend
            .connect("anthropic", key(ANTHROPIC_KEY))
            .await
            .unwrap(),
    );

    let models = h.backend.list_models(&conn.id).await.unwrap();

    assert_eq!(
        models,
        vec![
            Model {
                id: "claude-a".into(),
                connection_id: conn.id.clone(),
                name: "Claude A".into(),
                kind: "llm".into()
            },
            Model {
                id: "claude-b".into(),
                connection_id: conn.id.clone(),
                name: "Claude B".into(),
                kind: "llm".into()
            },
        ]
    );
}

#[tokio::test]
async fn openai_style_models_are_named_by_id() {
    let servers = Servers::start().await;
    mount_openai_style(&servers.groq, "/openai", "gsk-key-123456", &["llama-3"]).await;
    let h = harness(servers.urls());
    let (conn, _) = connected(
        h.backend
            .connect("groq", key("gsk-key-123456"))
            .await
            .unwrap(),
    );

    let models = h.backend.list_models(&conn.id).await.unwrap();

    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "llama-3");
    assert_eq!(models[0].name, "llama-3");
}

#[tokio::test]
async fn a_custom_connection_lists_models_from_its_own_address() {
    let custom = wiremock::MockServer::start().await;
    mount_open_models(&custom, &["local-model"]).await;
    let h = offline_harness();
    let (conn, _) = connected(
        h.backend
            .connect("custom", creds(json!({ "endpoint": custom.uri() })))
            .await
            .unwrap(),
    );

    let models = h.backend.list_models(&conn.id).await.unwrap();

    assert_eq!(models[0].id, "local-model");
    assert_eq!(Servers::requests(&custom).await.len(), 2);
}

#[tokio::test]
async fn listing_models_for_an_unknown_connection_is_not_found() {
    let h = offline_harness();

    let err = h.backend.list_models("no-such-id").await.unwrap_err();

    assert_eq!(err.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn a_key_revoked_after_connecting_is_a_provider_error() {
    let servers = Servers::start().await;
    mount_openai_style(&servers.openai, "", OPENAI_KEY, &["gpt-x"]).await;
    let h = harness(servers.urls());
    let (conn, _) = connected(h.backend.connect("openai", key(OPENAI_KEY)).await.unwrap());
    servers.openai.reset().await;
    mount_status(&servers.openai, 401).await;

    let err = h.backend.list_models(&conn.id).await.unwrap_err();

    assert_eq!(err.code, ErrorCode::Provider);
    assert_eq!(
        err.message,
        "That key was rejected. Check it and try again."
    );
}

// ---- detect ----

#[tokio::test]
async fn ollama_is_found_when_it_answers() {
    let servers = Servers::start().await;
    mount_ollama(&servers.ollama, &["qwen3:8b"]).await;
    let h = harness(servers.urls());

    assert_eq!(
        h.backend.detect("ollama").await.unwrap(),
        DetectResult::found()
    );
}

#[tokio::test]
async fn ollama_is_not_found_when_nothing_listens() {
    let h = offline_harness();

    assert_eq!(
        h.backend.detect("ollama").await.unwrap(),
        DetectResult::not_found("Ollama isn't running on this Mac.")
    );
}

#[tokio::test]
async fn detect_only_applies_to_detectable_providers() {
    let h = offline_harness();

    assert_eq!(
        h.backend.detect("anthropic").await.unwrap_err().code,
        ErrorCode::Invalid
    );
    assert_eq!(
        h.backend.detect("no-such-provider").await.unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn the_default_ollama_address_is_on_this_mac() {
    let urls = folderflow_lib::api::providers::ProviderUrls::default();

    assert_eq!(urls.ollama, "http://localhost:11434");
    assert_eq!(urls.anthropic, "https://api.anthropic.com");
    assert_eq!(urls.openai, "https://api.openai.com");
    assert_eq!(urls.groq, "https://api.groq.com/openai");
}
