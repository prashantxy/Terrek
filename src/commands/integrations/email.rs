use anyhow::{bail, Context, Result};
use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::io::{self, BufRead, Write};
use std::process::Command;

use super::env_var;
use crate::commands::Env;

struct SmtpConfig {
    from: String,
    host: String,
    port: u16,
    username: String,
    password: String,
}

fn smtp_config() -> Result<SmtpConfig> {
    Ok(SmtpConfig {
        from: env_var("TERREK_EMAIL_FROM", "Set it to the sender address.")?,
        host: std::env::var("TERREK_SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".into()),
        port: std::env::var("TERREK_SMTP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(587),
        username: env_var("TERREK_SMTP_USER", "Set it to your SMTP username.")?,
        password: env_var(
            "TERREK_SMTP_PASS",
            "For Gmail, use an App Password (https://myaccount.google.com/apppasswords).",
        )?,
    })
}

/// Interactive: runs with the terminal in normal mode.
pub fn send(env: &Env, to: &str, subject: &str, ai: bool) -> Result<String> {
    let cfg = smtp_config()?;
    let to_mailbox: Mailbox = to
        .parse()
        .with_context(|| format!("invalid address: {to}"))?;
    let subject = if subject.is_empty() {
        "(no subject)"
    } else {
        subject
    };

    let body = if ai {
        let brief = edit_text(
            "# Describe the email: purpose, key points, tone, anything the recipient needs.\n\
             # Lines starting with # are ignored. Save and close the editor when done.\n\n",
        )?;
        if brief.is_empty() {
            bail!("no description given; nothing sent");
        }
        println!("Drafting with AI…");
        let model = crate::ai::from_config(&env.config.ai)?;
        let draft = model.complete(
            "You write clear, natural emails. Return only the email body: no subject line, \
             no placeholders like [Name] unless the details are missing, no commentary.",
            &format!("To: {to}\nSubject: {subject}\n\nWhat the email should say:\n{brief}"),
        )?;
        review(draft)?
    } else {
        edit_text("# Write the email body. Lines starting with # are ignored.\n\n")?
    };

    if body.trim().is_empty() {
        bail!("empty body; nothing sent");
    }

    let message = Message::builder()
        .from(cfg.from.parse().context("invalid TERREK_EMAIL_FROM")?)
        .to(to_mailbox)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)?;
    let mailer = SmtpTransport::starttls_relay(&cfg.host)?
        .port(cfg.port)
        .credentials(Credentials::new(cfg.username, cfg.password))
        .build();
    mailer.send(&message).map_err(|e| {
        let hint = if e.to_string().contains("535") {
            " (authentication failed; for Gmail use an App Password)"
        } else {
            ""
        };
        anyhow::anyhow!("sending failed: {e}{hint}")
    })?;
    Ok(format!("Email sent to {to}"))
}

/// Show the AI draft and let the user send, edit, or cancel.
fn review(draft: String) -> Result<String> {
    println!("\n──── draft ────\n{draft}\n───────────────");
    print!("Send it? [y]es / [e]dit / [n]o: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    match answer.trim().to_lowercase().as_str() {
        "y" | "yes" => Ok(draft),
        "e" | "edit" => edit_text(&draft),
        _ => bail!("cancelled; nothing sent"),
    }
}

/// Open $EDITOR on `initial` and return the saved text without `#` comment lines.
fn edit_text(initial: &str) -> Result<String> {
    let mut file = tempfile::Builder::new()
        .prefix("terrek-email-")
        .suffix(".txt")
        .tempfile()?;
    file.write_all(initial.as_bytes())?;
    file.flush()?;

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "nano".into());
    let mut parts = shell_words::split(&editor).unwrap_or_else(|_| vec![editor.clone()]);
    let program = parts.remove(0);
    let status = Command::new(&program)
        .args(parts)
        .arg(file.path())
        .status()
        .with_context(|| format!("could not start editor `{editor}`"))?;
    if !status.success() {
        bail!("editor exited with {status}");
    }

    let text = std::fs::read_to_string(file.path())?;
    Ok(text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string())
}
