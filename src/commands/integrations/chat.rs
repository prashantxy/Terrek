use anyhow::{bail, Result};
use serde_json::json;

use super::env_var;
use crate::ai::check_status;

fn client() -> Result<reqwest::blocking::Client> {
    crate::ai::http_client()
}

pub fn slack(text: &str, channel: Option<&str>) -> Result<String> {
    let token = env_var(
        "TERREK_SLACK_BOT_TOKEN",
        "Create a Slack app with chat:write and export its xoxb- token.",
    )?;
    let channel = channel.unwrap_or("#general");
    let response = client()?
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(token)
        .json(&json!({ "channel": channel, "text": text }))
        .send()?;
    let body: serde_json::Value = check_status("Slack", response)?.json()?;
    // Slack reports most failures as HTTP 200 with ok=false.
    if body["ok"] != true {
        bail!(
            "Slack error: {}",
            body["error"].as_str().unwrap_or("unknown")
        );
    }
    Ok(format!("Sent to Slack {channel}"))
}

pub fn discord(text: &str, channel: Option<&str>) -> Result<String> {
    let token = env_var("TERREK_DISCORD_TOKEN", "Export your Discord bot token.")?;
    let channel = match channel {
        Some(c) => c.to_string(),
        None => env_var(
            "TERREK_DISCORD_CHANNEL_ID",
            "Pass --channel <id> or export a default channel ID.",
        )?,
    };
    let response = client()?
        .post(format!(
            "https://discord.com/api/v10/channels/{channel}/messages"
        ))
        .header("Authorization", format!("Bot {token}"))
        .json(&json!({ "content": text }))
        .send()?;
    check_status("Discord", response)?;
    Ok(format!("Sent to Discord channel {channel}"))
}

fn telegram_config() -> Result<(String, String)> {
    let token = env_var(
        "TERREK_TELEGRAM_TOKEN",
        "Create a bot with @BotFather and export its token.",
    )?;
    let chat_id = env_var(
        "TERREK_TELEGRAM_CHAT_ID",
        "Message your bot, then read the chat id from https://api.telegram.org/bot<token>/getUpdates.",
    )?;
    Ok((token, chat_id))
}

pub fn telegram(text: &str) -> Result<String> {
    let (token, chat_id) = telegram_config()?;
    let response = client()?
        .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
        .form(&[("chat_id", chat_id.as_str()), ("text", text)])
        .send()?;
    check_status("Telegram", response)?;
    Ok("Sent to Telegram".into())
}

pub fn telegram_photo(path: &str, caption: Option<&str>) -> Result<String> {
    let (token, chat_id) = telegram_config()?;
    if !std::path::Path::new(path).is_file() {
        bail!("photo not found: {path}");
    }
    let mut form = reqwest::blocking::multipart::Form::new().text("chat_id", chat_id);
    if let Some(caption) = caption {
        form = form.text("caption", caption.to_string());
    }
    let form = form.file("photo", path)?;
    let response = client()?
        .post(format!("https://api.telegram.org/bot{token}/sendPhoto"))
        .multipart(form)
        .send()?;
    check_status("Telegram", response)?;
    Ok("Photo sent to Telegram".into())
}

/// `7972655677` -> `917972655677@s.whatsapp.net` (default country code from TERREK_WA_COUNTRY_CODE, else 91).
pub fn whatsapp_jid(number: &str) -> String {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    let country = std::env::var("TERREK_WA_COUNTRY_CODE").unwrap_or_else(|_| "91".into());
    let full = if digits.len() == 10 && !number.trim().starts_with('+') {
        format!("{country}{digits}")
    } else {
        digits
    };
    format!("{full}@s.whatsapp.net")
}

pub fn whatsapp(number: &str, text: &str) -> Result<String> {
    let url = std::env::var("TERREK_WHATSAPP_BRIDGE_URL")
        .unwrap_or_else(|_| "http://localhost:3000/send".into());
    let jid = whatsapp_jid(number);
    let response = client()?
        .post(&url)
        .json(&json!({ "to": jid, "message": text }))
        .send()
        .map_err(|e| {
            anyhow::anyhow!("could not reach the WhatsApp bridge at {url} ({e}). Start it with `npm start` in bridges/whatsapp.")
        })?;
    check_status("WhatsApp bridge", response)?;
    Ok(format!("Sent WhatsApp message to {number}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jid_formatting() {
        std::env::remove_var("TERREK_WA_COUNTRY_CODE");
        assert_eq!(whatsapp_jid("7972655677"), "917972655677@s.whatsapp.net");
        assert_eq!(
            whatsapp_jid("+1 415 555 0100"),
            "14155550100@s.whatsapp.net"
        );
        assert_eq!(
            whatsapp_jid("+44 7700 900123"),
            "447700900123@s.whatsapp.net"
        );
    }
}
