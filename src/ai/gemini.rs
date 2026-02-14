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

    let res: serde_json::Value = response.json()?;



    let text = res["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("No response");

    Ok(text.to_string())
}
