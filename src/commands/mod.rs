//! Command implementations. Each returns the text to show, so the same code
//! serves the CLI (printed to stdout) and the palette (drawn in raw mode).

pub mod doctor;
#[cfg(feature = "integrations")]
pub mod integrations;
pub mod open;

use anyhow::{anyhow, Result};
use chrono::{Local, TimeZone};
use std::path::PathBuf;

use crate::ai::prompts::{ask_prompt, fix_prompt, SYSTEM_PROMPT};
use crate::ai::setup::SetupArgs;
use crate::cli::{Command, ConfigAction};
use crate::config::{config_path, mask_secret, Config};
use crate::context::ContextState;
use crate::db::{CommandRecord, HistoryStore};

/// Everything a command may need about where it runs.
#[derive(Clone)]
pub struct Env {
    pub config: Config,
    pub cwd: PathBuf,
    pub shell: String,
    /// Newest first.
    pub recent: Vec<CommandRecord>,
    pub last_failed: Option<CommandRecord>,
}

impl Env {
    /// For CLI invocations: context comes from the current directory and the history DB.
    pub fn from_process(config: Config) -> Self {
        let store = config
            .history
            .enabled
            .then(|| HistoryStore::open_default().ok())
            .flatten();
        let recent = store
            .as_ref()
            .and_then(|s| s.recent(10, false).ok())
            .unwrap_or_default();
        let last_failed = store.as_ref().and_then(|s| s.last_failed().ok().flatten());
        Self {
            config,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            shell: std::env::var("SHELL").unwrap_or_else(|_| "unknown".into()),
            recent,
            last_failed,
        }
    }

    fn context(&self) -> ContextState {
        ContextState::capture(&self.cwd, &self.shell, self.recent.clone())
    }
}

/// Commands that read from the terminal themselves (prompts, editors).
pub fn is_interactive(cmd: &Command) -> bool {
    match cmd {
        Command::Setup { .. } => true,
        #[cfg(feature = "integrations")]
        Command::Send { target } => integrations::is_interactive(target),
        _ => false,
    }
}

pub fn execute(cmd: Command, env: &Env, stdin: Option<String>) -> Result<String> {
    match cmd {
        Command::Shell => Ok("You are already in a Terrek shell.".into()),
        Command::Setup {
            provider,
            model,
            base_url,
        } => crate::ai::setup::run(
            env.config.clone(),
            SetupArgs {
                provider,
                model,
                base_url,
            },
        ),
        Command::Ask { question } => ask(env, &question.join(" "), stdin.as_deref()),
        Command::Fix => fix(env),
        Command::History { limit, failed } => {
            let store = HistoryStore::open_default()?;
            Ok(format_records(&store.recent(limit, failed)?))
        }
        Command::Search { query, limit } => {
            let store = HistoryStore::open_default()?;
            Ok(format_records(&store.search(&query, limit)?))
        }
        Command::Open { target } => open::open(&target.join(" ")),
        Command::Config { action } => config_command(action, env),
        Command::Doctor { ping } => Ok(doctor::run(env, ping)),
        #[cfg(feature = "integrations")]
        Command::Send { target } => integrations::send(target, env),
    }
}

pub fn ask(env: &Env, question: &str, stdin: Option<&str>) -> Result<String> {
    let model = crate::ai::from_config(&env.config.ai)?;
    model.complete(SYSTEM_PROMPT, &ask_prompt(&env.context(), question, stdin))
}

pub fn fix(env: &Env) -> Result<String> {
    let failed = env.last_failed.as_ref().ok_or_else(|| {
        anyhow!(
            "no failed command recorded yet. Commands are recorded when you run them inside \
             `terrek` (zsh, bash, or fish)."
        )
    })?;
    let model = crate::ai::from_config(&env.config.ai)?;
    let mut ctx = env.context();
    ctx.recent_commands
        .retain(|r| r.id.is_none() || r.id != failed.id);
    let answer = model.complete(SYSTEM_PROMPT, &fix_prompt(&ctx, failed))?;
    Ok(format!(
        "$ {}  (exit {})\n\n{answer}",
        failed.command,
        failed.exit_code.unwrap_or(-1)
    ))
}

pub fn format_records(records: &[CommandRecord]) -> String {
    if records.is_empty() {
        return "No commands recorded yet.".into();
    }
    let today = Local::now().date_naive();
    records
        .iter()
        .rev()
        .map(|r| {
            let when = Local
                .timestamp_opt(r.timestamp, 0)
                .single()
                .map(|t| {
                    if t.date_naive() == today {
                        t.format("%H:%M:%S").to_string()
                    } else {
                        t.format("%b %d %H:%M").to_string()
                    }
                })
                .unwrap_or_default();
            let status = match r.exit_code {
                Some(0) => "  ✓".to_string(),
                Some(code) => format!("{:>3}", code),
                None => "  ?".to_string(),
            };
            let duration = r
                .duration_ms
                .filter(|ms| *ms >= 1000)
                .map(|ms| format!("  ({:.1}s)", ms as f64 / 1000.0))
                .unwrap_or_default();
            format!("{when:>12}  {status}  {}{duration}", r.command)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn config_command(action: ConfigAction, env: &Env) -> Result<String> {
    match action {
        ConfigAction::Path => Ok(config_path().display().to_string()),
        ConfigAction::Show => {
            let mut shown = env.config.clone();
            shown.ai.api_key = shown.ai.api_key.as_deref().map(mask_secret);
            let mut text = format!(
                "# {}\n{}",
                config_path().display(),
                toml::to_string_pretty(&shown)?
            );
            match crate::ai::auto::resolve(&env.config.ai) {
                Ok(r) => text.push_str(&format!(
                    "\n# effective: {}/{} (key from {})\n",
                    r.provider,
                    r.model,
                    r.key_source.as_deref().unwrap_or("not needed")
                )),
                Err(e) => text.push_str(&format!("\n# {e}\n")),
            }
            Ok(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_records_oldest_first_with_status() {
        let records = vec![
            CommandRecord {
                command: "cargo test".into(),
                exit_code: Some(101),
                duration_ms: Some(2500),
                timestamp: 1_700_000_100,
                ..Default::default()
            },
            CommandRecord {
                command: "ls".into(),
                exit_code: Some(0),
                timestamp: 1_700_000_000,
                ..Default::default()
            },
        ];
        let text = format_records(&records);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].ends_with("✓  ls"));
        assert!(lines[1].contains("101  cargo test  (2.5s)"));
    }

    #[test]
    fn fix_without_history_explains_why() {
        let env = Env {
            config: Config::default(),
            cwd: ".".into(),
            shell: "zsh".into(),
            recent: vec![],
            last_failed: None,
        };
        assert!(fix(&env)
            .unwrap_err()
            .to_string()
            .contains("no failed command"));
    }
}
