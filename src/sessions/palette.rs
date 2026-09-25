//! The one-line prompt that opens on the palette key.

use crate::suggestion_engine::SuggestionEngine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Delete,
    Enter,
    Esc,
    Tab,
    Left,
    Right,
    Home,
    End,
    Up,
    Down,
    CtrlC,
    CtrlD,
    CtrlU,
    CtrlW,
    /// The palette key itself.
    Toggle,
}

/// Decodes raw terminal input bytes into palette keys.
#[derive(Default)]
pub struct KeyDecoder {
    utf8: Vec<u8>,
}

impl KeyDecoder {
    pub fn decode(&mut self, bytes: &[u8], toggle: u8) -> Vec<Key> {
        let mut keys = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            i += 1;
            if !self.utf8.is_empty() || b >= 0x80 {
                self.utf8.push(b);
                match std::str::from_utf8(&self.utf8) {
                    Ok(s) => {
                        keys.extend(s.chars().map(Key::Char));
                        self.utf8.clear();
                    }
                    Err(e) if e.error_len().is_some() => self.utf8.clear(),
                    Err(_) => {} // incomplete sequence; wait for more bytes
                }
                continue;
            }
            if b == toggle {
                keys.push(Key::Toggle);
                continue;
            }
            match b {
                0x1b => {
                    // A lone ESC at the end of a read is the Escape key; otherwise a sequence.
                    match bytes.get(i) {
                        None => keys.push(Key::Esc),
                        Some(b'[') | Some(b'O') => {
                            let start = i + 1;
                            let end = bytes[start..]
                                .iter()
                                .position(|c| (0x40..=0x7e).contains(c))
                                .map(|p| start + p);
                            match end {
                                Some(end) => {
                                    if let Some(key) = csi_key(&bytes[start..=end]) {
                                        keys.push(key);
                                    }
                                    i = end + 1;
                                }
                                None => i = bytes.len(),
                            }
                        }
                        Some(_) => i += 1, // Alt+key: ignored
                    }
                }
                0x7f | 0x08 => keys.push(Key::Backspace),
                b'\r' | b'\n' => keys.push(Key::Enter),
                b'\t' => keys.push(Key::Tab),
                0x03 => keys.push(Key::CtrlC),
                0x04 => keys.push(Key::CtrlD),
                0x15 => keys.push(Key::CtrlU),
                0x17 => keys.push(Key::CtrlW),
                0x01 => keys.push(Key::Home),
                0x05 => keys.push(Key::End),
                0x20..=0x7e => keys.push(Key::Char(b as char)),
                _ => {}
            }
        }
        keys
    }
}

fn csi_key(seq: &[u8]) -> Option<Key> {
    Some(match seq {
        b"A" => Key::Up,
        b"B" => Key::Down,
        b"C" => Key::Right,
        b"D" => Key::Left,
        b"H" | b"1~" | b"7~" => Key::Home,
        b"F" | b"4~" | b"8~" => Key::End,
        b"3~" => Key::Delete,
        _ => return None, // includes bracketed-paste markers 200~/201~
    })
}

/// What the session should do after a key.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Redraw,
    Submit(String),
    Close,
    Cancel,
}

#[derive(Default)]
pub struct LineEditor {
    chars: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    history_pos: Option<usize>,
}

