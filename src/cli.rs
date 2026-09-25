//! Command-line grammar. The same parser drives both `terrek <cmd>` and the
//! in-shell palette (Ctrl+T), so every command works in both places.

use clap::{Parser, Subcommand};

use crate::ai::provider::ProviderKind;

#[derive(Debug, Parser)]
#[command(
    name = "terrek",
    version,
    about = "An AI layer for the shell you already use",
    long_about = "Run `terrek` to start your shell with Terrek attached. Every command you run is \
                  remembered with its exit code and output, and Ctrl+T opens a palette where you \
                  can ask questions or type `fix` after something fails."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Start your shell with Terrek attached (the default)
    Shell,

    /// Choose an AI provider and store its key
    Setup {
        #[arg(long, value_enum)]
        provider: Option<ProviderKind>,
        #[arg(long)]
        model: Option<String>,
        /// Custom endpoint (OpenAI-compatible servers, remote Ollama, proxies)
        #[arg(long)]
        base_url: Option<String>,
    },

    /// Ask a question with your terminal context attached (stdin is included when piped)
    #[command(visible_alias = "ai")]
    Ask {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        question: Vec<String>,
    },

    /// Explain the most recent failed command and suggest a fix
    Fix,

    /// Show recent commands
    History {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Only commands that exited non-zero
        #[arg(long)]
        failed: bool,
    },

    /// Search recorded commands
    Search {
        query: String,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },

    /// Open an app, URL, or search (`open yt lofi`, `open gh rust-lang/rust`)
    Open {
        #[arg(required = true, trailing_var_arg = true)]
        target: Vec<String>,
    },

    /// Inspect configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Check that config, provider, shell hooks, and history are working
    Doctor {
        /// Also send a tiny request to the AI provider
        #[arg(long)]
        ping: bool,
    },

    /// Send a message to Slack, Discord, Telegram, email, X, WhatsApp, or Reddit
    #[cfg(feature = "integrations")]
    Send {
        #[command(subcommand)]
        target: crate::commands::integrations::SendTarget,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    /// Print the config file location
    Path,
    /// Print the effective configuration (keys masked)
    Show,
}

/// Top-level command names, used for palette completion.
pub fn command_names() -> Vec<String> {
    use clap::CommandFactory;
    Cli::command()
        .get_subcommands()
        .flat_map(|c| {
            std::iter::once(c.get_name().to_string())
                .chain(c.get_visible_aliases().map(str::to_string))
        })
        .filter(|name| name != "shell")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn ask_keeps_hyphenated_words() {
        let cli = Cli::try_parse_from(["terrek", "ask", "why", "does", "-v", "fail"]).unwrap();
        match cli.command {
            Some(Command::Ask { question }) => assert_eq!(question.join(" "), "why does -v fail"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn ai_is_an_alias_for_ask() {
        let cli = Cli::try_parse_from(["terrek", "ai", "hello"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Ask { .. })));
    }

    #[test]
    fn command_names_include_aliases_but_not_shell() {
        let names = command_names();
        assert!(names.contains(&"ask".to_string()));
        assert!(names.contains(&"ai".to_string()));
        assert!(!names.contains(&"shell".to_string()));
    }
}
