use crate::ai::gemini::ask_gemini;
use crate::ai::setup::setup as ai_setup;
use crate::ai::provider::Provider;
use crate::context::ContextState;
use crate::db::history::{get_history, search_history};
use crate::config::{load_config, delete_config};
use anyhow::Result;
use chrono::{Local, TimeZone};

pub enum TerrekAction {
    Output(String),
}

// helper: capitalize provider name
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn handle_command(context: &ContextState, cmd: &str) -> Result<TerrekAction> {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();

    if parts.is_empty() {
        return Ok(TerrekAction::Output(String::new()));
    }

    let output = match parts[0] {
        "hello" => "Hello from Terrek!".to_string(),

        "time" => format!("Current time: {}", Local::now()),

        "clear" => "__CLEAR__".to_string(),

        "history" => {
            let history = get_history(10)?;
            let mut lines = Vec::new();

            for (cmd, _, ts) in history {
                let time = Local.timestamp_opt(ts, 0).unwrap();
                lines.push(format!("[{}] {}", time.format("%H:%M:%S"), cmd));
            }

            lines.join("\n")
        }

        "last" => {
            let history = get_history(1)?;
            if let Some((cmd, output, ts)) = history.first() {
                let time = Local.timestamp_opt(*ts, 0).unwrap();

                format!("Last Command [{}]:\n{}\n\nOutput:\n{}", time, cmd, output)
            } else {
                "No history found".to_string()
            }
        }

        "search" => {
            if parts.len() < 2 {
                "Usage: terrek search <keyword>".to_string()
            } else {
                let results = search_history(parts[1])?;
                let mut lines = Vec::new();

                for (cmd, _, ts) in results {
                    let time = Local.timestamp_opt(ts, 0).unwrap();
                    lines.push(format!("[{}] {}", time.format("%H:%M:%S"), cmd));
                }

                lines.join("\n")
            }
        }

        "ai" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output(
                    "Usage:
  terrek ai setup
  terrek ai change-key
  terrek ai show-config
  terrek ai remove-key
  terrek ai <your question>"
                        .to_string(),
                ));
            }

            match parts[1] {
                "setup" | "change-key" => {
                    ai_setup()?;

                    if let Some(cfg) = load_config() {
                        let provider = capitalize(&cfg.provider);
                        return Ok(TerrekAction::Output(format!(
                            "{} API key saved successfully.",
                            provider
                        )));
                    }

                    return Ok(TerrekAction::Output(
                        "API key saved successfully.".into(),
                    ));
                }

                "show-config" => {
                    if let Some(cfg) = load_config() {
                        let preview = &cfg.api_key[..6.min(cfg.api_key.len())];
                        let provider = capitalize(&cfg.provider);

                        return Ok(TerrekAction::Output(format!(
                            "Provider: {}\nKey: {}****",
                            provider, preview
                        )));
                    } else {
                        return Ok(TerrekAction::Output("No API key configured.".into()));
                    }
                }

                "remove-key" => {
                    delete_config()?;
                    return Ok(TerrekAction::Output(
                        "API key removed successfully.".into(),
                    ));
                }

                _ => {
                    let prompt = parts[1..].join(" ");

                    let cfg = match load_config() {
                        Some(c) => c,
                        None => {
                            return Ok(TerrekAction::Output(
                                "No config found. Run `terrek ai setup` first.".into(),
                            ))
                        }
                    };

                    let provider = Provider::from_str(&cfg.provider);

                    let reply = match provider {
                        Provider::Gemini => {
                            match ask_gemini(&context, &prompt) {
                                Ok(r) => r,
                                Err(e) => return Ok(TerrekAction::Output(format!("{}", e))),
                            }
                        }

                        Provider::OpenAI => {
                            "[OpenAI integration coming soon 🚧]".to_string()
                        }

                        Provider::Claude => {
                            "[Claude integration coming soon 🚧]".to_string()
                        }

                        Provider::Ollama => {
                            "[Ollama integration coming soon 🚧]".to_string()
                        }
                    };

                    return Ok(TerrekAction::Output(reply));
                }
            }
        }

        "help" => r#"Terrek Commands:
  terrek hello
  terrek time
  terrek clear
  terrek history
  terrek last
  terrek ai
  terrek search <keyword>"#
            .to_string(),

        _ => "Unknown Terrek command".to_string(),
    };

    Ok(TerrekAction::Output(output))
}