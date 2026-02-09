use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

mod db;
use db::worker::{start_db_worker, DbEvent};

enum Mode {
    Shell,
    Command,
}

fn execute_terrek_command(cmd: &str) -> anyhow::Result<bool> {
    match cmd.trim() {
        "terrek hello" => println!("\nHello from Terrek!"),
        "terrek time" => println!("\nCurrent time: {}", chrono::Local::now()),
        "terrek clear" => print!("\x1B[2J\x1B[1;1H"),
        "terrek exit" => return Ok(true),
        _ => println!("\nUnknown Terrek command: {}", cmd),
    }
    Ok(false)
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;

    // Ensure raw mode is always disabled
    let _cleanup = scopeguard::guard((), |_| {
        disable_raw_mode().ok();
    });

    let db_tx: Sender<DbEvent> = start_db_worker();
    let session_id = uuid::Uuid::new_v4().to_string();

    let mut current_shell_input = String::new();
    let mut current_output = String::new();

    let mut mode = Mode::Shell;
    let mut command_buffer = String::new();

    // PTY setup
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let cmd = CommandBuilder::new("/bin/bash");
    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (out_tx, out_rx) = channel::<String>();

    // Thread to read PTY output
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

    // MAIN LOOP
    loop {
        // Drain PTY output
        while let Ok(text) = out_rx.try_recv() {
            print!("{}", text);
            std::io::stdout().flush().ok();
            current_output.push_str(&text);
        }

        // Read keyboard
        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {
                KeyCode::Char('x') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = Mode::Command;
                    command_buffer.clear();
                    println!("\n[Terrek] > ");
                }

                KeyCode::Char(c) => match mode {
                    Mode::Shell => {
                        current_shell_input.push(c);
                        writer.write_all(&[c as u8])?;
                        writer.flush()?;
                    }
                    Mode::Command => {
                        command_buffer.push(c);
                        print!("{}", c);
                        std::io::stdout().flush().ok();
                    }
                },

                KeyCode::Enter => match mode {
                    Mode::Shell => {
                        writer.write_all(b"\r")?;
                        writer.flush()?;

                        db_tx.send(DbEvent::StoreCommand {
                            session_id: session_id.clone(),
                            command: current_shell_input.clone(),
                            output: current_output.clone(),
                            timestamp: chrono::Local::now().timestamp(),
                        })?;

                        current_shell_input.clear();
                        current_output.clear();
                    }

                    Mode::Command => {
                        println!();
                        let should_exit = execute_terrek_command(&command_buffer)?;
                        command_buffer.clear();

                        if should_exit {
                            mode = Mode::Shell;
                            println!("[Back to Shell]");
                        } else {
                            print!("[Terrek] > ");
                            std::io::stdout().flush().ok();
                        }
                    }
                },

                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        if !current_shell_input.is_empty() {
                            current_shell_input.pop();
                            writer.write_all(b"\x7f")?;
                            writer.flush()?;
                        }
                    }
                    Mode::Command => {
                        if !command_buffer.is_empty() {
                            command_buffer.pop();
                            print!("\x08 \x08");
                            std::io::stdout().flush().ok();
                        }
                    }
                },

                KeyCode::Esc => match mode {
                    Mode::Command => {
                        mode = Mode::Shell;
                        println!("\n[Back to Shell]");
                    }
                    Mode::Shell => break,
                },

                _ => {}
            }
        }
    }
}
