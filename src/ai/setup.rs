use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use scopeguard::defer;
use std::io::{self, Write};

use crate::config::{Config, config_exists, save_config};

pub fn setup() -> anyhow::Result<()> {
    if config_exists() {
        println!("\n[Terrek] AI already configured.");
        return Ok(());
    }

    disable_raw_mode()?;
    defer! { let _ = enable_raw_mode(); }

    println!("\nPaste your  API key:");
    print!("> ");
    io::stdout().flush()?;

    let mut key = String::new();
    io::stdin().read_line(&mut key)?;

    let key = key
        .replace("\u{1b}[200~", "")
        .replace("\u{1b}[201~", "")
        .trim()
        .to_string();

    let cfg = Config {
        provider: "gemini".to_string(),
        gemini_api_key: key,
        auto_ai_on_error: true,
    };

    save_config(&cfg)?;

    if config_exists() {
        println!("\n[Terrek] Configuration successful. config.json created.\n");
    } else {
        println!("\n[Terrek] ERROR: config.json not created.\n");
    }

    Ok(())
}
