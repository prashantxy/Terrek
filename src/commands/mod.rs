// src/terrek/mod.rs   (or wherever your command handler lives)

use crate::ai::gemini::ask_gemini;
use crate::ai::setup::setup as ai_setup;
use crate::ai::provider::Provider;
use crate::context::ContextState;
use crate::db::history::{get_history, search_history};
use crate::config::{load_config};
use anyhow::Result;
use chrono::{Local, TimeZone};
use std::process::Command;
use std::fs;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

pub enum TerrekAction {
    Output(String),
    // You can later add variants like ClearScreen, SplitPane, etc.
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// ===================== APP FINDER =====================
fn find_app(input: &str) -> Option<String> {
    let matcher = SkimMatcherV2::default();
    let paths = ["/Applications", "/System/Applications"];

    let mut best_match = None;
    let mut best_score = i64::MIN;

    for dir in paths {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    let clean = name.replace(".app", "").to_lowercase();
                    if let Some(score) = matcher.fuzzy_match(&clean, input) {
                        if score > best_score {
                            best_score = score;
                            best_match = Some(name.replace(".app", ""));
                        }
                    }
                }
            }
        }
    }
    best_match
}

// ===================== URL RESOLVER =====================
fn resolve_url(input: &str) -> String {
    let input = input.trim().to_lowercase();
    let words: Vec<&str> = input.split_whitespace().collect();

    if words.is_empty() {
        return "https://www.google.com".to_string();
    }

    if input.starts_with("http://") || input.starts_with("https://") {
        return input;
    }

    if input.contains('.') && !input.contains(' ') {
        return format!("https://{}", input);
    }

    let first = words[0];
    let query = words[1..].join("+");

    match first {
        "youtube" | "yt" => {
            if query.is_empty() { "https://www.youtube.com".to_string() }
            else { format!("https://www.youtube.com/results?search_query={}", query) }
        }
        "github" | "gh" => {
            if query.is_empty() { "https://github.com".to_string() }
            else if words.len() == 2 { format!("https://github.com/{}", words[1]) }
            else { format!("https://github.com/search?q={}", query) }
        }
        "stackoverflow" | "so" => format!("https://stackoverflow.com/search?q={}", input.replace(" ", "+")),
        "reddit" => format!("https://www.reddit.com/search/?q={}", query),
        "leetcode" | "lc" => {
            if query.is_empty() { "https://leetcode.com".to_string() }
            else { format!("https://leetcode.com/problemset/?search={}", query) }
        }
        "gfg" => format!("https://www.geeksforgeeks.org/?s={}", query),
        _ => {
            if !input.contains(' ') {
                format!("https://{}.com", input)
            } else {
                format!("https://www.google.com/search?q={}", input.replace(" ", "+"))
            }
        }
    }
}

// ===================== MARKETING HELPERS =====================

fn post_to_x(message: &str, image_path: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("xmaster");
    cmd.arg("post").arg(message);

    if let Some(img) = image_path {
        cmd.arg("--image").arg(img);
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("X posting failed. Run `cargo install xmaster` and `xmaster auth` first.");
    }
    Ok(())
}

fn generate_announce_message() -> Result<String> {
    let output = Command::new("git")
        .args(["log", "--oneline", "-1"])
        .output()?;

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if commit.is_empty() {
        return Ok("🚀 Terrek just got even better!".to_string());
    }

    Ok(format!(
        "🚀 New update in Terrek!\n{}\n\nhttps://github.com/yourname/terrek",
        commit
    ))
}

