use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::thread;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{enable_raw_mode, disable_raw_mode},
};

fn main() -> anyhow::Result<()> {
    // 1️ Enable raw mode
    enable_raw_mode()?;

    // 2️ Create PTY
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // 3️ Spawn shell
    let cmd = CommandBuilder::new("/bin/bash");
    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    // 4️ Shell output thread
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            let n = reader.read(&mut buffer).unwrap();
            print!("{}", String::from_utf8_lossy(&buffer[..n]));
        }
    });

    // 5️ Keyboard → PTY
    loop {
        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Char(c) => {
                    writer.write_all(&[c as u8])?;
                }
                KeyCode::Enter => {
                    writer.write_all(b"\r")?;
                }
                KeyCode::Backspace => {
                    writer.write_all(b"\x7f")?;
                }
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    Ok(())
}
