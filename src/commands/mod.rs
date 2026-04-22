use crate::ai::gemini::ask_gemini;
use crate::ai::setup::setup as ai_setup;
use crate::ai::provider::Provider;
use crate::context::ContextState;
use crate::db::history::{get_history, search_history};
use crate::config::load_config;
use roux::Reddit;
use anyhow::{Context, Result};
use chrono::{Local, TimeZone};
use std::process::Command;
use std::fs;
use std::io::{self, Write};

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

// Email
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        PoolConfig,
    },
    Message, SmtpTransport, Transport,
};

// Telegram
use reqwest;

// For X fallback
use urlencoding;

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

// ===================== WHATSAPP HELPER FUNCTIONS =====================

// ===================== WHATSAPP =====================

fn format_number_to_jid(number: &str) -> String {
    let num = number.trim().replace("+", "").replace(" ", "");
    if num.starts_with("91") && num.len() == 12 {
        format!("{}@s.whatsapp.net", num)
    } else if num.len() == 10 {
        format!("91{}@s.whatsapp.net", num)
    } else {
        format!("{}@s.whatsapp.net", num)
    }
}

fn send_whatsapp_via_bridge(jid: &str, message: &str) -> Result<()> {
    println!("📱 Sending WhatsApp message to {}...", jid.replace("@s.whatsapp.net", ""));

    let root = std::env::current_dir().context("Failed to get project root")?;

    let candidates = vec![
        root.join("src/commands/whatsapp.ts"),
        root.join("src/commands/whatsapp.js"),
        root.join("commands/whatsapp.ts"),
        root.join("commands/whatsapp.js"),
    ];

    let whatsapp_file = candidates.into_iter()
        .find(|p| p.exists())
        .context("WhatsApp file not found at src/commands/whatsapp.ts")?;

    println!("Using WhatsApp file: {}", whatsapp_file.display());

    let status = Command::new("npx")
        .arg("tsx")
        .arg("-e")
        .arg(format!(
            r#"
            const whatsapp = require('{}');
            whatsapp.sendWhatsAppMessage('{}', {{ text: `{}` }})
                .then(() => {{
                    console.log('Bridge: Message sent successfully');
                    process.exit(0);
                }})
                .catch(err => {{
                    console.error('Bridge Error:', err.message);
                    process.exit(1);
                }});
            "#,
            whatsapp_file.to_string_lossy().replace("\\", "\\\\"),
            jid,
            message.replace("`", "\\`").replace("\\", "\\\\")
        ))
        .current_dir(&root)
        .status()
        .context("Failed to run tsx bridge")?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("WhatsApp send failed. Check Node logs for details.")
    }
}
// ===================== REDDIT =====================

async fn post_to_reddit(subreddit: &str, title: &str, body: &str) -> Result<()> {
    let client_id = std::env::var("REDDIT_CLIENT_ID")
        .context("REDDIT_CLIENT_ID not set.\nCreate a 'script' app at https://www.reddit.com/prefs/apps")?;

    let client_secret = std::env::var("REDDIT_CLIENT_SECRET")
        .context("REDDIT_CLIENT_SECRET not set")?;

    let username = std::env::var("REDDIT_USERNAME")
        .context("REDDIT_USERNAME not set")?;

    let password = std::env::var("REDDIT_PASSWORD")
        .context("REDDIT_PASSWORD not set")?;

    let reddit = Reddit::new(
        "terrek-cli/0.1 (by /u/your_reddit_username)", 
        &client_id,
        &client_secret,
    )
    .username(&username)
    .password(&password)
    .login()
    .await
    .context("Failed to login to Reddit. Check your credentials.")?;

    reddit.submit_text(title, body, subreddit)
        .await
        .context("Failed to submit post to Reddit")?;

    println!(" Successfully posted to r/{}!", subreddit);
    println!("   Title: {}", title);
    Ok(())
}

fn open_reddit_fallback(subreddit: &str, title: &str, body: &str) -> Result<()> {
    let encoded_title = urlencoding::encode(title);
    let encoded_body = urlencoding::encode(body);

    let url = format!(
        "https://www.reddit.com/r/{}/submit?title={}&text={}",
        subreddit, encoded_title, encoded_body
    );

    println!("🌐 Opening Reddit submit page...");

    let status = Command::new("open").arg(&url).status();

    if status.is_ok() && status.unwrap().success() {
        println!("✅ Browser opened. You can post manually.");
    } else {
        let _ = Command::new("sh")
            .arg("-c")
            .arg(format!("open '{}'", url))
            .status();
    }

    Ok(())
}

// ===================== EMAIL =====================

