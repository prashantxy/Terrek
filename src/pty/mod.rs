//! The user's shell running inside a pseudo-terminal.

pub mod integration;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};

use integration::{Integration, ShellKind};

pub struct ShellProcess {
    pub kind: ShellKind,
    pub program: String,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    _integration: Integration,
}

pub struct SpawnOptions<'a> {
    pub program: &'a str,
    pub integration: bool,
    pub session_id: &'a str,
    pub cols: u16,
    pub rows: u16,
}

/// `$SHELL`, falling back to `/bin/sh`.
pub fn default_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/sh".into())
}

impl ShellProcess {
    pub fn spawn(opts: SpawnOptions) -> Result<Self> {
        let kind = ShellKind::detect(opts.program);
        let integration = if opts.integration && kind.is_supported() {
            Integration::prepare(kind)?
        } else {
            Integration::none()
        };

        let pair = native_pty_system()
            .openpty(size(opts.cols, opts.rows))
            .context("opening a pseudo-terminal")?;

        let mut cmd = CommandBuilder::new(opts.program);
        cmd.args(&integration.args);
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }
        cmd.env(
            "TERM",
            std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into()),
        );
        cmd.env(crate::SESSION_ENV, opts.session_id);
        for (key, value) in &integration.env {
            cmd.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("starting {}", opts.program))?;
        // Close our copy of the slave so reads return EOF once the shell exits.
        drop(pair.slave);
        let writer = pair.master.take_writer()?;

        Ok(Self {
            kind,
            program: opts.program.to_string(),
            master: pair.master,
            writer,
            child,
            _integration: integration,
        })
    }

    pub fn reader(&self) -> Result<Box<dyn Read + Send>> {
        self.master.try_clone_reader()
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master.resize(size(cols, rows))?;
        Ok(())
    }

    /// Wait for the shell and return its exit code.
    pub fn wait(&mut self) -> Result<u32> {
        Ok(self.child.wait()?.exit_code())
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}
