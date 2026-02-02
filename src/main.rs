use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::thread;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

enum Mode {
    Shell,
    Command,
}

fn main() -> anyhow::Result<()> {
    let mut mode = Mode::Shell;
    let mut command_buffer = String::new();

    enable_raw_mode()?;

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
    let mut mut_writer = pair.master.take_writer()?;

    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    print!("{}", String::from_utf8_lossy(&buffer[..n]));
                }
                Err(_) => break,
            }
        }
    });

    loop {
        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: _,
            state: _,
        }) = event::read()?
        {
            match code {
                KeyCode::Char('x') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = Mode::Command;
                    command_buffer.clear();
                    print!("\n[Terrek Command Mode] > ");
                }

                KeyCode::Char(c) => match mode {
                    Mode::Shell => {
                        mut_writer.write_all(&[c as u8])?;
                    }
                    Mode::Command => {
                        command_buffer.push(c);
                        print!("{}", c);
                    }
                },

                KeyCode::Enter => match mode {
                    Mode::Shell => {
                        mut_writer.write_all(b"\r")?;
                    }
                    Mode::Command => {
                        println!();
                        execute_terrek_command(&command_buffer, &mut mut_writer)?;
                        command_buffer.clear();
                        mode = Mode::Shell;
                    }
                },

                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        mut_writer.write_all(b"\x7f")?;
                    }
                    Mode::Command => {
                        command_buffer.pop();
                        print!("\x08 \x08");
                    }
                },

                KeyCode::Esc => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    Ok(())
}

fn execute_terrek_command(cmd: &str, _writer: &mut dyn Write) -> anyhow::Result<()> {
    match cmd.trim() {
        "terrek hello" => println!(" Hello from Terrek!"),
        "terrek time" => {
            println!(" Current time: {}", chrono::Local::now());
        }
        "terrek clear" => {
            print!("\x1B[2J\x1B[1;1H");
        }
        _ => {
            println!("Unknown Terrek command: {}", cmd);
        }
    }
    Ok(())
}
