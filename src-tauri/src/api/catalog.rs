//! The fixed lists the app ships with: model kinds, providers and templates.
//! This is the single source; the front end reads them through commands.

use super::types::{ConnectMethod, Location, ModelKind, Provider, Template};

pub fn model_kinds() -> Vec<ModelKind> {
    vec![
        kind(
            "llm",
            "LLM",
            "Reads and writes text: pulling out details, writing summaries, open-ended steps.",
            &["Extract", "Write", "Agent step"],
        ),
        kind(
            "system1",
            "System 1",
            "Fast, lightweight models that pick one option from a list.",
            &["Classify"],
        ),
    ]
}

pub fn providers() -> Vec<Provider> {
    use ConnectMethod::{Detect, Endpoint};
    use Location::Local;
    vec![
        Provider {
            help_url: Some("https://ollama.com".into()),
            ..provider(
                "ollama",
                "Ollama",
                Local,
                Detect,
                "llm",
                "Runs on this Mac. Files never leave it.",
            )
        },
        provider(
            "custom",
            "Custom server",
            Local,
            Endpoint,
            "llm",
            "Files go to the server address you enter.",
        ),
        Provider {
            key_url: Some("https://platform.claude.com/settings/keys".into()),
            ..cloud("anthropic", "Anthropic", "llm")
        },
        Provider {
            key_url: Some("https://platform.openai.com/api-keys".into()),
            ..cloud("openai", "OpenAI", "llm")
        },
        Provider {
            key_url: Some("https://console.groq.com/keys".into()),
            ..cloud("groq", "Groq", "llm")
        },
        // A placeholder: its API isn't known yet, so it can't be connected.
        cloud("jev", "Jev", "system1"),
    ]
}

pub fn templates() -> Vec<Template> {
    vec![
        template(
            "receipts",
            "Sort receipts",
            "Rename receipts by date and vendor, and file them by year",
            "File added",
        ),
        template(
            "screenshots",
            "Tidy screenshots",
            "Move screenshots off the Desktop into a dated folder",
            "File added",
        ),
        template(
            "summaries",
            "Summarise PDFs",
            "Write a one-paragraph summary next to each new PDF",
            "File added",
        ),
        template(
            "invoices",
            "Log invoices",
            "Add each invoice to a spreadsheet, and ask before big ones",
            "File added",
        ),
        template(
            "cleanup",
            "Weekly clean-up",
            "Every Friday, archive Downloads files older than 30 days",
            "Schedule",
        ),
    ]
}

pub fn provider_by_id(id: &str) -> Option<Provider> {
    providers().into_iter().find(|p| p.id == id)
}

fn kind(id: &str, name: &str, description: &str, used_by: &[&str]) -> ModelKind {
    ModelKind {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        used_by: used_by.iter().map(|s| s.to_string()).collect(),
    }
}

fn provider(
    id: &str,
    name: &str,
    location: Location,
    connect: ConnectMethod,
    kind: &str,
    privacy: &str,
) -> Provider {
    Provider {
        id: id.into(),
        name: name.into(),
        location,
        connect,
        kinds: vec![kind.into()],
        privacy: privacy.into(),
        help_url: None,
        key_url: None,
    }
}

/// A cloud provider connected by API key.
fn cloud(id: &str, name: &str, kind: &str) -> Provider {
    provider(
        id,
        name,
        Location::Cloud,
        ConnectMethod::ApiKey,
        kind,
        &format!("Files your workflows run on are sent to {name}."),
    )
}

fn template(id: &str, name: &str, blurb: &str, trigger: &str) -> Template {
    Template {
        id: id.into(),
        name: name.into(),
        blurb: blurb.into(),
        trigger: trigger.into(),
    }
}