fn load_email_config() -> Result<EmailConfig> {
    Ok(EmailConfig {
        from_email: std::env::var("TERREK_EMAIL_FROM")
            .context("TERREK_EMAIL_FROM is not set.")?,
        smtp_host: std::env::var("TERREK_SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string()),
        smtp_port: std::env::var("TERREK_SMTP_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()
            .unwrap_or(587),
        username: std::env::var("TERREK_SMTP_USER")
            .context("TERREK_SMTP_USER is not set")?,
        password: std::env::var("TERREK_SMTP_PASS")
            .context("TERREK_SMTP_PASS is not set. Use 16-char Gmail App Password (no spaces)")?,
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
        .context("Failed to build email")?;

    let creds = Credentials::new(cfg.username.clone(), cfg.password.clone());

    let mailer = SmtpTransport::starttls_relay(&cfg.smtp_host)?
        .port(cfg.smtp_port)
        .credentials(creds)
        .authentication(vec![Mechanism::Plain])
        .pool_config(PoolConfig::new().max_size(5))
        .build();

    match mailer.send(&email) {
        Ok(_) => {
            println!(" Email sent successfully to {}", to);
            Ok(())
        }
        Err(e) => {
            println!(" Failed to send email: {}", e);
            if e.to_string().contains("535") || e.to_string().contains("authentication") {
                println!("\n💡 TIP: Regenerate your Gmail App Password at https://myaccount.google.com/apppasswords");
            }
            Err(e.into())
        }
    }
}

// ===================== TELEGRAM =====================

fn get_telegram_config() -> Result<(String, String)> {
    let token = std::env::var("TERREK_TELEGRAM_TOKEN")
        .context("TERREK_TELEGRAM_TOKEN not set")?;
    
    let chat_id = std::env::var("TERREK_TELEGRAM_CHAT_ID")
        .context("TERREK_TELEGRAM_CHAT_ID not set. Run: terrek telegram setup")?;

    Ok((token, chat_id))
}

fn send_telegram_message(text: &str) -> Result<()> {
    let (token, chat_id) = get_telegram_config()?;

    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);

    let client = reqwest::blocking::Client::new();
    
    let response = client.post(&url)
        .form(&[
            ("chat_id", chat_id.as_str()),
            ("text", text),
            ("parse_mode", "HTML"),
        ])
        .send()?;

    if response.status().is_success() {
        println!(" Telegram message sent successfully!");
        Ok(())
    } else {
        let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("Telegram API Error: {}", error_text)
    }
}

fn send_telegram_photo(photo_path: &str, caption: Option<&str>) -> Result<()> {
    let (token, chat_id) = get_telegram_config()?;

    if !std::path::Path::new(photo_path).exists() {
        anyhow::bail!("Photo file not found: {}", photo_path);
    }

    let url = format!("https://api.telegram.org/bot{}/sendPhoto", token);

    let client = reqwest::blocking::Client::new();
    let mut form = reqwest::blocking::multipart::Form::new()
        .text("chat_id", chat_id.clone());

    if let Some(c) = caption {
        form = form.text("caption", c.to_string());
    }

    form = form.file("photo", photo_path)?;

    let response = client.post(&url)
        .multipart(form)
        .send()?;

    if response.status().is_success() {
        println!(" Photo sent to Telegram!");
        Ok(())
    } else {
        let err = response.text().unwrap_or_default();
        anyhow::bail!("Failed to send photo: {}", err)
    }
}

fn setup_telegram() -> Result<()> {
    println!("📱 Telegram Bot Setup:\n");
    println!("1. Open Telegram → Search @BotFather");
    println!("2. Send /newbot and create your bot");
    println!("3. Copy the token and set it:");
    println!("   export TERREK_TELEGRAM_TOKEN=\"your_token_here\"");
    println!("\n4. Message your bot with any text");
    println!("5. Then run: terrek telegram setup");
    Ok(())
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
        "youtube" | "yt" => if query.is_empty() { "https://www.youtube.com".to_string() } else { format!("https://www.youtube.com/results?search_query={}", query) },
        "github" | "gh" => if query.is_empty() { "https://github.com".to_string() } else if words.len() == 2 { format!("https://github.com/{}", words[1]) } else { format!("https://github.com/search?q={}", query) },
        "stackoverflow" | "so" => format!("https://stackoverflow.com/search?q={}", input.replace(" ", "+")),
        "reddit" => format!("https://www.reddit.com/search/?q={}", query),
        "leetcode" | "lc" => if query.is_empty() { "https://leetcode.com".to_string() } else { format!("https://leetcode.com/problemset/?search={}", query) },
        "gfg" => format!("https://www.geeksforgeeks.org/?s={}", query),
        _ => if !input.contains(' ') { format!("https://{}.com", input) } else { format!("https://www.google.com/search?q={}", input.replace(" ", "+")) },
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
            println!(" Posted to X via API");
            Ok(PostResult::Api)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            println!(" X Post failed via API");

            if stderr.contains("402") || stderr.contains("payment") || stderr.contains("credits") {
                println!("💰 No credits remaining. Opening browser fallback...");
                open_intent_fallback(message)?;
                Ok(PostResult::Fallback)
            } else {
                println!("Error: {}", String::from_utf8_lossy(&output.stderr));
                anyhow::bail!("X post failed")
            }
        }
        Err(_) => {
            println!(" xmaster not found. Opening browser fallback...");
            open_intent_fallback(message)?;
            Ok(PostResult::Fallback)
        }
    }
}

fn open_intent_fallback(message: &str) -> Result<()> {
    let encoded = urlencoding::encode(message);
    let url = format!("https://twitter.com/intent/tweet?text={}", encoded);

    println!("🌐 Opening X compose window in browser...");

    let status = Command::new("open").arg(&url).status();

    if status.is_ok() && status.unwrap().success() {
        println!("✅ Browser opened. You can now post manually.");
        Ok(())
    } else {
        let _ = Command::new("sh")
            .arg("-c")
            .arg(format!("open '{}'", url))
            .status();
        println!("✅ Browser should now be open.");
        Ok(())
    }
}

// ===================== EMAIL HELPERS =====================

fn compose_manual_email() -> Result<String> {
    let tmp_path = "/tmp/terrek-email-manual.txt";

    let template = "\
# Write your email body below.
# Lines starting with '#' will be ignored.
# Save and close the editor when finished.\n\n";

    fs::write(tmp_path, template)?;

    println!("📧 Opening editor to compose your email...");

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let status = Command::new(&editor).arg(tmp_path).status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with error"));
    }

    let content = fs::read_to_string(tmp_path)?;
    let _ = fs::remove_file(tmp_path);

    let body: String = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(body.trim().to_string())
}

