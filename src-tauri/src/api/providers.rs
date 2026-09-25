//! Talking to model providers over HTTP. Each key travels only inside the
//! `ModelSource` of its own provider, and only to that provider's address.

use std::future::Future;
use std::time::Duration;

use reqwest::{RequestBuilder, StatusCode};
use serde::Deserialize;

use crate::storage::secrets::Secret;

/// Where each provider is reached. Tests point these at fake servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderUrls {
    pub ollama: String,
    pub anthropic: String,
    pub openai: String,
    /// Groq's OpenAI-compatible API, under which `/v1/models` lives.
    pub groq: String,
}

impl Default for ProviderUrls {
    fn default() -> Self {
        Self {
            ollama: "http://localhost:11434".into(),
            anthropic: "https://api.anthropic.com".into(),
            openai: "https://api.openai.com".into(),
            groq: "https://api.groq.com/openai".into(),
        }
    }
}

/// One provider to ask for its models, carrying only that provider's key.
pub enum ModelSource<'a> {
    Ollama,
    Anthropic {
        key: &'a Secret,
    },
    OpenAi {
        key: &'a Secret,
    },
    Groq {
        key: &'a Secret,
    },
    /// An OpenAI-compatible server at an address the user entered.
    Custom {
        endpoint: &'a str,
        key: Option<&'a Secret>,
    },
}

/// A model as a provider lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModel {
    pub id: String,
    pub name: String,
}

/// Why a provider didn't list its models. Carries no text from the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckError {
    /// 401 or 403: the key is wrong or revoked.
    Rejected,
    /// No answer: not running, offline, or the address is wrong.
    Unreachable,
    /// An answer we couldn't use: another status, or an unexpected body.
    Unexpected,
}

/// The network boundary. The app uses `HttpProviders`.
pub trait ProviderClient: Send + Sync {
    fn list_models(
        &self,
        source: ModelSource<'_>,
    ) -> impl Future<Output = Result<Vec<RemoteModel>, CheckError>> + Send;
}

pub struct HttpProviders {
    client: reqwest::Client,
    urls: ProviderUrls,
}

impl HttpProviders {
    pub fn new(urls: ProviderUrls) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            // A redirect could carry a key's header to another host.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("the HTTP client's fixed settings are valid");
        Self { client, urls }
    }
}

impl Default for HttpProviders {
    fn default() -> Self {
        Self::new(ProviderUrls::default())
    }
}

impl ProviderClient for HttpProviders {
    async fn list_models(&self, source: ModelSource<'_>) -> Result<Vec<RemoteModel>, CheckError> {
        match source {
            ModelSource::Ollama => {
                let url = format!("{}/api/tags", trim(&self.urls.ollama));
                let tags: OllamaTags = fetch(self.client.get(url)).await?;
                Ok(tags
                    .models
                    .into_iter()
                    .map(|m| RemoteModel {
                        id: m.name.clone(),
                        name: m.name,
                    })
                    .collect())
            }
            ModelSource::Anthropic { key } => {
                let url = format!("{}/v1/models", trim(&self.urls.anthropic));
                let request = self
                    .client
                    .get(url)
                    .header("x-api-key", key.expose())
                    .header("anthropic-version", "2023-06-01");
                let list: DataList<AnthropicModel> = fetch(request).await?;
                Ok(list
                    .data
                    .into_iter()
                    .map(|m| RemoteModel {
                        name: m.display_name.unwrap_or_else(|| m.id.clone()),
                        id: m.id,
                    })
                    .collect())
            }
            ModelSource::OpenAi { key } => self.openai_style(&self.urls.openai, Some(key)).await,
            ModelSource::Groq { key } => self.openai_style(&self.urls.groq, Some(key)).await,
            ModelSource::Custom { endpoint, key } => self.openai_style(endpoint, key).await,
        }
    }
}

impl HttpProviders {
    /// `GET {base}/v1/models`, with the key as a bearer token when there is one.
    async fn openai_style(
        &self,
        base: &str,
        key: Option<&Secret>,
    ) -> Result<Vec<RemoteModel>, CheckError> {
        let base = trim(base);
        // Accept an address entered with or without the /v1 suffix.
        let base = base.strip_suffix("/v1").unwrap_or(base);
        let mut request = self.client.get(format!("{base}/v1/models"));
        if let Some(key) = key {
            request = request.bearer_auth(key.expose());
        }
        let list: DataList<OpenAiModel> = fetch(request).await?;
        Ok(list
            .data
            .into_iter()
            .map(|m| RemoteModel {
                name: m.id.clone(),
                id: m.id,
            })
            .collect())
    }
}

fn trim(base: &str) -> &str {
    base.trim_end_matches('/')
}

/// Sends the request and reads a JSON body. Never passes on the provider's or
/// the HTTP library's own text, which could echo a request header.
async fn fetch<T: for<'de> Deserialize<'de>>(request: RequestBuilder) -> Result<T, CheckError> {
    let response = request.send().await.map_err(|_| CheckError::Unreachable)?;
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(CheckError::Rejected),
        s if s.is_success() => response.json().await.map_err(|_| CheckError::Unexpected),
        _ => Err(CheckError::Unexpected),
    }
}

#[derive(Deserialize)]
struct OllamaTags {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct DataList<T> {
    data: Vec<T>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}
