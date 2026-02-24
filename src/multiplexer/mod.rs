use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::multiplexer::session::Session;
use crate::multiplexer::command::TerrekCommand;

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
        let initial_session = Session::new(session_name)?;

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

    /// Send key to active pane
    pub fn send_key(&mut self, key: KeyEvent) -> Result<()> {
        if let Some(session) = self.active_session_mut() {
            if let Some(window) = session.active_window_mut() {
                window.send_key(key)?;
            }
        }
        Ok(())
    }

    /// Execute high-level command (splits, navigation, etc.)
    pub fn execute(&mut self, cmd: TerrekCommand) -> Result<()> {
    match cmd {
        TerrekCommand::SplitVertical => {
            if let Some(session) = self.active_session_mut() {
                session.split_vertical()?;
            }
        }

        TerrekCommand::SplitHorizontal => {
            if let Some(session) = self.active_session_mut() {
                session.split_horizontal()?;
            }
        }

        TerrekCommand::NextPane => {
            if let Some(session) = self.active_session_mut() {
                if let Some(window) = session.active_window_mut() {
                    window.next_pane();
                }
            }
        }

        TerrekCommand::ClosePane => {
            if let Some(session) = self.active_session_mut() {
                if let Some(window) = session.active_window_mut() {
                    window.close_active_pane();
                }
            }
        }

       
        _ => {
            
        }
    }

    Ok(())
}

    /// Poll active session panes
    pub fn poll(&mut self) -> Result<()> {
        if let Some(session) = self.active_session_mut() {
            if let Some(window) = session.active_window_mut() {
                window.poll_panes()?;
            }
        }
        Ok(())
    }
}