//! `terrek send <target>`: push a message somewhere from the terminal.
//! Credentials come from environment variables (see `docs/integrations.md`).

mod chat;
mod email;
mod social;

use anyhow::{Context, Result};
use clap::Subcommand;

use super::Env;

#[derive(Debug, Clone, Subcommand)]
pub enum SendTarget {
    /// Post to Slack (TERREK_SLACK_BOT_TOKEN)
    Slack {
        /// Channel name or ID [default: #general]
        #[arg(short, long)]
        channel: Option<String>,
        #[arg(required = true, trailing_var_arg = true)]
        message: Vec<String>,
    },
    /// Post to a Discord channel (TERREK_DISCORD_TOKEN, TERREK_DISCORD_CHANNEL_ID)
    Discord {
        #[arg(short, long)]
        channel: Option<String>,
        #[arg(required = true, trailing_var_arg = true)]
        message: Vec<String>,
    },
    /// Message your Telegram bot chat (TERREK_TELEGRAM_TOKEN, TERREK_TELEGRAM_CHAT_ID)
    Telegram {
        /// Send an image, with the message as its caption
        #[arg(long)]
        photo: Option<String>,
        #[arg(trailing_var_arg = true)]
        message: Vec<String>,
    },
    /// Send an email over SMTP; opens $EDITOR for the body (TERREK_SMTP_*)
    Email {
        to: String,
        #[arg(short, long, default_value = "")]
        subject: String,
        /// Describe the email and let the AI draft it
        #[arg(long)]
        ai: bool,
    },
    /// Post to X via the `xmaster` CLI, or open the compose page
    X {
        #[arg(required = true, trailing_var_arg = true)]
        message: Vec<String>,
    },
    /// Send a WhatsApp message through the local bridge (bridges/whatsapp)
    Whatsapp {
        number: String,
        #[arg(required = true, trailing_var_arg = true)]
        message: Vec<String>,
    },
    /// Open Reddit's submit page pre-filled
    Reddit {
        subreddit: String,
        title: String,
        #[arg(trailing_var_arg = true)]
        body: Vec<String>,
    },
}

pub fn is_interactive(target: &SendTarget) -> bool {
    matches!(target, SendTarget::Email { .. })
}

pub fn send(target: SendTarget, env: &Env) -> Result<String> {
    match target {
        SendTarget::Slack { channel, message } => {
            chat::slack(&message.join(" "), channel.as_deref())
        }
        SendTarget::Discord { channel, message } => {
            chat::discord(&message.join(" "), channel.as_deref())
        }
        SendTarget::Telegram { photo, message } => {
            let text = message.join(" ");
            match photo {
                Some(path) => {
                    chat::telegram_photo(&path, (!text.is_empty()).then_some(text.as_str()))
                }
                None if text.is_empty() => anyhow::bail!("nothing to send"),
                None => chat::telegram(&text),
            }
        }
        SendTarget::Whatsapp { number, message } => chat::whatsapp(&number, &message.join(" ")),
        SendTarget::Email { to, subject, ai } => email::send(env, &to, &subject, ai),
        SendTarget::X { message } => social::post_to_x(&message.join(" ")),
        SendTarget::Reddit {
            subreddit,
            title,
            body,
        } => social::reddit(&subreddit, &title, &body.join(" ")),
    }
}

pub(crate) fn env_var(name: &str, hint: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .with_context(|| format!("{name} is not set. {hint}"))
}
