use anyhow :: {anyhow, Result};
use reqwest :: blocking :: Client;

use serde_json::json;

use crate::config::load_config;

fn get_key() -> Result<String>{
    if let Some(cnf) = load_config(){
        return Ok(cnf.gemini_api_key);
    }
     if let Ok(env) = std::env::var("GEMINI_API_KEY") {
        return Ok(env);
    }

    Err(anyhow!("Run `terrek ai setup` first"))
}

pub fn ask_gemini(prompt: &str) -> Result<String> {
    let key = get_key()?;

   let url = format!(
    "https://generativelanguage.googleapis.com/v1/models/gemini-1.5-flash:generateContent?key={}",
    key
);




    let body = json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });

    let client = Client::new();
    let response = client.post(&url).json(&body).send()?;

    if !response.status().is_success() {
        let err_text = response.text()?;
        return Err(anyhow!("Gemini API error: {}", err_text));
    }

    let res: serde_json::Value = response.json()?;

    // Handle missing candidates properly
    let text = res
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow!("Invalid Gemini response format: {}", res))?;

    Ok(text.to_string())
}
