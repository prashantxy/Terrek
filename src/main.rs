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

use db::worker::{start_db_worker, DbEvent};
use commands::{handle_command, TerrekAction};

enum Mode {
    Shell,
    Command,
}

struct Screen {
    lines: Vec<String>,
}

impl Screen {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn push_pty_text(&mut self, text: &str) {
        for line in text.replace("\r", "").split('\n') {
            self.lines.push(line.to_string());
        }

        if self.lines.len() > 300 {
            self.lines.drain(0..self.lines.len() - 300);
        }
    }

    fn push_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    fn render(&self, mode: &Mode, command_buffer: &str) {
        use std::io::{stdout, Write};

        print!("\x1B[2J\x1B[1;1H");

        for line in &self.lines {
            println!("{}", line);
        }

        if let Mode::Command = mode {
            print!("\n[Terrek] > {}", command_buffer);
        }

        stdout().flush().ok();
    }
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let _cleanup = scopeguard::guard((), |_| {
        disable_raw_mode().ok();
    });

    let db_tx: Sender<DbEvent> = start_db_worker();
    let session_id = uuid::Uuid::new_v4().to_string();

    let mut screen = Screen::new();
    let mut current_output = String::new();

    let mut mode = Mode::Shell;
    let mut command_buffer = String::new();

    // === PTY SETUP ===
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

    // === MAIN LOOP ===
    loop {
        // Collect PTY output into screen buffer
        while let Ok(text) = out_rx.try_recv() {
            screen.push_pty_text(&text);
            current_output.push_str(&text);
        }

        // Render once
        screen.render(&mode, &command_buffer);

        // Input handling
        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {
                // Enter Terrek mode
                KeyCode::Char('x') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = Mode::Command;
                    command_buffer.clear();
                }

                KeyCode::Char(c) => match mode {
                    // Shell = pure passthrough
                    Mode::Shell => {
                        writer.write_all(&[c as u8])?;
                        writer.flush()?;
                    }
                    // Command mode = local buffer
                    Mode::Command => {
                        command_buffer.push(c);
                    }
                },

                KeyCode::Enter => match mode {
                    Mode::Shell => {
                        writer.write_all(b"\r")?;
                        writer.flush()?;

                        db_tx.send(DbEvent::StoreCommand {
                            session_id: session_id.clone(),
                            command: "[shell command]".to_string(),
                            output: current_output.clone(),
                            timestamp: chrono::Local::now().timestamp(),
                        })?;

                        current_output.clear();
                    }

                    Mode::Command => {
                        let action = handle_command(
                            command_buffer.trim_start_matches("terrek ").trim(),
                        )?;

                        match action {
                            TerrekAction::ExitToShell => {
                                mode = Mode::Shell;
                            }
                            TerrekAction::Output(text) => {
                                screen.push_line(&format!("[Terrek] {}", text));
                            }
                        }

                        command_buffer.clear();
                    }
                },

                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        writer.write_all(b"\x7f")?;
                        writer.flush()?;
                    }
                    Mode::Command => {
                        command_buffer.pop();
                    }
                },

                KeyCode::Esc => match mode {
                    Mode::Command => mode = Mode::Shell,
                    Mode::Shell => break Ok(()),
                },

                _ => {}
            }
        }
    }
}
