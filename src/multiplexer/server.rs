pub struct Multiplexer{
    pub sessions : Vec<Session>
    pub active_session : usize
   
}

impl Multiplexer {
    pub fn new() -> Self;
    pub fn createsession(&mut self, name: String);
    pub fn kill_session(&mut self,id : Uuid);
    pub fn active_session_mut(&mut self) -> &mut Session;
    use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

impl Multiplexer {

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
}