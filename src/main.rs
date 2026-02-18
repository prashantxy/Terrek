use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use crate::ai::auto::maybe_trigger_ai;
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

enum Mode {
    Shell,
    Terrek,
}

fn draw_prompt(buf: &str) {
    print!("\r\x1B[K[Terrek Command] > {}", buf);
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

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or("/bin/bash".to_string());
    let mut cmd = CommandBuilder::new(shell);
    cmd.env("PS1", "[Terrek-Shell] \\w > ");
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

    let mut current_output = String::new();
    let mut mode = Mode::Shell;
    let mut command_buffer = String::new();
    let mut shell_input_buffer = String::new();

    loop {
        while let Ok(text) = out_rx.try_recv() {
            print!("{}", text);
            std::io::stdout().flush().ok();
            current_output.push_str(&text);
        }

        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {
                KeyCode::Char('t') if modifiers.contains(KeyModifiers::CONTROL) => {
                mode = Mode::Terrek;
                command_buffer.clear();
                println!("\n-- TERREK MODE --");
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
                        current_output.clear();

                        let full_command = format!(
                            "{}; echo __TERREK_EXIT__$?\n",
                            shell_input_buffer
                        );

                        writer.write_all(full_command.as_bytes())?;
                        writer.flush()?;

                        context.add_command(shell_input_buffer.clone());
                        shell_input_buffer.clear();

                        std::thread::sleep(std::time::Duration::from_millis(120));

                        if let Some(pos) = current_output.rfind("__TERREK_EXIT__") {
                            let exit_part =
                                &current_output[pos + "__TERREK_EXIT__".len()..];

                            if let Some(code_str) = exit_part.lines().next() {
                                if let Ok(code) = code_str.trim().parse::<i32>() {
                                    let clean_output =
                                        current_output[..pos].to_string();

                                    context.update_exit_code(
                                        code,
                                        if code != 0 {
                                            Some(clean_output.clone())
                                        } else {
                                            None
                                        },
                                    );

                                    current_output = clean_output;
                                }
                            }
                        }

                        context.set_git_branch(get_git_branch());

                        if let Ok(path) = std::env::current_dir() {
                            context.set_project_root(path);
                        }

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

                        if context.last_exit_code.unwrap_or(0) != 0 {
                        if let Err(e) = ai::auto::maybe_trigger_ai(&context) {
                       println!("\n[Terrek AI Error] {}\n", e);
    }
}

                        current_output.clear();
                    }

                    Mode::Command => {
                        println!();

                        let action = handle_command(
                        &context,
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
