use crate::ai::gemini::ask_gemini;
use crate::ai::setup::setup as ai_setup;
use crate::ai::provider::Provider;
use crate::context::ContextState;
use crate::db::history::{get_history, search_history};
use crate::config::load_config;

use anyhow::{Context, Result};
use chrono::{Local, TimeZone};
use std::process::Command;
use std::fs;
use std::io::{self, Read, Write};   // ← FIXED: Added `Read`

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

// Lettre imports
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        PoolConfig,
    },
    Message, SmtpTransport, Transport,
};

pub enum TerrekAction {
    Output(String),
}

#[derive(Debug)]
enum PostResult {
    Api,
    Fallback,
}

#[derive(Debug)]
struct EmailConfig {
    from_email: String,
    smtp_host: String,
    smtp_port: u16,
    username: String,
    password: String,
}

fn load_email_config() -> Result<EmailConfig> {
    Ok(EmailConfig {
        from_email: std::env::var("TERREK_EMAIL_FROM")
            .context("TERREK_EMAIL_FROM is not set. Example: export TERREK_EMAIL_FROM='pdubey1924@gmail.com'")?,
        smtp_host: std::env::var("TERREK_SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string()),
        smtp_port: std::env::var("TERREK_SMTP_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()
            .unwrap_or(587),
        username: std::env::var("TERREK_SMTP_USER")
            .context("TERREK_SMTP_USER is not set")?,
        password: std::env::var("TERREK_SMTP_PASS")
            .context("TERREK_SMTP_PASS is not set. Use your 16-character Gmail App Password (no spaces)")?,
    })
}

fn send_email_native(to: &str, subject: &str, body: &str) -> Result<()> {
    let cfg = load_email_config()?;

    let email = Message::builder()
        .from(cfg.from_email.parse::<Mailbox>().context("Invalid from email")?)
        .to(to.parse::<Mailbox>().context("Invalid to email")?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .context("Failed to build email message")?;

    let creds = Credentials::new(cfg.username.clone(), cfg.password.clone());

    let mailer = SmtpTransport::starttls_relay(&cfg.smtp_host)
        .context("Failed to create SMTP transport")?
        .port(cfg.smtp_port)
        .credentials(creds)
        .authentication(vec![Mechanism::Plain])
        .pool_config(PoolConfig::new().max_size(5))
        .build();

    match mailer.send(&email) {
        Ok(_) => {
            println!("✅ Email sent successfully to {}", to);
            Ok(())
        }
        Err(e) => {
            let err_str = e.to_string().to_lowercase();
            println!("❌ Failed to send email: {}", e);
            if err_str.contains("535") || err_str.contains("authentication") {
                println!("\n💡 TIP: Your App Password is likely incorrect or has spaces.");
                println!("   Go to: https://myaccount.google.com/apppasswords");
                println!("   Create a new one for 'Terrek CLI' and set it again.");
            }
            Err(e.into())
        }
    }
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
            if err.contains("rate limit") || err.contains("429") {
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
    let url = format!("https://twitter.com/intent/tweet?text={}", encoded);
    let _ = Command::new("open").arg(&url).spawn();
    println!("🌐 Opened browser fallback");
    Ok(())
}

// ===================== EMAIL HELPERS =====================

fn compose_manual_email() -> Result<String> {
    println!("📧 Compose your email (multi-line supported). Press Ctrl+D when done:\n");
    let mut body = String::new();
    io::stdin().read_to_string(&mut body)?;   // Now works because Read is imported
    Ok(body.trim().to_string())
}

fn ask_for_email_context() -> Result<String> {
    println!("🤖 Describe the email you want Gemini to write (be detailed):");
    let mut context = String::new();
    io::stdin().read_to_string(&mut context)?;   // Fixed here too
    Ok(context.trim().to_string())
}

fn generate_email_with_gemini(
    context: &ContextState,
    user_context: &str,
    to: &str,
    subject: &str,
) -> Result<String> {
    let prompt = format!(
        "Write a professional, natural, and polite email.\n\n\
        To: {to}\nSubject: {subject}\n\nContext: {user_context}\n\n\
        Return ONLY the email body. No subject, no explanations.",
        to = to,
        subject = subject,
        user_context = user_context
    );
    ask_gemini(context, &prompt)
}

fn generate_announce_message() -> Result<String> {
    let output = Command::new("git")
        .args(["log", "--oneline", "-1"])
        .output()?;

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if commit.is_empty() {
        return Ok("Terrek just got even better!".to_string());
    }

    Ok(format!(
        "New update in Terrek!\n{}\n\nhttps://github.com/yourname/terrek",
        commit
    ))
}

// ===================== MAIN COMMAND HANDLER =====================

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
                Ok(PostResult::Api) => format!("🚀 Posted via API\n{}", message),
                Ok(PostResult::Fallback) => format!("🌐 Opened in browser\n{}", message),
                Err(e) => format!("❌ Post failed: {}", e),
            }
        }

        "announce" => {
            let message = generate_announce_message().unwrap_or_default();
            match post_to_x(&message, None) {
                Ok(PostResult::Api) => format!("Announced via API\n{}", message),
                Ok(PostResult::Fallback) => format!("Opened browser for announcement\n{}", message),
                Err(e) => format!("Failed to announce: {}", e),
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
                return Ok(TerrekAction::Output("Usage: ai setup | ai <prompt>".to_string()));
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
                _ => "Only Gemini supported currently.".to_string(),
            };
            return Ok(TerrekAction::Output(reply));
        }

        "mail" => {
            if parts.len() < 3 {
                return Ok(TerrekAction::Output(
                    "Usage:\n  mail manual <to> [subject]\n  mail ai <to> [subject]".to_string(),
                ));
            }

            let mode = parts[1];
            let to = parts[2].to_string();
            let subject = if parts.len() > 3 {
                parts[3..].join(" ")
            } else {
                "No Subject".to_string()
            };

            match mode {
                "manual" => {
                    let body = compose_manual_email()?;
                    if body.is_empty() {
                        "Email body empty. Cancelled.".to_string()
                    } else {
                        match send_email_native(&to, &subject, &body) {
                            Ok(_) => format!("📧 Manual email sent to {}", to),
                            Err(e) => format!("❌ {}", e),
                        }
                    }
                }

                "ai" => {
                    let user_context = ask_for_email_context()?;
                    if user_context.is_empty() {
                        return Ok(TerrekAction::Output("Context cannot be empty.".to_string()));
                    }

                    println!("🤖 Generating email with Gemini...");
                    let body = generate_email_with_gemini(context, &user_context, &to, &subject)?;

                    println!("\n=== AI Generated Email ===\n{}\n==========================\n", body);

                    print!("Send this? (y / n / edit): ");
                    io::stdout().flush()?;

                    let mut choice = String::new();
                    io::stdin().read_line(&mut choice)?;
                    let choice = choice.trim().to_lowercase();

                    match choice.as_str() {
                        "y" | "yes" => match send_email_native(&to, &subject, &body) {
                            Ok(_) => format!("📧 AI email sent to {}", to),
                            Err(e) => format!("❌ {}", e),
                        },
                        "e" | "edit" => {
                            let tmp_path = "/tmp/terrek-email-draft.txt";
                            fs::write(tmp_path, &body)?;

                            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
                            let _ = Command::new(editor).arg(tmp_path).status();

                            let edited = fs::read_to_string(tmp_path)?;
                            let _ = fs::remove_file(tmp_path);

                            match send_email_native(&to, &subject, edited.trim()) {
                                Ok(_) => format!("📧 Edited email sent to {}", to),
                                Err(e) => format!("❌ {}", e),
                            }
                        }
                        _ => "Cancelled.".to_string(),
                    }
                }

                _ => "Unknown mode. Use: mail manual or mail ai".to_string(),
            }
        }

        "help" => {
            "Available commands:\n\
             hello, time, clear\n\
             open <app|url>\n\
             post \"message\"\n\
             announce\n\
             ai setup | ai <prompt>\n\
             mail manual <to> [subject]\n\
             mail ai <to> [subject]\n\
             history, search <word>".to_string()
        }

        _ => format!("Unknown command: {}", parts[0]),
    };

    Ok(TerrekAction::Output(output))
}