impl LineEditor {
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
        self.history_pos = None;
    }

    fn set(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
    }

    pub fn handle(&mut self, key: Key, engine: &SuggestionEngine) -> Action {
        match key {
            Key::Char(c) => {
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
            }
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            Key::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            Key::Left => self.cursor = self.cursor.saturating_sub(1),
            Key::Right if self.cursor == self.chars.len() => {
                // Right at the end accepts the suggestion, like fish.
                if let Some(s) = engine.complete(&self.text()) {
                    self.set(&s);
                }
            }
            Key::Right => self.cursor += 1,
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.chars.len(),
            Key::Tab => {
                if let Some(s) = engine.complete(&self.text()) {
                    self.set(&s);
                }
            }
            Key::CtrlU => self.clear(),
            Key::CtrlW => {
                let mut start = self.cursor;
                while start > 0 && self.chars[start - 1] == ' ' {
                    start -= 1;
                }
                while start > 0 && self.chars[start - 1] != ' ' {
                    start -= 1;
                }
                self.chars.drain(start..self.cursor);
                self.cursor = start;
            }
            Key::Up if !self.history.is_empty() => {
                let pos = match self.history_pos {
                    None => self.history.len() - 1,
                    Some(p) => p.saturating_sub(1),
                };
                self.history_pos = Some(pos);
                let entry = self.history[pos].clone();
                self.set(&entry);
            }
            Key::Down => match self.history_pos {
                Some(p) if p + 1 < self.history.len() => {
                    self.history_pos = Some(p + 1);
                    let entry = self.history[p + 1].clone();
                    self.set(&entry);
                }
                Some(_) => self.clear(),
                None => {}
            },
            Key::Enter => {
                let text = self.text().trim().to_string();
                if !text.is_empty() && self.history.last() != Some(&text) {
                    self.history.push(text.clone());
                }
                self.clear();
                return Action::Submit(text);
            }
            Key::CtrlD if self.chars.is_empty() => return Action::Close,
            Key::Esc | Key::Toggle => return Action::Close,
            Key::CtrlC => return Action::Cancel,
            _ => return Action::None,
        }
        Action::Redraw
    }

    /// Terminal bytes that redraw the palette line, with the suggestion as dim ghost text.
    pub fn render(&self, prompt: &str, prompt_width: usize, engine: &SuggestionEngine) -> String {
        let text = self.text();
        let ghost = if self.cursor == self.chars.len() {
            engine
                .complete(&text)
                .and_then(|s| s.strip_prefix(text.as_str()).map(str::to_string))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let column = prompt_width + self.cursor;
        format!("\r\x1b[K{prompt}{text}\x1b[2m{ghost}\x1b[0m\r\x1b[{column}C")
            .replace("\x1b[0C", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> SuggestionEngine {
        SuggestionEngine::new(vec!["fix".into(), "history".into()])
    }

    #[test]
    fn decodes_mixed_input() {
        let mut d = KeyDecoder::default();
        let keys = d.decode(b"a\x1b[D\x7f\r\x14", 0x14);
        assert_eq!(
            keys,
            vec![
                Key::Char('a'),
                Key::Left,
                Key::Backspace,
                Key::Enter,
                Key::Toggle
            ]
        );
        assert_eq!(d.decode(b"\x1b", 0x14), vec![Key::Esc]);
        assert_eq!(
            d.decode(b"\x1b[200~hi\x1b[201~", 0x14),
            vec![Key::Char('h'), Key::Char('i')]
        );
    }

    #[test]
    fn decodes_utf8_split_across_reads() {
        let mut d = KeyDecoder::default();
        let bytes = "é".as_bytes();
        assert!(d.decode(&bytes[..1], 0x14).is_empty());
        assert_eq!(d.decode(&bytes[1..], 0x14), vec![Key::Char('é')]);
    }

    #[test]
    fn editing_and_submit() {
        let e = engine();
        let mut ed = LineEditor::default();
        for c in "fxi".chars() {
            ed.handle(Key::Char(c), &e);
        }
        ed.handle(Key::Left, &e);
        ed.handle(Key::Backspace, &e); // removes the 'x'
        ed.handle(Key::End, &e);
        assert_eq!(ed.text(), "fi");
        ed.handle(Key::Tab, &e);
        assert_eq!(ed.text(), "fix");
        assert_eq!(ed.handle(Key::Enter, &e), Action::Submit("fix".into()));
        assert_eq!(ed.text(), "");
        ed.handle(Key::Up, &e);
        assert_eq!(ed.text(), "fix");
    }

    #[test]
    fn ctrl_w_deletes_previous_word() {
        let e = engine();
        let mut ed = LineEditor::default();
        for c in "ask why  ".chars() {
            ed.handle(Key::Char(c), &e);
        }
        ed.handle(Key::CtrlW, &e);
        assert_eq!(ed.text(), "ask ");
    }

    #[test]
    fn render_shows_ghost_suggestion() {
        let e = engine();
        let mut ed = LineEditor::default();
        ed.handle(Key::Char('h'), &e);
        let out = ed.render("> ", 2, &e);
        assert!(out.contains("> h\x1b[2mistory"));
    }
}
