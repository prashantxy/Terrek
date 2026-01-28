use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::thread;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{enable_raw_mode, disable_raw_mode},
};

enum Mode {
    Shell,
    Command,
}

fn main() -> anyhow::Result<()> {
    // ---  Setup ---
    let mut mode = Mode::Shell;
    let mut command_buffer = String::new();

    enable_raw_mode()?;

    // --- 1️⃣ Create PTY ---
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // --- 2️⃣ Spawn shell ---
    let cmd = CommandBuilder::new("/bin/bash");
    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut mut_writer = pair.master.take_writer()?; // mutable writer

    // --- 3️⃣ PTY output thread ---
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

    // --- 4️⃣ Keyboard input loop ---
    loop {
        if let Event::Key(KeyEvent { code, modifiers, kind: _, state: _ }) = event::read()? {

            match code {
                // Ctrl+X → Enter command mode
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
                        print!("{}", c); // show typed command
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
                        print!("\x08 \x08"); // erase last char in terminal
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

// --- Example Terrek commands ---
fn execute_terrek_command(
    cmd: &str,
    _writer: &mut dyn Write,
) -> anyhow::Result<()> {
    match cmd.trim() {
        ":hello" => println!(" Hello from Terrek!"),
        ":time" => {
            println!(" Current time: {}", chrono::Local::now());
        }
        ":clear" => {
            print!("\x1B[2J\x1B[1;1H"); // ANSI clear screen
        }
        _ => {
            println!("Unknown Terrek command: {}", cmd);
        }
    }
    Ok(())
}
