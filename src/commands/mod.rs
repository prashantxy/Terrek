use crate::ai::gemini::ask_gemini;
use crate::ai::setup::setup as ai_setup;
use crate::ai::provider::Provider;
use crate::context::ContextState;
use crate::db::history::{get_history, search_history};
use crate::config::{load_config, delete_config};
use anyhow::Result;
use chrono::{Local, TimeZone};
use std::process::Command;
use std::fs;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

pub enum TerrekAction {
    Output(String),
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

//
// 🔥 DYNAMIC APP FINDER (fuzzy + system scan)
//
fn find_app(input: &str) -> Option<String> {
    let matcher = SkimMatcherV2::default();
    let paths = ["/Applications", "/System/Applications"];

    let mut best_match = None;
    let mut best_score = i64::MIN;

    for dir in paths {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    let clean = name.replace(".app", "");

                    if let Some(score) = matcher.fuzzy_match(&clean.to_lowercase(), input) {
                        if score > best_score {
                            best_score = score;
                            best_match = Some(clean);
                        }
                    }
                }
            }
        }
    }

    best_match
}

//
// 🔥 INSANE URL RESOLVER
//
fn resolve_url(input: &str) -> String {
    let input = input.trim().to_lowercase();
    let words: Vec<&str> = input.split_whitespace().collect();

    if words.is_empty() {
        return "https://www.google.com".to_string();
    }

    // Full URL
    if input.starts_with("http://") || input.starts_with("https://") {
        return input;
    }

    // Domain like google.com
    if input.contains('.') && !input.contains(' ') {
        return format!("https://{}", input);
    }

    let first = words[0];
    let query = words[1..].join("+");

    match first {
        // YouTube
        "youtube" | "yt" => {
            if query.is_empty() {
                "https://www.youtube.com".to_string()
            } else {
                format!("https://www.youtube.com/results?search_query={}", query)
            }
        }

        // GitHub
        "github" | "gh" => {
            if query.is_empty() {
                "https://github.com".to_string()
            } else if words.len() == 2 {
                format!("https://github.com/{}", words[1])
            } else {
                format!("https://github.com/search?q={}", query)
            }
        }

        // StackOverflow
        "stackoverflow" | "so" => {
            format!("https://stackoverflow.com/search?q={}", input.replace(" ", "+"))
        }

        // Reddit
        "reddit" => {
            format!("https://www.reddit.com/search/?q={}", query)
        }

        // LeetCode
        "leetcode" | "lc" => {
            if query.is_empty() {
                "https://leetcode.com".to_string()
            } else {
                format!("https://leetcode.com/problemset/?search={}", query)
            }
        }

        // GFG
        "gfg" => {
            format!("https://www.geeksforgeeks.org/?s={}", query)
        }

        // Default
        _ => {
            if !input.contains(' ') {
                format!("https://{}.com", input)
            } else {
                format!(
                    "https://www.google.com/search?q={}",
                    input.replace(" ", "+")
                )
            }
        }
    }
}

pub fn handle_command(context: &ContextState, cmd: &str) -> Result<TerrekAction> {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();

    if parts.is_empty() {
        return Ok(TerrekAction::Output(String::new()));
    }

    let output = match parts[0] {
        "hello" => "Hello from Terrek! 👋".to_string(),

        "time" => format!("Current time: {}", Local::now()),

        "clear" => "__CLEAR__".to_string(),

        // ===================== OPEN =====================
        "open" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output(
                    "Usage: terrek open <app | url | search>".to_string()
                ));
            }

            let input = parts[1..].join(" ").to_lowercase();

            // 🔥 Try app
            if let Some(app_name) = find_app(&input) {
                let _ = Command::new("open")
                    .arg("-a")
                    .arg(&app_name)
                    .spawn();

                return Ok(TerrekAction::Output(format!("Opened app: {}", app_name)));
            }

            // 🔥 Fallback → URL
            let url = resolve_url(&input);

            let _ = Command::new("open")
                .arg(&url)
                .spawn();

            format!("Opened: {}", url)
        }

        // ===================== HISTORY =====================
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

        // ===================== AI =====================
        "ai" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output(
                    "Usage:\n  terrek ai setup\n  terrek ai <question>".to_string(),
                ));
            }

            match parts[1] {
                "setup" | "change-key" => {
                    ai_setup()?;
                    return Ok(TerrekAction::Output("API key saved successfully.".into()));
                }

                _ => {
                    let prompt = parts[1..].join(" ");

                    let cfg = match load_config() {
                        Some(c) => c,
                        None => {
                            return Ok(TerrekAction::Output(
                                "Run `terrek ai setup` first.".into(),
                            ))
                        }
                    };

                    let provider = Provider::from_str(&cfg.provider);

                    let reply = match provider {
                        Provider::Gemini => ask_gemini(context, &prompt)?,
                        _ => "Provider not implemented yet.".to_string(),
                    };

                    return Ok(TerrekAction::Output(reply));
                }
            }
        }

        // ===================== HELP =====================
        "help" => r#"Terrek Commands:
  terrek open <anything>
  terrek ai <question>
  terrek history
  terrek help"#
            .to_string(),

        _ => format!("Unknown command: '{}'", parts[0]),
    };

    Ok(TerrekAction::Output(output))
}