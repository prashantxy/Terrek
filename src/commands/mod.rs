use crate::ai::gemini::ask_gemini;
use crate::ai::setup::setup as ai_setup;
use crate::ai::provider::Provider;
use crate::context::ContextState;
use crate::db::history::{get_history, search_history};
use crate::config::load_config;

use anyhow::Result;
use chrono::{Local, TimeZone};
use std::process::Command;
use std::fs;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;



pub enum TerrekAction {
    Output(String),
}

#[derive(Debug)]
enum PostResult {
    Api,
    Fallback,
}

// ===================== UTILS =====================

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

// ===================== X POSTING =====================

fn post_to_x(message: &str, image_path: Option<&str>) -> Result<PostResult> {
    let mut cmd = Command::new("xmaster");
    cmd.arg("post").arg(message);

    if let Some(img) = image_path {
        cmd.arg("--image").arg(img);
    }

    match cmd.output() {
        Ok(output) if output.status.success() => {
            println!("🚀 Posted via API");
            Ok(PostResult::Api)
        }

        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr).to_lowercase();

            if err.contains("rate limit")
                || err.contains("429")
                || err.contains("payment")
                || err.contains("unauthorized")
                || err.contains("forbidden")
            {
                println!("⚠️ API issue → fallback triggered");
                open_intent_fallback(message)?;
                Ok(PostResult::Fallback)
            } else {
                anyhow::bail!("Post failed: {}", err);
            }
        }

        Err(_) => {
            println!("⚠️ xmaster missing → fallback");
            open_intent_fallback(message)?;
            Ok(PostResult::Fallback)
        }
    }
}

fn open_intent_fallback(message: &str) -> Result<()> {
    let encoded = urlencoding::encode(message);

    let url = format!(
        "https://twitter.com/intent/tweet?text={}",
        encoded
    );

    Command::new("open").arg(&url).spawn()?;

    println!("🌐 Opened browser fallback");
    Ok(())
}

// ===================== HELPERS =====================

fn generate_announce_message() -> Result<String> {
    let output = Command::new("git")
        .args(["log", "--oneline", "-1"])
        .output()?;

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if commit.is_empty() {
        return Ok(" Terrek just got even better!".to_string());
    }

    Ok(format!(
        " New update in Terrek!\n{}\n\nhttps://github.com/yourname/terrek",
        commit
    ))
}

fn take_tmux_screenshot() -> Result<Option<String>> {
    let path = format!("/tmp/terrek-screenshot-{}.png", Local::now().timestamp());

    let status = Command::new("screencapture")
        .args(["-x", "-u", "-w", &path])
        .status()?;

    if status.success() && fs::metadata(&path).is_ok() {
        Ok(Some(path))
    } else {
        Ok(None)
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

       
        "post" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output("Usage: post \"message\"".to_string()));
            }

            let message = parts[1..].join(" ");

            match post_to_x(&message, None) {
                Ok(PostResult::Api) =>
                    format!("🚀 Posted via API\n{}", message),

                Ok(PostResult::Fallback) =>
                    format!("🌐 Opened in browser (manual post)\n{}", message),

                Err(e) =>
                    format!("❌ Post failed: {}", e),
            }
        }

       
        "announce" => {
            let message = generate_announce_message().unwrap_or_default();

            match post_to_x(&message, None) {
                Ok(PostResult::Api) =>
                    format!(" Announced via API\n{}", message),

                Ok(PostResult::Fallback) =>
                    format!(" Opened browser for announcement\n{}", message),

                Err(e) =>
                    format!(" Failed to announce: {}", e),
            }
        }

       
        "open" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output("Usage: open <app | url>".to_string()));
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

     
        "history" => {
            let history = get_history(10)?;
            let mut lines = Vec::new();

            for (c, _, ts) in history {
                let time = Local.timestamp_opt(ts, 0).unwrap();
                lines.push(format!("[{}] {}", time.format("%H:%M:%S"), c));
            }

            lines.join("\n")
        }

        "search" => {
            if parts.len() < 2 {
                "Usage: search <word>".to_string()
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

     
        "ai" => {
            if parts.len() < 2 {
                return Ok(TerrekAction::Output("Usage: ai setup | ai <q>".to_string()));
            }

            if parts[1] == "setup" {
                ai_setup()?;
                return Ok(TerrekAction::Output("AI setup done.".into()));
            }

            let prompt = parts[1..].join(" ");

            let cfg = match load_config() {
                Some(c) => c,
                None => return Ok(TerrekAction::Output("Run `ai setup` first.".into())),
            };

            let reply = match Provider::from_str(&cfg.provider) {
                Provider::Gemini => ask_gemini(context, &prompt)?,
                _ => "Only Gemini supported.".to_string(),
            };

            return Ok(TerrekAction::Output(reply));
        }

        "help" => "Commands: open, post, announce, ai, history, search".to_string(),

        _ => format!("Unknown command: {}", parts[0]),
    };

    Ok(TerrekAction::Output(output))
}