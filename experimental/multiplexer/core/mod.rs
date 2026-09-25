pub mod command;
pub mod session;
pub mod window;
pub mod pane;
pub mod layout;
pub mod geometry;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use session::Session;
use command::TerrekCommand;

pub struct Multiplexer {
    pub sessions: Vec<Session>,
    pub active_session: usize,
}

impl Multiplexer {

    pub fn new(name: String) -> Result<Self> {
        let session = Session::new(name)?;
        Ok(Self {
            sessions: vec![session],
            active_session: 0,
        })
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.sessions.get(self.active_session)
    }

    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(self.active_session)
    }

    pub fn execute(&mut self, cmd: TerrekCommand) -> Result<()> {
        let session = match self.active_session_mut() {
            Some(s) => s,
            None => return Ok(()),
        };

        match cmd {
            TerrekCommand::SplitVertical => session.split_vertical()?,
            TerrekCommand::SplitHorizontal => session.split_horizontal()?,
            TerrekCommand::NextWindow => session.next_window(),
            TerrekCommand::NewWindow => session.create_window()?,
            TerrekCommand::CloseWindow => {
                let idx = session.active_window;
                session.close_window(idx);
            }
            TerrekCommand::NextSession => {
                self.active_session =
                    (self.active_session + 1) % self.sessions.len();
            }
            _ => {}
        }

        Ok(())
    }

    pub fn send_input_to_active(&mut self, key: KeyEvent) -> Result<()> {
        let session = match self.active_session_mut() {
            Some(s) => s,
            None => return Ok(()),
        };

        let window = match session.active_window_mut() {
            Some(w) => w,
            None => return Ok(()),
        };

        let pane = match window.active_pane_mut() {
            Some(p) => p,
            None => return Ok(()),
        };

        match key.code {
            KeyCode::Char(c) => pane.write(&[c as u8])?,
            KeyCode::Enter => pane.write(b"\n")?,
            KeyCode::Backspace => pane.write(b"\x7f")?,
            _ => {}
        }

        Ok(())
    }

    pub fn handle_resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(session) = self.active_session_mut() {
            if let Some(window) = session.active_window_mut() {
                window.resize_all(rows, cols)?;
            }
        }
        Ok(())
    }

    pub fn poll(&mut self) -> Result<()> {
        if let Some(session) = self.active_session_mut() {
            if let Some(window) = session.active_window_mut() {
                window.poll_panes()?;
            }
        }
        Ok(())
    }
}