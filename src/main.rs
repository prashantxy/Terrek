use std::io::{IsTerminal, Read};
use std::process::ExitCode;

use clap::Parser;

use terrek::cli::{Cli, Command};
use terrek::commands::{self, Env};
use terrek::config::Config;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("terrek: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<u8> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command.unwrap_or(Command::Shell) {
        Command::Shell => shell(config),
        command => {
            let stdin = match &command {
                Command::Ask { .. } => piped_stdin()?,
                _ => None,
            };
            let env = Env::from_process(config);
            let output = commands::execute(command, &env, stdin)?;
            if !output.is_empty() {
                println!("{output}");
            }
            Ok(0)
        }
    }
}

#[cfg(unix)]
fn shell(config: Config) -> anyhow::Result<u8> {
    // A session that ended normally is a success, whatever the shell's last status was, so
    // `terrek && exit` in an rc file only exits when Terrek actually ran.
    terrek::sessions::run(config)?;
    Ok(0)
}

#[cfg(not(unix))]
fn shell(_config: Config) -> anyhow::Result<u8> {
    anyhow::bail!(
        "the interactive shell wrapper supports macOS and Linux for now; \
         `terrek ask`, `open`, and `send` work here"
    )
}

/// `cargo build 2>&1 | terrek ask "why"` attaches the piped text.
fn piped_stdin() -> anyhow::Result<Option<String>> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    stdin.read_to_end(&mut bytes)?;
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}
