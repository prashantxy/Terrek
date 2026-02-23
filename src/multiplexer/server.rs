use anyhow::Result;
use uuid::Uuid;
use crossterm::event::{KeyCode, KeyEvent};

use crate::multiplexer::session::Session;

pub struct Multiplexer {
    pub sessions: Vec<Session>,
    pub active_session: usize,
}

impl Multiplexer {

    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            active_session: 0,
        }
    }

    pub fn create_session(&mut self, name: String) {
        let session = Session::new(name);
        self.sessions.push(session);
        self.active_session = self.sessions.len() - 1;
    }

    
    pub fn kill_session(&mut self, id: Uuid) {
        self.sessions.retain(|s| s.id != id);

        if self.active_session >= self.sessions.len() {
            self.active_session = 0;
        }
    }

  
    pub fn active_session_mut(&mut self) -> &mut Session {
        &mut self.sessions[self.active_session]
    }

    pub fn send_input_to_active(&mut self, key: KeyEvent) -> Result<()> {
        let session = self.active_session_mut();
        let window = session.active_window_mut();
        let pane = window.active_pane_mut();

        match key.code {
            KeyCode::Char(c) => {
                pane.write(&[c as u8])?;
            }
            KeyCode::Enter => {
                pane.write(b"\n")?;
            }
            KeyCode::Backspace => {
                pane.write(b"\x7f")?;
            }
            KeyCode::Tab => {
                pane.write(b"\t")?;
            }
            _ => {}
        }

        Ok(())
    }
}