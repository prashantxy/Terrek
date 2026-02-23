use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::multiplexer::server::Multiplexer;
use crate::multiplexer::commands::TerrekCommand;

pub struct InputHandler {
    prefix_mode: bool,
}

impl InputHandler {
    pub fn new() -> Self {
        Self { prefix_mode: false }
    }

    pub fn is_prefix_active(&self) -> bool {
        self.prefix_mode
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        mux: &mut Multiplexer,
    ) -> Result<()> {

        if self.prefix_mode {

            match key.code {
                KeyCode::Char('v') => {
                    mux.execute(TerrekCommand::SplitVertical)?;
                }
                KeyCode::Char('h') => {
                    mux.execute(TerrekCommand::SplitHorizontal)?;
                }
                KeyCode::Char('o') => {
                    mux.execute(TerrekCommand::NextPane)?;
                }
                KeyCode::Char('x') => {
                    mux.execute(TerrekCommand::ClosePane)?;
                }
                KeyCode::Char('c') => {
                    mux.execute(TerrekCommand::NewWindow)?;
                }
                KeyCode::Esc => {
                    // Cancel prefix
                }
                _ => {
                    // Unknown prefix command → keep prefix active
                    return Ok(());
                }
            }

            self.prefix_mode = false;
            return Ok(());
        }

        if key.code == KeyCode::Char('t')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.prefix_mode = true;
            return Ok(());
        }

        mux.send_input_to_active(key)?;

        Ok(())
    }
}