//! AI providers behind one small trait. Each provider is a blocking HTTP call;
//! callers that must stay responsive run it on a worker thread.

pub mod anthropic;
pub mod auto;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod prompts;
pub mod provider;
pub mod setup;

use anyhow::{anyhow, Result};
use reqwest::blocking::{Client, Response};
use std::time::Duration;

pub use auto::from_config;

pub trait ChatModel: Send {
    /// Single-turn completion: a system prompt plus one user message.
    fn complete(&self, system: &str, user: &str) -> Result<String>;

    /// `anthropic/claude-opus-5`, shown to the user.
    fn label(&self) -> String;
}

pub(crate) fn http_client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("terrek/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()?)
}

/// Turn a non-2xx response into an error that tells the user what to do.
pub(crate) fn check_status(provider: &str, response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().unwrap_or_default();
    let detail = extract_error_message(&body).unwrap_or_else(|| body.chars().take(300).collect());
    let hint = match status.as_u16() {
        401 | 403 => " — the API key was rejected; run `terrek setup`",
        404 => " — check the model name (`terrek config show`)",
        429 => " — rate limited or out of credits; try again shortly",
        500..=599 => " — the provider is having trouble; try again shortly",
        _ => "",
    };
    Err(anyhow!("{provider} returned {status}: {detail}{hint}"))
}

/// Most providers nest the useful text at `error.message` (or `error` as a string).
fn extract_error_message(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let error = json.get("error")?;
    error
        .get("message")
        .and_then(|m| m.as_str())
        .or_else(|| error.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_error_message() {
        let body = r#"{"type":"error","error":{"type":"not_found_error","message":"model: nope"}}"#;
        assert_eq!(extract_error_message(body).as_deref(), Some("model: nope"));
        assert_eq!(
            extract_error_message(r#"{"error":"flat"}"#).as_deref(),
            Some("flat")
        );
        assert_eq!(extract_error_message("not json"), None);
    }
}
