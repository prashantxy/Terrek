use anyhow::Result;
use uuid::Uuid;

use crate::multiplexer::session::Session;

pub struct Multiplexer {
    pub sessions: Vec<Session>,
    pub active_session: usize,
}

impl Multiplexer {

    /// Create a new multiplexer with one session
    pub fn new(session_name: String) -> Self {
        let initial_session = Session::new(session_name);

        Self {
            sessions: vec![initial_session],
            active_session: 0,
        }
    }

    /// Get active session safely
    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(self.active_session)
    }

    /// Create new session
    pub fn create_session(&mut self, name: String) -> Result<()> {
        let session = Session::new(name);
        self.sessions.push(session);
        self.active_session = self.sessions.len() - 1;
        Ok(())
    }

    /// Close session (never allow zero)
    pub fn close_session(&mut self, index: usize) {
        if self.sessions.len() <= 1 {
            return;
        }

        if index < self.sessions.len() {
            self.sessions.remove(index);

            if self.active_session >= self.sessions.len() {
                self.active_session = self.sessions.len() - 1;
            }
        }
    }

    /// Switch to next session
    pub fn next_session(&mut self) {
        if !self.sessions.is_empty() {
            self.active_session = (self.active_session + 1) % self.sessions.len();
        }
    }

    /// Poll all panes in active session
    pub fn poll(&mut self) -> Result<()> {
        if let Some(session) = self.active_session_mut() {
            if let Some(window) = session.active_window_mut() {
                window.poll_panes()?;
            }
        }
        Ok(())
    }
}