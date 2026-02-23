use anyhow::Result;
use uuid::Uuid;

use std::io::{Read, Write};

use portable_pty::{
    CommandBuilder,
    PtySize,
    MasterPty,
    native_pty_system,
};

use crate::renderer::screen::ScrollbackBuffer;

pub struct Pane {
    pub id: Uuid,

    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    reader: Box<dyn Read + Send>,

    pub buffer: ScrollbackBuffer,

    pub rows: u16,
    pub cols: u16,
}

impl Pane {

    /// Spawn a new shell inside a PTY
    pub fn spawn_shell() -> Result<Self> {
        let pty_system = native_pty_system();

        let rows = 24;
        let cols = 80;

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/bash".to_string());

        let cmd = CommandBuilder::new(shell);
        let _child = pair.slave.spawn_command(cmd)?;

        let writer = pair.master.take_writer()?;
        let reader = pair.master.try_clone_reader()?;

        Ok(Self {
            id: Uuid::new_v4(),
            master: pair.master,
            writer,
            reader,
            buffer: ScrollbackBuffer::new(),
            rows,
            cols,
        })
    }

    /// Write input into the pane
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Read shell output and push into scrollback buffer
    pub fn read_output(&mut self) -> Result<()> {
        let mut buf = [0u8; 4096];

        match self.reader.read(&mut buf) {
            Ok(n) if n > 0 => {
                let text = String::from_utf8_lossy(&buf[..n]);
                self.buffer.push(&text);
            }
            _ => {}
        }

        Ok(())
    }

    /// Resize the PTY
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.rows = rows;
        self.cols = cols;

        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        Ok(())
    }
}