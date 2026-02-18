use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

use uuid::Uuid;
mod ai;
mod db;
mod commands;
mod context;
mod config;

use db::worker::{start_db_worker, DbEvent};
use commands::{handle_command, TerrekAction};
use context::state::ContextState;

enum Mode {
    Shell,
    Terrek,
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
    let session_id = Uuid::new_v4().to_string();

    let mut context = ContextState::new();
---------------------------

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

    // -----------------------------
    // Runtime State
    // -----------------------------

    let mut mode = Mode::Shell;
    let mut shell_input_buffer = String::new();
    let mut terrek_buffer = String::new();
    let mut current_output = String::new();

    println!("You are in SHELL session. Press Ctrl+T for TERREK mode.");

    loop {
        // Print shell output
        while let Ok(text) = out_rx.try_recv() {
            print!("{}", text);
            std::io::stdout().flush().ok();
            current_output.push_str(&text);
        }

        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {

                // -----------------------------
                // Switch to TERREK Mode
                // -----------------------------
                KeyCode::Char('t') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = Mode::Terrek;
                    terrek_buffer.clear();
                    println!("\n-- TERREK MODE --");
                    draw_prompt("");
                }

                // -----------------------------
                // Character Input
                // -----------------------------
                KeyCode::Char(c) => match mode {
                    Mode::Shell => {
                        shell_input_buffer.push(c);
                        writer.write_all(&[c as u8])?;
                        writer.flush()?;
                    }
                    Mode::Terrek => {
                        terrek_buffer.push(c);
                        draw_prompt(&terrek_buffer);
                    }
                },

                // -----------------------------
                // Backspace
                // -----------------------------
                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        shell_input_buffer.pop();
                        writer.write_all(b"\x7f")?;
                        writer.flush()?;
                    }
                    Mode::Terrek => {
                        terrek_buffer.pop();
                        draw_prompt(&terrek_buffer);
                    }
                },

                // -----------------------------
                // Enter Handling
                // -----------------------------
                KeyCode::Enter => match mode {

                    // ---- SHELL MODE ----
                    Mode::Shell => {
                        current_output.clear();

                        let full_command =
                            format!("{}; echo __TERREK_EXIT__$?\n", shell_input_buffer);

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

                                    context.update_exit_code(code, None);
                                    current_output = clean_output;
                                }
                            }
                        }

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

                        current_output.clear();
                    }

                    // ---- TERREK MODE ----
                    Mode::Terrek => {
                        println!();

                        let input = terrek_buffer.trim();

                        if input.starts_with("terrek ") {
                            let stripped =
                                input.trim_start_matches("terrek ").trim();

                            let action =
                                handle_command(&context, stripped)?;

                            match action {
                                TerrekAction::Output(text) => {
                                    println!("[Terrek] {}", text);
                                }
                            }
                        } else {
                            // Raw forward to shell
                            let full_command =
                                format!("{}\n", input);

                            writer.write_all(full_command.as_bytes())?;
                            writer.flush()?;
                        }

                        terrek_buffer.clear();
                        draw_prompt("");
                    }
                },

                // -----------------------------
                // ESC Handling
                // -----------------------------
                KeyCode::Esc => match mode {
                    Mode::Terrek => {
                        clear_prompt();
                        mode = Mode::Shell;
                        println!("\n-- SHELL MODE --");
                    }
                    Mode::Shell => break Ok(()),
                },

                _ => {}
            }
        }
    }
}
