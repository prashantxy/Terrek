//! `terrek setup`: pick a provider, paste a key (hidden), verify it works, save.

use anyhow::{bail, Result};
use std::io::{self, BufRead, Write};

use super::auto::{build, resolve};
use super::provider::ProviderKind;
use crate::config::{mask_secret, Config};

pub struct SetupArgs {
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

pub fn run(mut config: Config, args: SetupArgs) -> Result<String> {
    println!("Terrek setup\n");

    let provider = match args.provider {
        Some(p) => p,
        None => choose_provider(config.ai.provider)?,
    };

    let mut api_key = None;
    if provider.needs_key() {
        let env_hint = provider
            .key_env_vars()
            .iter()
            .find(|v| std::env::var(v).is_ok_and(|k| !k.is_empty()));
        let prompt = match env_hint {
            Some(var) => format!("API key (hidden; Enter to use ${var} from the environment): "),
            None => "API key (hidden): ".to_string(),
        };
        let key = rpassword::prompt_password(prompt)?;
        let key = key.replace("\u{1b}[200~", "").replace("\u{1b}[201~", "");
        let key = key.trim().to_string();
        if key.is_empty() && env_hint.is_none() {
            bail!("no key entered");
        }
        if let Some(detected) = ProviderKind::detect_from_key(&key) {
            if detected != provider {
                println!("Note: that key looks like a {detected} key, not {provider}.");
            }
        }
        // With an env var present we store nothing and keep reading it from the environment.
        api_key = (!key.is_empty()).then_some(key);
    }

    let model = match args.model {
        Some(m) => Some(m),
        None => prompt_default("Model", provider.default_model())?,
    };

    let base_url = match args.base_url {
        Some(url) => Some(url),
        None if matches!(provider, ProviderKind::OpenAi | ProviderKind::Ollama) => {
            let default = match provider {
                ProviderKind::Ollama => "http://localhost:11434",
                _ => "https://api.openai.com/v1",
            };
            prompt_default("Base URL", default)?
        }
        None => None,
    };

    config.ai.provider = Some(provider);
    config.ai.api_key = api_key;
    config.ai.model = model.filter(|m| m != provider.default_model());
    config.ai.base_url = base_url;

    print!("Checking the connection… ");
    io::stdout().flush()?;
    let check = resolve(&config.ai).and_then(build).and_then(|model| {
        model
            .complete("Reply with exactly: ok", "ping")
            .map(|_| model.label())
    });
    match check {
        Ok(label) => println!("ok ({label})"),
        Err(e) => {
            println!("failed\n  {e:#}");
            if !confirm("Save this configuration anyway?")? {
                bail!("setup cancelled; nothing was saved");
            }
        }
    }

    let path = config.save()?;
    let key_note = config
        .ai
        .api_key
        .as_deref()
        .map(|k| format!(", key {}", mask_secret(k)))
        .unwrap_or_default();
    Ok(format!(
        "Saved {provider}{key_note} to {} (readable only by you).",
        path.display()
    ))
}

fn choose_provider(current: Option<ProviderKind>) -> Result<ProviderKind> {
    println!("Which AI provider?");
    for (i, kind) in ProviderKind::ALL.iter().enumerate() {
        let marker = if Some(*kind) == current {
            "  (current)"
        } else {
            ""
        };
        println!("  {}) {}{marker}", i + 1, kind.display_name());
    }
    loop {
        let answer = read_line("> ")?;
        if let Some(kind) = answer
            .parse::<usize>()
            .ok()
            .and_then(|n| ProviderKind::ALL.get(n.wrapping_sub(1)).copied())
            .or_else(|| ProviderKind::parse(&answer))
        {
            return Ok(kind);
        }
        println!("Enter a number from 1 to {}.", ProviderKind::ALL.len());
    }
}

fn prompt_default(label: &str, default: &str) -> Result<Option<String>> {
    let answer = read_line(&format!("{label} [{default}]: "))?;
    Ok(Some(if answer.is_empty() {
        default.to_string()
    } else {
        answer
    }))
}

fn confirm(question: &str) -> Result<bool> {
    let answer = read_line(&format!("{question} [y/N]: "))?;
    Ok(matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn read_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line)? == 0 {
        bail!("input closed");
    }
    Ok(line.trim().to_string())
}
