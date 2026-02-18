use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde_json::json;

use crate::config::load_config;
use crate::state::ContextState;
use crate::ai::prompts::build_gemini_prompt;

fn get_key() -> Result<String> {
    if let Some(cnf) = load_config() {
        return Ok(cnf.gemini_api_key);
    }

    if let Ok(env) = std::env::var("GEMINI_API_KEY") {
        return Ok(env);
    }

    Err(anyhow!("Run `terrek ai setup` first"))
}

pub fn ask_gemini(
    context: &ContextState,
    user_input: &str,
) -> Result<String> {

    let key = get_key()?;

    let final_input = if user_input.trim().is_empty() {
        if context.last_exit_code.unwrap_or(0) != 0 {
            "Explain why the last command failed and suggest a fix."
        } else {
            "Explain the last terminal activity."
        }
    } else {
        user_input
    };

    let prompt = build_gemini_prompt(context, final_input);

    let url = format!(
        "https://generativelanguage.googleapis.com/v1/models/gemini-2.5-flash:generateContent?key={}",
        key
    );

    let body = json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });

    let client = Client::new();

    let response = client.post(&url)
        .json(&body)
        .send()?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Gemini API error: {}",
            response.text().unwrap_or_default()
        ));
    }

    let res: serde_json::Value = response.json()?;

    let text = res
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("No response from Gemini");

    Ok(text.to_string())
}