fn take_tmux_screenshot() -> Result<Option<String>> {
    let path = format!("/tmp/terrek-screenshot-{}.png", Local::now().timestamp());

    let status = Command::new("screencapture")
        .args(["-x", "-u", "-w", &path])   // capture front window
        .status()?;

    if status.success() && fs::metadata(&path).is_ok() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

// ===================== MAIN HANDLER =====================

pub fn handle_command(context: &ContextState, cmd: &str) -> Result<TerrekAction> {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();

    if parts.is_empty() {
        return Ok(TerrekAction::Output(String::new()));
    }

    let output = match parts[0] {
        "hello" => "Hello from Terrek! 👋".to_string(),

        "time" => format!("Current time: {}", Local::now()),

        "clear" => "__CLEAR__".to_string(),

        // ===================== MARKETING =====================
        "post" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output("Usage: post \"Your message here\"".to_string()));
            }
            let message = parts[1..].join(" ");
            match post_to_x(&message, None) {
                Ok(_) => format!("✅ Posted to X!\n{}", message),
                Err(e) => format!("❌ Post failed: {}", e),
            }
        }

        "announce" => {
            let mut custom_msg = None;
            let mut with_screenshot = false;

            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "-m" | "--message" if i + 1 < parts.len() => {
                        custom_msg = Some(parts[i + 1..].join(" "));
                        break;
                    }
                    "-s" | "--screenshot" => with_screenshot = true,
                    _ => {}
                }
                i += 1;
            }

            let message = custom_msg.unwrap_or_else(|| generate_announce_message().unwrap_or_default());

            let result = if with_screenshot {
                match take_tmux_screenshot() {
                    Ok(Some(path)) => match post_to_x(&message, Some(&path)) {
                        Ok(_) => format!("✅ Announced with screenshot!\n{}", message),
                        Err(e) => format!("❌ Failed to post with screenshot: {}", e),
                    },
                    _ => match post_to_x(&message, None) {
                        Ok(_) => format!("✅ Announced (screenshot skipped)\n{}", message),
                        Err(e) => format!("❌ Failed to announce: {}", e),
                    },
                }
            } else {
                match post_to_x(&message, None) {
                    Ok(_) => format!("✅ Announced on X!\n{}", message),
                    Err(e) => format!("❌ Failed to announce: {}", e),
                }
            };
            result
        }

        // ===================== OPEN =====================
        "open" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output("Usage: open <app | url | search term>".to_string()));
            }
            let input = parts[1..].join(" ").to_lowercase();

            if let Some(app_name) = find_app(&input) {
                let _ = Command::new("open").arg("-a").arg(&app_name).spawn();
                return Ok(TerrekAction::Output(format!("Opened: {}", app_name)));
            }

            let url = resolve_url(&input);
            let _ = Command::new("open").arg(&url).spawn();
            format!("Opened: {}", url)
        }

        // ===================== HISTORY =====================
        "history" => {
            let history = get_history(10)?;
            let mut lines = Vec::new();
            for (c, _, ts) in history {
                let time = Local.timestamp_opt(ts, 0).unwrap();
                lines.push(format!("[{}] {}", time.format("%H:%M:%S"), c));
            }
            lines.join("\n")
        }

        "last" => {
            let history = get_history(1)?;
            if let Some((c, out, ts)) = history.first() {
                let time = Local.timestamp_opt(*ts, 0).unwrap();
                format!("Last: {}\nTime: {}\nOutput:\n{}", c, time, out)
            } else {
                "No history yet.".to_string()
            }
        }

        "search" => {
            if parts.len() < 2 {
                "Usage: search <keyword>".to_string()
            } else {
                let results = search_history(parts[1])?;
                let mut lines = Vec::new();
                for (c, _, ts) in results {
                    let time = Local.timestamp_opt(ts, 0).unwrap();
                    lines.push(format!("[{}] {}", time.format("%H:%M:%S"), c));
                }
                lines.join("\n")
            }
        }

        // ===================== AI =====================
        "ai" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output("Usage: ai setup\nai <your question>".to_string()));
            }

            if parts[1] == "setup" || parts[1] == "change-key" {
                ai_setup()?;
                return Ok(TerrekAction::Output("AI setup completed.".into()));
            }

            let prompt = parts[1..].join(" ");
            let cfg = match load_config() {
                Some(c) => c,
                None => return Ok(TerrekAction::Output("Run `ai setup` first.".into())),
            };

            let reply = match Provider::from_str(&cfg.provider) {
                Provider::Gemini => ask_gemini(context, &prompt)?,
                _ => "Only Gemini is supported right now.".to_string(),
            };

            return Ok(TerrekAction::Output(reply));
        }

        // ===================== HELP =====================
        "help" | "?" => r#"Available commands inside Terrek:

  open <app-name | url | search>
  ai <question> | ai setup
  post "Your message"
  announce [-s] [-m "custom text"]
  history | last | search <word>
  time | hello | clear
  help"#
            .to_string(),

        _ => format!("Unknown command: '{}'. Type 'help' for available commands.", parts[0]),
    };

    Ok(TerrekAction::Output(output))
}