use std::io::{stdin, stdout, Write};
use crate::config::{save_config, AiConfig};

pub fn setup_ai() -> anyhow::Result<String> {
    println!("Choose AI Provider:");
    println!("1. OpenAI");
    println!("2. Claude (Anthropic)");
    println!("3. Gemini");
    println!("4. Ollama (local)");

    print!("Enter choice: ");
    stdout().flush()?;

    let mut choice = String::new();
    stdin().read_line(&mut choice)?;

    let (provider, url) = match choice.trim() {
        "1" => ("openai", "https://platform.openai.com/api-keys"),
        "2" => ("claude", "https://console.anthropic.com/"),
        "3" => ("gemini", "https://makersuite.google.com/app/apikey"),
        "4" => ("ollama", ""),
        _ => ("openai", ""),
    };

    if !url.is_empty() {
        open::that(url)?;
        println!("Opened browser. Create API key and paste here.");
    }

    print!("Paste API Key: ");
    stdout().flush()?;

    let mut key = String::new();
    stdin().read_line(&mut key)?;

    let cfg = AiConfig {
        provider: provider.to_string(),
        api_key: key.trim().to_string(),
    };

    save_config(&cfg)?;

    Ok("AI setup complete. Terrek AI is ready.".into())
}
