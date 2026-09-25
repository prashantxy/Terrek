//! Local models through Ollama: no key, nothing leaves the machine.

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use super::{check_status, http_client, ChatModel};

const DEFAULT_BASE_URL: &str = "http://localhost:11434";

pub struct Ollama {
    client: Client,
    model: String,
    base_url: String,
}

impl Ollama {
    pub fn new(model: String, base_url: Option<String>) -> Result<Self> {
        Ok(Self {
            client: http_client()?,
            model,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
        })
    }
}

pub(crate) fn request_body(model: &str, system: &str, user: &str) -> Value {
    json!({
        "model": model,
        "stream": false,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    })
}

pub(crate) fn parse_response(json: &Value) -> Result<String> {
    json["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Ollama returned no message content"))
}

impl ChatModel for Ollama {
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(&url)
            .json(&request_body(&self.model, system, user))
            .send()
            .with_context(|| {
                format!(
                    "could not reach Ollama at {} — is `ollama serve` running?",
                    self.base_url
                )
            })?;
        let response = check_status("Ollama", response)?;
        parse_response(&response.json()?)
    }

    fn label(&self) -> String {
        format!("ollama/{}", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_streaming_chat() {
        let body = request_body("llama3.2", "s", "u");
        assert_eq!(body["stream"], false);
        let json = json!({"message": {"role": "assistant", "content": "hi"}, "done": true});
        assert_eq!(parse_response(&json).unwrap(), "hi");
    }
}