fn ask_for_email_context() -> Result<String> {
    let tmp_path = "/tmp/terrek-email-context.txt";

    let template = "\
# Describe the email you want Gemini to write.
# Include tone, key points, purpose, recipient details, etc.
# Be detailed for better results.
# Save and close the editor when done.\n\n";

    fs::write(tmp_path, template)?;

    println!("🤖 Opening editor to describe the email for Gemini...");

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let status = Command::new(&editor).arg(tmp_path).status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with error"));
    }

    let content = fs::read_to_string(tmp_path)?;
    let _ = fs::remove_file(tmp_path);

    let context: String = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

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
                Ok(PostResult::Api) => "🚀 Posted to X via API".to_string(),
                Ok(PostResult::Fallback) => "🌐 Browser opened for manual posting on X".to_string(),
                Err(e) => format!(" Post failed: {}", e),
            }
        }

        "reddit" | "r" => {
            if parts.len() < 4 {
                return Ok(TerrekAction::Output("Usage: reddit <sub> \"title\" \"body\"".into()));
            }

            let subreddit = parts[1];
            let title = parts[2];
            let body = parts[3..].join(" ");

            match open_reddit_fallback(subreddit, title, &body) {
                Ok(_) => "🌐 Opened Reddit post page".to_string(),
                Err(e) => format!(" Failed: {}", e),
            }
        }

        "announce" => {
            let message = generate_announce_message().unwrap_or_default();
            match post_to_x(&message, None) {
                Ok(PostResult::Api) => "Announced via API".to_string(),
                Ok(PostResult::Fallback) => "Opened browser for announcement".to_string(),
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
                        "Email body was empty. Cancelled.".to_string()
                    } else {
                        match send_email_native(&to, &subject, &body) {
                            Ok(_) => format!("📧 Manual email sent to {}", to),
                            Err(e) => format!(" {}", e),
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
                            Err(e) => format!(" {}", e),
                        },
                        "e" | "edit" => {
                            let tmp_path = "/tmp/terrek-email-draft.txt";
                            fs::write(tmp_path, &body)?;

                            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
                            let _ = Command::new(editor).arg(tmp_path).status();

                            let edited = fs::read_to_string(tmp_path)?;
                            let _ = fs::remove_file(tmp_path);

                            match send_email_native(&to, &subject, edited.trim()) {
                                Ok(_) => format!(" Edited email sent to {}", to),
                                Err(e) => format!(" {}", e),
                            }
                        }
                        _ => "Cancelled.".to_string(),
                    }
                }

                _ => "Unknown mode. Use: mail manual or mail ai".to_string(),
            }
        }

        // ===================== WHATSAPP =====================
                // ===================== WHATSAPP =====================
               "whatsapp" | "wa" => {
            if parts.len() < 3 {
                return Ok(TerrekAction::Output(
                    "Usage:\n  wa <number> <message>\n  whatsapp <number> <message>\n\nExample:\n  wa 7972655677 hey there".to_string()
                ));
            }

            let raw_number = parts[1];
            let message_text = parts[2..].join(" ");

            let jid = format_number_to_jid(raw_number);

            match send_whatsapp_via_bridge(&jid, &message_text) {
                Ok(_) => format!("✅ WhatsApp message sent to {}", raw_number),
                Err(e) => format!("❌ {}", e),
            }
        }
        
        "help" => {
            "Available commands:\n\
             hello, time, clear\n\
             open <app|url>\n\
             post \"message\" (to X)\n\
             reddit <subreddit> \"title\" \"body\"   (or r ...)\n\
             announce\n\
             ai setup | ai <prompt>\n\
             mail manual <to> [subject]\n\
             mail ai <to> [subject]\n\
             telegram <message> | tg \"message\"\n\
             telegram setup\n\
             telegram photo <file> [caption]\n\
             whatsapp <number> <message>   (or wa ...)\n\
             history, search <word>".to_string()
        }

        _ => format!("Unknown command: {}", parts[0]),
    };

    Ok(TerrekAction::Output(output))
}