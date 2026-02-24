use anyhow::Result;

use crate::multiplexer::session::Session;

pub mod session;
pub mod geometry;
pub mod layout;
pub mod pane;
pub mod window;
pub mod command;

pub struct Multiplexer {
    pub sessions: Vec<Session>,
    pub active_session: usize,
}

impl Multiplexer {
    /// Create multiplexer with first session
    pub fn new(session_name: String) -> Result<Self> {
        let initial_session = Session::new(session_name)?; // <- FIX

        Ok(Self {
            sessions: vec![initial_session],
            active_session: 0,
        })
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.sessions.get(self.active_session)
    }

    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(self.active_session)
    }

    pub fn create_session(&mut self, name: String) -> Result<()> {
        let session = Session::new(name)?;
        self.sessions.push(session);
        self.active_session = self.sessions.len() - 1;
        Ok(())
    }

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

    pub fn next_session(&mut self) {
        if !self.sessions.is_empty() {
            self.active_session =
                (self.active_session + 1) % self.sessions.len();
        }
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