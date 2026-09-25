//! The critical risks behind the commands: keys never leak, and content goes only
//! to the provider it belongs to. See TESTING.md, "Leaking keys" and "Sending
//! files to the wrong place".

mod common;

use folderflow_lib::api::types::{ConnectOutcome, Credentials};
use folderflow_lib::storage::secrets::Secret;
use serde_json::json;

use common::api::{
    creds, harness, key, mount_anthropic, mount_ollama, mount_openai_style, mount_status,
    offline_harness, request_text, Servers,
};

const ANTHROPIC_KEY: &str = "sk-ant-secret-AAAAAAAAAA";
const OPENAI_KEY: &str = "sk-openai-secret-OOOOOOOOOO";
const GROQ_KEY: &str = "gsk-secret-GGGGGGGGGG";
const CUSTOM_KEY: &str = "custom-secret-CCCCCCCCCC";

#[tokio::test]
async fn a_redirect_never_carries_a_key_to_another_server() {
    let servers = Servers::start().await;
    let elsewhere = wiremock::MockServer::start().await;
    mount_ollama(&elsewhere, &[]).await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(307).insert_header(
            "location",
            format!("{}/v1/models", elsewhere.uri()).as_str(),
        ))
        .mount(&servers.anthropic)
        .await;
    let h = harness(servers.urls());

    let result = h.backend.connect("anthropic", key(ANTHROPIC_KEY)).await;

    assert!(Servers::requests(&elsewhere).await.is_empty());
    assert!(!matches!(result, Ok(ConnectOutcome::Ok { .. })));
    assert!(h.secrets.is_empty());
}

// A key can never be part of what a command returns: the front end only gets
// Serialize types, and Secret is not one.
static_assertions::assert_not_impl_any!(Secret: serde::Serialize, std::fmt::Display, Clone);
static_assertions::assert_not_impl_any!(Credentials: serde::Serialize, Clone);

fn connection_id(outcome: &ConnectOutcome) -> String {
    match outcome {
        ConnectOutcome::Ok { connection, .. } => connection.id.clone(),
        ConnectOutcome::Failed { error, .. } => panic!("connect was refused: {error}"),
    }
}

async fn all_mounted() -> (Servers, wiremock::MockServer) {
    let servers = Servers::start().await;
    mount_anthropic(
        &servers.anthropic,
        ANTHROPIC_KEY,
        &[("claude-a", "Claude A")],
    )
    .await;
    mount_openai_style(&servers.openai, "", OPENAI_KEY, &["gpt-x"]).await;
    mount_openai_style(&servers.groq, "/openai", GROQ_KEY, &["llama"]).await;
    mount_ollama(&servers.ollama, &["qwen3:8b"]).await;
    let custom = wiremock::MockServer::start().await;
    mount_openai_style(&custom, "", CUSTOM_KEY, &["local"]).await;
    (servers, custom)
}

#[test]
fn debug_output_of_credentials_never_shows_the_key() {
    let credentials = key(ANTHROPIC_KEY);

    let printed = format!("{credentials:?} {credentials:#?}");

    assert!(!printed.contains(ANTHROPIC_KEY), "{printed}");
    assert!(!printed.contains("AAAA"), "{printed}");
}

#[tokio::test]
async fn each_key_is_sent_only_to_its_own_provider() {
    let (servers, custom) = all_mounted().await;
    let h = harness(servers.urls());

    let mut ids = Vec::new();
    for (provider, credentials) in [
        ("anthropic", key(ANTHROPIC_KEY)),
        ("openai", key(OPENAI_KEY)),
        ("groq", key(GROQ_KEY)),
        (
            "custom",
            creds(json!({ "endpoint": custom.uri(), "apiKey": CUSTOM_KEY })),
        ),
    ] {
        ids.push(connection_id(
            &h.backend.connect(provider, credentials).await.unwrap(),
        ));
    }
    for id in &ids {
        h.backend.list_models(id).await.unwrap();
    }

    let expectations = [
        (&servers.anthropic, ANTHROPIC_KEY),
        (&servers.openai, OPENAI_KEY),
        (&servers.groq, GROQ_KEY),
        (&custom, CUSTOM_KEY),
    ];
    for (server, own) in expectations {
        let requests = Servers::requests(server).await;
        assert_eq!(requests.len(), 2, "one check and one model list each");
        for request in &requests {
            let text = request_text(request);
            assert!(text.contains(own), "{text}");
            for other in [ANTHROPIC_KEY, OPENAI_KEY, GROQ_KEY, CUSTOM_KEY] {
                if other != own {
                    assert!(
                        !text.contains(other),
                        "{other} was sent to the wrong provider: {text}"
                    );
                }
            }
        }
    }
    assert!(
        Servers::requests(&servers.ollama).await.is_empty(),
        "Ollama was never used but got a request"
    );
}

