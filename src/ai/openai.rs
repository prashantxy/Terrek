//! OpenAI Chat Completions. `base_url` makes this work with any compatible
//! server (OpenRouter, Groq, LM Studio, vLLM, ...).

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use super::{check_status, http_client, ChatModel};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAi {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAi {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Result<Self> {
        Ok(Self {
            client: http_client()?,
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
        })
    }
}

pub(crate) fn request_body(model: &str, system: &str, user: &str) -> Value {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    })
}

pub(crate) fn parse_response(json: &Value) -> Result<String> {
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("OpenAI returned no message content"))
}

impl ChatModel for OpenAi {
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let mut request = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.base_url.trim_end_matches('/')
            ))
            .json(&request_body(&self.model, system, user));
        if !self.api_key.is_empty() {
            request = request.bearer_auth(&self.api_key);
        }
        let response = check_status("OpenAI", request.send()?)?;
        parse_response(&response.json()?)
    }

    fn label(&self) -> String {
        format!("openai/{}", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_uses_messages_array() {
        let body = request_body("m", "sys", "hi");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "hi");
    }

    #[test]
    fn parses_first_choice() {
        let json = json!({"choices": [{"message": {"role": "assistant", "content": "ok"}}]});
        assert_eq!(parse_response(&json).unwrap(), "ok");
        assert!(parse_response(&json!({"choices": []})).is_err());
    }
}
