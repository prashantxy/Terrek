use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use super::{check_status, http_client, ChatModel};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct Gemini {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl Gemini {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Result<Self> {
        Ok(Self {
            client: http_client()?,
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
        })
    }
}

pub(crate) fn request_body(system: &str, user: &str) -> Value {
    json!({
        "systemInstruction": { "parts": [{ "text": system }] },
        "contents": [{ "role": "user", "parts": [{ "text": user }] }]
    })
}

pub(crate) fn parse_response(json: &Value) -> Result<String> {
    let text: Vec<&str> = json["candidates"][0]["content"]["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect();
    if text.is_empty() {
        let reason = json["candidates"][0]["finishReason"]
            .as_str()
            .or_else(|| json["promptFeedback"]["blockReason"].as_str())
            .unwrap_or("no text");
        bail!("Gemini returned no answer ({reason})");
    }
    Ok(text.concat())
}

impl ChatModel for Gemini {
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        // The key goes in a header, not the URL, so it never shows up in error messages or logs.
        let url = format!(
            "{}/models/{}:generateContent",
            self.base_url.trim_end_matches('/'),
            self.model
        );
        let response = self
            .client
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .json(&request_body(system, user))
            .send()?;
        let response = check_status("Gemini", response)?;
        parse_response(&response.json()?)
    }

    fn label(&self) -> String {
        format!("gemini/{}", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concatenates_parts() {
        let json = json!({"candidates": [{"content": {"parts": [{"text": "a"}, {"text": "b"}]}}]});
        assert_eq!(parse_response(&json).unwrap(), "ab");
    }

    #[test]
    fn reports_block_reason() {
        let json = json!({"promptFeedback": {"blockReason": "SAFETY"}});
        assert!(parse_response(&json)
            .unwrap_err()
            .to_string()
            .contains("SAFETY"));
    }
}