#[tokio::test]
async fn anthropic_gets_its_key_only_in_its_own_header() {
    let (servers, _custom) = all_mounted().await;
    let h = harness(servers.urls());

    h.backend
        .connect("anthropic", key(ANTHROPIC_KEY))
        .await
        .unwrap();

    let request = &Servers::requests(&servers.anthropic).await[0];
    assert!(!request.headers.contains_key("authorization"));
    assert!(!request.url.to_string().contains(ANTHROPIC_KEY));
}

#[tokio::test]
async fn no_file_in_the_data_folder_contains_a_key_after_connecting() {
    let (servers, custom) = all_mounted().await;
    let h = harness(servers.urls());

    h.backend
        .connect("anthropic", key(ANTHROPIC_KEY))
        .await
        .unwrap();
    h.backend.connect("openai", key(OPENAI_KEY)).await.unwrap();
    h.backend.connect("groq", key(GROQ_KEY)).await.unwrap();
    h.backend
        .connect(
            "custom",
            creds(json!({ "endpoint": custom.uri(), "apiKey": CUSTOM_KEY })),
        )
        .await
        .unwrap();

    for k in [ANTHROPIC_KEY, OPENAI_KEY, GROQ_KEY, CUSTOM_KEY] {
        common::assert_no_file_contains(&h.root, k);
    }
}

#[tokio::test]
async fn nothing_returned_to_the_front_end_contains_the_key() {
    let (servers, _custom) = all_mounted().await;
    let h = harness(servers.urls());

    let outcome = h
        .backend
        .connect("anthropic", key(ANTHROPIC_KEY))
        .await
        .unwrap();
    let id = connection_id(&outcome);
    let returned = [
        serde_json::to_string(&outcome).unwrap(),
        serde_json::to_string(&h.backend.get_settings().unwrap()).unwrap(),
        serde_json::to_string(&h.backend.list_models(&id).await.unwrap()).unwrap(),
        serde_json::to_string(&h.backend.remove_connection(&id).unwrap()).unwrap(),
    ];

    for json in returned {
        assert!(!json.contains(ANTHROPIC_KEY), "{json}");
    }
}

#[tokio::test]
async fn a_rejected_key_is_not_stored_anywhere_or_echoed_back() {
    let servers = Servers::start().await;
    mount_status(&servers.anthropic, 401).await;
    let h = harness(servers.urls());

    let outcome = h
        .backend
        .connect("anthropic", key(ANTHROPIC_KEY))
        .await
        .unwrap();

    assert!(matches!(outcome, ConnectOutcome::Failed { .. }));
    assert!(!serde_json::to_string(&outcome)
        .unwrap()
        .contains(ANTHROPIC_KEY));
    assert!(h.secrets.is_empty());
    assert!(h
        .backend
        .get_settings()
        .unwrap()
        .settings
        .connections
        .is_empty());
    common::assert_no_file_contains(&h.root, ANTHROPIC_KEY);
}

#[tokio::test]
async fn failures_never_echo_the_key() {
    let servers = Servers::start().await;
    mount_status(&servers.openai, 500).await;
    let h = harness(servers.urls());
    let offline = offline_harness();

    let error = h
        .backend
        .connect("openai", key(OPENAI_KEY))
        .await
        .unwrap_err();
    let unreachable = offline
        .backend
        .connect("openai", key(OPENAI_KEY))
        .await
        .unwrap();
    let jev = offline
        .backend
        .connect("jev", key(OPENAI_KEY))
        .await
        .unwrap();

    for text in [
        serde_json::to_string(&error).unwrap(),
        format!("{error} {error:?}"),
        serde_json::to_string(&unreachable).unwrap(),
        serde_json::to_string(&jev).unwrap(),
    ] {
        assert!(!text.contains(OPENAI_KEY), "{text}");
    }
}

#[tokio::test]
async fn ollama_requests_go_only_to_the_configured_local_address() {
    let (servers, _custom) = all_mounted().await;
    let h = harness(servers.urls());

    h.backend.detect("ollama").await.unwrap();
    let id = connection_id(&h.backend.connect("ollama", key("")).await.unwrap());
    h.backend.list_models(&id).await.unwrap();

    assert_eq!(Servers::requests(&servers.ollama).await.len(), 3);
    for server in [&servers.anthropic, &servers.openai, &servers.groq] {
        assert!(Servers::requests(server).await.is_empty());
    }
}
