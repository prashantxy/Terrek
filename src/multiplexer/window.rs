use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::multiplexer::pane::Pane;

#[derive(Debug, Clone, Copy)]
pub enum SplitDirection {
    Vertical,
    Horizontal,
}

pub struct Window {
    pub panes: Vec<Pane>,
    pub active_pane: usize,
}

impl Window {
    pub fn new(initial_pane: Pane) -> Self {
        Self {
            panes: vec![initial_pane],
            active_pane: 0,
        }
    }

   
    pub fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(self.active_pane)
    }

    pub fn send_key(&mut self, key: KeyEvent) -> Result<()> {
        if let Some(pane) = self.active_pane_mut() {
            match key.code {
                KeyCode::Char(c) => {
                    pane.write(&[c as u8])?;
                }
                KeyCode::Enter => {
                    pane.write(b"\n")?;
                }
                KeyCode::Backspace => {
                    pane.write(&[0x7f])?;
                }
                KeyCode::Tab => {
                    pane.write(b"\t")?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Generic split handler
    pub fn split(&mut self, _direction: SplitDirection) -> Result<()> {
        let new_pane = Pane::spawn_shell()?;
        self.panes.push(new_pane);
        self.active_pane = self.panes.len() - 1;

        Ok(())
    }

    pub fn split_vertical(&mut self) -> Result<()> {
        self.split(SplitDirection::Vertical)
    }

    pub fn split_horizontal(&mut self) -> Result<()> {
        self.split(SplitDirection::Horizontal)
    }

    /// Cycle focus
    pub fn next_pane(&mut self) {
        if !self.panes.is_empty() {
            self.active_pane = (self.active_pane + 1) % self.panes.len();
        }
    }

    /// Close active pane safely
    pub fn close_active_pane(&mut self) {
        if self.panes.len() <= 1 {
            return; // never allow zero panes
        }

        self.panes.remove(self.active_pane);

        if self.active_pane >= self.panes.len() {
            self.active_pane = self.panes.len() - 1;
        }
    }

    /// Resize all panes equally (temporary simple layout)
    pub fn resize_all(&mut self, rows: u16, cols: u16) -> Result<()> {
        let pane_count = self.panes.len() as u16;

        if pane_count == 0 {
            return Ok(());
        }

        let each_rows = rows / pane_count;

        for pane in &mut self.panes {
            pane.resize(each_rows, cols)?;
        }

        Ok(())
    }

    /// Poll all panes for output
    pub fn poll_panes(&mut self) -> Result<()> {
        for pane in &mut self.panes {
            pane.read_output()?;
        }
        Ok(())
    }
}