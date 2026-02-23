use uuid::Uuid;
use anyhow::Result;

use crate::multiplexer::window::Window;
use crate::multiplexer::pane::Pane;

pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub windows: Vec<Window>,
    pub active_window: usize,
}

impl Session {

    pub fn new(name: String) -> Result<Self> {
        let initial_pane = Pane::spawn_shell()?;
        let initial_window = Window::new(initial_pane);

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            windows: vec![initial_window],
            active_window: 0,
        })
    }

    pub fn create_window(&mut self) -> Result<()> {
        let pane = Pane::spawn_shell()?;
        let window = Window::new(pane);
        self.windows.push(window);
        self.active_window = self.windows.len() - 1;
        Ok(())
    }

    pub fn close_window(&mut self, index: usize) {
        if self.windows.len() <= 1 {
            return; // never allow zero windows
        }

        if index < self.windows.len() {
            self.windows.remove(index);

            if self.active_window >= self.windows.len() {
                self.active_window = self.windows.len() - 1;
            }
        }
    }

    pub fn active_window_mut(&mut self) -> Option<&mut Window> {
        self.windows.get_mut(self.active_window)
    }

    pub fn next_window(&mut self) {
        if !self.windows.is_empty() {
            self.active_window =
                (self.active_window + 1) % self.windows.len();
        }
    }

    pub fn split_vertical(&mut self) -> Result<()> {
        if let Some(window) = self.active_window_mut() {
            window.split_vertical()?;
        }
        Ok(())
    }

    pub fn split_horizontal(&mut self) -> Result<()> {
        if let Some(window) = self.active_window_mut() {
            window.split_horizontal()?;
        }
        Ok(())
    }
}