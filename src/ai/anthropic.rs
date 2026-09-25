use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use super::{check_status, http_client, ChatModel};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
/// Lets the API re-run a declined request on a fallback model server-side.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

pub struct Anthropic {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl Anthropic {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Result<Self> {
        Ok(Self {
            client: http_client()?,
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
        })
    }

    fn supports_fallbacks(&self) -> bool {
        matches!(self.model.as_str(), "claude-opus-5" | "claude-fable-5-1")
    }
}

pub(crate) fn request_body(model: &str, system: &str, user: &str, fallbacks: bool) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": 16000,
        "system": system,
        "messages": [{ "role": "user", "content": user }],
    });
    if fallbacks {
        body["fallbacks"] = json!("default");
    }
    body
}

pub(crate) fn parse_response(json: &Value) -> Result<String> {
    if json["stop_reason"] == "refusal" {
        let category = json["stop_details"]["category"]
            .as_str()
            .unwrap_or("unspecified");
        bail!("Claude declined this request (category: {category})");
    }
    let text: Vec<&str> = json["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect();
    if text.is_empty() {
        bail!("Claude returned no text");
    }
    Ok(text.join("\n"))
}

impl ChatModel for Anthropic {
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let fallbacks = self.supports_fallbacks();
        let mut request = self
            .client
            .post(format!(
                "{}/v1/messages",
                self.base_url.trim_end_matches('/')
            ))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .json(&request_body(&self.model, system, user, fallbacks));
        if fallbacks {
            request = request.header("anthropic-beta", FALLBACK_BETA);
        }
        let response = check_status("Anthropic", request.send()?)?;
        parse_response(&response.json()?)
    }

    fn label(&self) -> String {
        format!("anthropic/{}", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_shape() {
        let body = request_body("claude-opus-5", "sys", "hi", true);
        assert_eq!(body["system"], "sys");
        assert_eq!(body["messages"][0]["content"], "hi");
        assert_eq!(body["fallbacks"], "default");
        assert!(request_body("claude-haiku-4-5", "s", "u", false)
            .get("fallbacks")
            .is_none());
    }

    #[test]
    fn joins_text_blocks_and_skips_others() {
        let json = json!({
            "stop_reason": "end_turn",
            "content": [
                {"type": "thinking", "thinking": ""},
                {"type": "text", "text": "one"},
                {"type": "text", "text": "two"}
            ]
        });
        assert_eq!(parse_response(&json).unwrap(), "one\ntwo");
    }

    #[test]
    fn refusal_is_an_error() {
        let json =
            json!({"stop_reason": "refusal", "stop_details": {"category": "cyber"}, "content": []});
        assert!(parse_response(&json)
            .unwrap_err()
            .to_string()
            .contains("cyber"));
    }
}
