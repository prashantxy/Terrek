use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

mod db;
mod commands;
mod config;
mod context;
mod ai;
use db::worker::{start_db_worker, DbEvent};
use commands::{handle_command, TerrekAction};

use context::state::ContextState;
use context::git::get_git_branch;
use context::context_builder::build_context;

enum Mode {
    Shell,
    Command,
}

fn draw_prompt(buf: &str) {
    print!("\r\x1B[K[Terrek] > {}", buf);
    std::io::stdout().flush().ok();
}

fn clear_prompt() {
    print!("\r\x1B[K");
    std::io::stdout().flush().ok();
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let _cleanup = scopeguard::guard((), |_| {
        disable_raw_mode().ok();
    });

    let db_tx: Sender<DbEvent> = start_db_worker();
    let session_id = uuid::Uuid::new_v4().to_string();

    
    let mut context = ContextState::new();

    if let Ok(path) = std::env::current_dir() {
        context.set_project_root(path);
    }

    context.set_git_branch(get_git_branch());

    let mut current_output = String::new();
    let mut mode = Mode::Shell;
    let mut command_buffer = String::new();
    let mut shell_input_buffer = String::new();

   
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or("/bin/bash".to_string());
    let cmd = CommandBuilder::new(shell);
    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (out_tx, out_rx) = channel::<String>();

    
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buffer[..n]).to_string();
                    out_tx.send(text).ok();
                }
                Err(_) => break,
            }
        }
    });

   
    loop {
        while let Ok(text) = out_rx.try_recv() {
            print!("{}", text);
            std::io::stdout().flush().ok();
            current_output.push_str(&text);
        }

        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {

                // Switch to Terrek command mode
                KeyCode::Char('x') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = Mode::Command;
                    command_buffer.clear();
                    println!();
                    draw_prompt("");
                }

                KeyCode::Char(c) => match mode {
                    Mode::Shell => {
                        shell_input_buffer.push(c);
                        writer.write_all(&[c as u8])?;
                        writer.flush()?;
                    }
                    Mode::Command => {
                        command_buffer.push(c);
                        draw_prompt(&command_buffer);
                    }
                },

                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        shell_input_buffer.pop();
                        writer.write_all(b"\x7f")?;
                        writer.flush()?;
                    }
                    Mode::Command => {
                        command_buffer.pop();
                        draw_prompt(&command_buffer);
                    }
                },

                KeyCode::Enter => match mode {
                    Mode::Shell => {
                        // Inject exit code marker
                        let full_command = format!(
                            "{}; echo __TERREK_EXIT__$?\n",
                            shell_input_buffer
                        );

                        writer.write_all(full_command.as_bytes())?;
                        writer.flush()?;

                        context.add_command(shell_input_buffer.clone());

                        shell_input_buffer.clear();

                        // Wait briefly for output
                        std::thread::sleep(std::time::Duration::from_millis(100));

                        // Parse exit code
                        if let Some(pos) = current_output.find("__TERREK_EXIT__") {
                            let exit_part = &current_output[pos + 15..];
                            if let Some(code_str) = exit_part.lines().next() {
                                if let Ok(code) = code_str.trim().parse::<i32>() {
                                    context.(
                                        code,
                                        if code != 0 {
                                            Some(current_output.clone())
                                        } else {
                                            None
                                        },
                                    );
                                }
                            }
                        }

                        // Refresh git branch after command 
                        context.set_git_branch(get_git_branch());

                        context.debug_print();

                        db_tx.send(DbEvent::StoreCommand {
                            session_id: session_id.clone(),
                            command: context
                                .last_commands
                                .last()
                                .unwrap_or(&"[unknown]".to_string())
                                .to_string(),
                            output: current_output.clone(),
                            timestamp: chrono::Local::now().timestamp(),
                        })?;

                        // AI Hook on error
                        if context.last_exit_code.unwrap_or(0) != 0 {
                            println!("\n[Terrek AI] Detected failure. Context ready.");
                            println!("{}", build_context(&context));
                        }

                        current_output.clear();
                    }

                    Mode::Command => {
                        println!();

                        let action = handle_command(
                            command_buffer.trim_start_matches("terrek ").trim(),
                        )?;

                        match action {
                            TerrekAction::ExitToShell => {
                                clear_prompt();
                                mode = Mode::Shell;
                            }
                            TerrekAction::Output(text) => {
                                println!("[Terrek] {}", text);
                                draw_prompt("");
                            }
                        }

                        command_buffer.clear();
                    }
                },

                KeyCode::Esc => match mode {
                    Mode::Command => {
                        clear_prompt();
                        mode = Mode::Shell;
                    }
                    Mode::Shell => break Ok(()),
                },

                _ => {}
            }
        }
    }
}
