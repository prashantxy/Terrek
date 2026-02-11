use anyhow::Result;
use chrono::{Local, TimeZone};

use crate::db::history::{get_history, search_history};

pub enum TerrekAction {
    ExitToShell,
    Output(String),
}

pub fn handle_command(cmd: &str) -> Result<TerrekAction> {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();

    if parts.is_empty() {
        return Ok(TerrekAction::Output(String::new()));
    }

    let output = match parts[0] {
        "hello" => {
            "Hello from Terrek!".to_string()
        }

        "time" => {
            format!("Current time: {}", Local::now())
        }

        "clear" => {
            // special signal to main to clear screen buffer
            "__CLEAR__".to_string()
        }

        "history" => {
            let history = get_history(20)?;
            let mut lines = Vec::new();

            for (cmd, _, ts) in history {
                let time = Local.timestamp_opt(ts, 0).unwrap();
                lines.push(format!(
                    "[{}] {}",
                    time.format("%H:%M:%S"),
                    cmd
                ));
            }

            lines.join("\n")
        }

        "last" => {
            let history = get_history(1)?;
            if let Some((cmd, output, ts)) = history.first() {
                let time = Local.timestamp_opt(*ts, 0).unwrap();

                format!(
                    "Last Command [{}]:\n{}\n\nOutput:\n{}",
                    time, cmd, output
                )
            } else {
                "No history found".to_string()
            }
        }

        "search" => {
            if parts.len() < 2 {
                "Usage: terrek search <keyword>".to_string()
            } else {
                let results = search_history(parts[1])?;
                let mut lines = Vec::new();

                for (cmd, _, ts) in results {
                    let time = Local.timestamp_opt(ts, 0).unwrap();
                    lines.push(format!(
                        "[{}] {}",
                        time.format("%H:%M:%S"),
                        cmd
                    ));
                }

                lines.join("\n")
            }
        }

        "ai"=>{
          if parts.len()>=2 && parts[1]=="setup"{
            return Ok(TerrekAction::Output(Setup_ai()?));
          }
        }

        "help" => {
            r#"Terrek Commands:
  terrek hello
  terrek time
  terrek clear
  terrek history
  terrek last
  terrek ai
  terrek search <keyword>
  terrek exit"#
                .to_string()
        }

        "exit" => return Ok(TerrekAction::ExitToShell),

        _ => "Unknown Terrek command".to_string(),
    };

    Ok(TerrekAction::Output(output))
}
