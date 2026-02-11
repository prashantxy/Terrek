use std::io::{self, Write};
use crate::config::{save_config, Config};

pub fn setup() -> anyhow::Result<()> {
    println!("Paste your Gemini API key:");
    print!("> ");
    io::stdout().flush()?;

    let mut key = String::new();
    io::stdin().read_line(&mut key)?;

    let cfg = Config {
        provider: "gemini".to_string(),
        gemini_api_key: key.trim().to_string(),
    };

    save_config(&cfg)?;

    println!("Gemini configured successfully.");
    Ok(())
}
