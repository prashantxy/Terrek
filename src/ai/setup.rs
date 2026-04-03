use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use scopeguard::defer;
use std::io::{self, Write};

use crate::config::{Config, config_exists, save_config};

fn detect_provider(api_key: &str) -> String {
    if api_key.starts_with("sk-ant-") {
        "claude".to_string()
    } else if api_key.starts_with("sk-") {
        "openai".to_string()
    } else if api_key.starts_with("AIza") {
        "gemini".to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn setup() -> anyhow::Result<()> {
    if config_exists() {
        println!("\n[Terrek] AI already configured.");
        return Ok(());
    }

    disable_raw_mode()?;
    defer! { let _ = enable_raw_mode(); }

    println!("\nPaste your API key:");
    print!("> ");
    io::stdout().flush()?;

    let mut key = String::new();
    io::stdin().read_line(&mut key)?;

    let key = key
        .replace("\u{1b}[200~", "")
        .replace("\u{1b}[201~", "")
        .trim()
        .to_string();

    let provider = detect_provider(&key);

    if provider == "unknown" {
        println!("\n[Terrek] ❌ Could not detect provider.");
        println!("Try specifying manually later.\n");
    } else {
        println!("\n[Terrek] 🔍 Detected provider: {}", provider);
    }

    let cfg = Config {
        provider,
        api_key: key,
        auto_ai_on_error: true,
    };

    save_config(&cfg)?;

    if config_exists() {
        println!("\n[Terrek] Configuration successful.\n");
    } else {
        println!("\n[Terrek]  ERROR: config not created.\n");
    }

    Ok(())
}