use anyhow::Result;
use chrono::{Local, TimeZone};

use crate::db::history::{get_history, search_history};

pub enum TerrekAction {
    Continue,
    ExitToShell,
}

pub fn handle_command(cmd: &str) -> Result<TerrekAction> {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();

    if parts.is_empty() {
        return Ok(TerrekAction::Continue);
    }

    match parts[0] {
        "hello" => {
            println!("\nHello from Terrek!");
        }

        "time" => {
            println!("\nCurrent time: {}", Local::now());
        }

        "clear" => {
            print!("\x1B[2J\x1B[1;1H");
        }

        "history" => {
            let history = get_history(20)?;
            for (cmd, _, ts) in history {
                let time = Local.timestamp_opt(ts, 0).unwrap();
                println!("[{}] {}", time.format("%H:%M:%S"), cmd);
            }
        }

        "last" => {
            let history = get_history(1)?;
            if let Some((cmd, output, ts)) = history.first() {
                let time = Local.timestamp_opt(*ts, 0).unwrap();
                println!("\nLast Command [{}]:\n{}\n", time, cmd);
                println!("Output:\n{}\n", output);
            }
        }

        "search" => {
            if parts.len() < 2 {
                println!("Usage: terrek search <keyword>");
            } else {
                let results = search_history(parts[1])?;
                for (cmd, _, ts) in results {
                    let time = Local.timestamp_opt(ts, 0).unwrap();
                    println!("[{}] {}", time.format("%H:%M:%S"), cmd);
                }
            }
        }

        "help" => {
            println!(
                r#"
Terrek Commands:
  terrek hello
  terrek time
  terrek clear
  terrek history
  terrek last
  terrek search <keyword>
  terrek exit
"#
            );
        }

        "exit" => return Ok(TerrekAction::ExitToShell),

        _ => {
            println!("Unknown Terrek command");
        }
    }

    Ok(TerrekAction::Continue)
}
