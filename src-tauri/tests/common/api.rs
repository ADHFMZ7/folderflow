//! Helpers for the api tests: a backend on a temp data folder, an in-memory key
//! store, and one fake HTTP server per provider.

use std::path::PathBuf;
use std::sync::Arc;

use folderflow_lib::api::providers::{HttpProviders, ProviderUrls};
use folderflow_lib::api::types::Credentials;
use folderflow_lib::api::Backend;
use folderflow_lib::storage::secrets::MemorySecretStore;
use serde_json::json;
use wiremock::matchers::{any, header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// One fake server per provider, so a test can see exactly where each request went.
pub struct Servers {
    pub ollama: MockServer,
    pub anthropic: MockServer,
    pub openai: MockServer,
    pub groq: MockServer,
}

impl Servers {
    pub async fn start() -> Self {
        Self {
            ollama: MockServer::start().await,
            anthropic: MockServer::start().await,
            openai: MockServer::start().await,
            groq: MockServer::start().await,
        }
    }

    pub fn urls(&self) -> ProviderUrls {
        ProviderUrls {
            ollama: self.ollama.uri(),
            anthropic: self.anthropic.uri(),
            openai: self.openai.uri(),
            groq: format!("{}/openai", self.groq.uri()),
        }
    }

    pub async fn requests(server: &MockServer) -> Vec<Request> {
        server.received_requests().await.unwrap_or_default()
    }
}

/// A backend on a temp data folder with an in-memory key store.
pub struct Harness {
    pub _tmp: tempfile::TempDir,
    pub root: PathBuf,
    pub secrets: Arc<MemorySecretStore>,
    pub backend: Backend<HttpProviders>,
}

pub fn harness(urls: ProviderUrls) -> Harness {
    let (tmp, dir) = super::data_dir();
    let root = dir.root().to_path_buf();
    let secrets = Arc::new(MemorySecretStore::default());
    let backend = Backend::new(dir, secrets.clone(), HttpProviders::new(urls));
    Harness {
        _tmp: tmp,
        root,
        secrets,
        backend,
    }
}

/// A backend whose providers all point at an address nothing listens on.
pub fn offline_harness() -> Harness {
    let dead = closed_url();
    harness(ProviderUrls {
        ollama: dead.clone(),
        anthropic: dead.clone(),
        openai: dead.clone(),
        groq: dead,
    })
}

/// An http:// address on this Mac that refuses connections.
pub fn closed_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{addr}")
}

/// Credentials exactly as the front end sends them.
pub fn creds(value: serde_json::Value) -> Credentials {
    serde_json::from_value(value).unwrap()
}

pub fn key(k: &str) -> Credentials {
    creds(json!({ "apiKey": k }))
}

pub fn no_creds() -> Credentials {
    creds(json!({}))
}

pub async fn mount_ollama(server: &MockServer, models: &[&str]) {
    let models: Vec<_> = models.iter().map(|m| json!({ "name": m })).collect();
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "models": models })))
        .mount(server)
        .await;
}

/// Anthropic's model list, answered only for this key and API version.
pub async fn mount_anthropic(server: &MockServer, key: &str, models: &[(&str, &str)]) {
    let data: Vec<_> = models
        .iter()
        .map(|(id, name)| json!({ "id": id, "display_name": name, "type": "model" }))
        .collect();
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("x-api-key", key))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": data })))
        .mount(server)
        .await;
}

/// An OpenAI-style model list at `{prefix}/v1/models`, answered only for this key.
pub async fn mount_openai_style(server: &MockServer, prefix: &str, key: &str, models: &[&str]) {
    let data: Vec<_> = models
        .iter()
        .map(|id| json!({ "id": id, "object": "model" }))
        .collect();
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/v1/models")))
        .and(header("authorization", format!("Bearer {key}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": data })))
        .mount(server)
        .await;
}

/// An OpenAI-style model list that asks for no key.
pub async fn mount_open_models(server: &MockServer, models: &[&str]) {
    let data: Vec<_> = models.iter().map(|id| json!({ "id": id })).collect();
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": data })))
        .mount(server)
        .await;
}

/// Answers every request with `status`.
pub async fn mount_status(server: &MockServer, status: u16) {
    Mock::given(any())
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

/// Everything a request carried that could hold a key: URL, headers and body.
pub fn request_text(r: &Request) -> String {
    let mut text = r.url.to_string();
    for (name, value) in r.headers.iter() {
        text.push_str(&format!(
            "\n{name}: {}",
            String::from_utf8_lossy(value.as_bytes())
        ));
    }
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&r.body));
    text
}
