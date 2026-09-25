//! Turns the raw PTY byte stream into (a) the bytes to show the user, with our
//! markers removed, and (b) finished [`CommandRecord`]s.

use base64::Engine;
use std::time::Instant;

use super::models::{Piece, ShellEvent};
use crate::db::CommandRecord;

const MARKER_PREFIX: &[u8] = b"\x1b]7777;";
/// A marker larger than this is not ours; give up and pass it through.
const MAX_MARKER_LEN: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Normal,
    /// Matched `n` bytes of MARKER_PREFIX.
    Prefix(usize),
    Payload,
    /// Saw ESC inside a payload; `\` completes an ST terminator.
    PayloadEsc,
}

/// Streaming filter for `ESC ] 7777 ; ... (BEL | ESC \)`. Markers may be split
/// across reads; partial prefixes are held back until they can be decided.
pub struct MarkerParser {
    state: State,
    payload: Vec<u8>,
}

impl Default for MarkerParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkerParser {
    pub fn new() -> Self {
        Self {
            state: State::Normal,
            payload: Vec::new(),
        }
    }

    pub fn feed(&mut self, data: &[u8]) -> Vec<Piece> {
        let mut pieces = Vec::new();
        let mut out = Vec::with_capacity(data.len());
        for &byte in data {
            self.step(byte, &mut out, &mut pieces);
        }
        if !out.is_empty() {
            pieces.push(Piece::Output(out));
        }
        pieces
    }

    fn step(&mut self, byte: u8, out: &mut Vec<u8>, pieces: &mut Vec<Piece>) {
        match self.state {
            State::Normal => {
                if byte == MARKER_PREFIX[0] {
                    self.state = State::Prefix(1);
                } else {
                    out.push(byte);
                }
            }
            State::Prefix(matched) => {
                if byte == MARKER_PREFIX[matched] {
                    self.state = if matched + 1 == MARKER_PREFIX.len() {
                        self.payload.clear();
                        State::Payload
                    } else {
                        State::Prefix(matched + 1)
                    };
                } else {
                    // Not ours: release the held prefix, then reconsider this byte.
                    out.extend_from_slice(&MARKER_PREFIX[..matched]);
                    self.state = State::Normal;
                    self.step(byte, out, pieces);
                }
            }
            State::Payload => match byte {
                0x07 => self.finish(out, pieces),
                0x1b => self.state = State::PayloadEsc,
                _ if self.payload.len() >= MAX_MARKER_LEN => self.abandon(out, byte),
                _ => self.payload.push(byte),
            },
            State::PayloadEsc => {
                if byte == b'\\' {
                    self.finish(out, pieces);
                } else {
                    self.payload.push(0x1b);
                    self.abandon(out, byte);
                }
            }
        }
    }

    fn finish(&mut self, out: &mut Vec<u8>, pieces: &mut Vec<Piece>) {
        self.state = State::Normal;
        if let Some(event) = parse_payload(&self.payload) {
            if !out.is_empty() {
                pieces.push(Piece::Output(std::mem::take(out)));
            }
            pieces.push(Piece::Event(event));
        }
    }

    fn abandon(&mut self, out: &mut Vec<u8>, byte: u8) {
        out.extend_from_slice(MARKER_PREFIX);
        out.append(&mut self.payload);
        out.push(byte);
        self.state = State::Normal;
    }
}

fn parse_payload(payload: &[u8]) -> Option<ShellEvent> {
    let text = std::str::from_utf8(payload).ok()?;
    let mut fields = text.split(';');
    match fields.next()? {
        "start" => Some(ShellEvent::Started {
            command: decode_b64(fields.next().unwrap_or(""))?,
        }),
        "done" => Some(ShellEvent::Finished {
            exit_code: fields.next()?.trim().parse().ok()?,
            command: fields.next().and_then(decode_b64).filter(|s| !s.is_empty()),
            cwd: fields.next().and_then(decode_b64).filter(|s| !s.is_empty()),
        }),
        "prompt" => Some(ShellEvent::Prompt),
        _ => None,
    }
}

fn decode_b64(text: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Tracks terminal modes the running program turns on, by watching its output.
#[derive(Debug, Default)]
pub struct TerminalModes {
    /// A full-screen app (vim, less, htop) is using the alternate screen.
    pub alt_screen: bool,
    /// Carried over between reads so a sequence split in two is still seen.
    tail: Vec<u8>,
}

impl TerminalModes {
    pub fn observe(&mut self, output: &[u8]) {
        const SEQUENCES: &[(&[u8], bool)] = &[
            (b"\x1b[?1049h", true),
            (b"\x1b[?1049l", false),
            (b"\x1b[?1047h", true),
            (b"\x1b[?1047l", false),
            (b"\x1b[?47h", true),
            (b"\x1b[?47l", false),
        ];
        let mut window = std::mem::take(&mut self.tail);
        window.extend_from_slice(output);

        // The last occurrence decides the current state.
        let mut latest: Option<(usize, bool)> = None;
        for (seq, on) in SEQUENCES {
            if let Some(pos) = rfind(&window, seq) {
                if latest.is_none_or(|(p, _)| pos > p) {
                    latest = Some((pos, *on));
                }
            }
        }
        if let Some((_, on)) = latest {
            self.alt_screen = on;
        }

        let keep = window.len().min(8);
        self.tail = window[window.len() - keep..].to_vec();
    }
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// The output of the command currently running, capped to the most recent bytes.
#[derive(Debug)]
pub struct OutputCapture {
    buf: Vec<u8>,
    cap: usize,
}

impl OutputCapture {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: Vec::new(),
            cap,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        if self.cap == 0 {
            return;
        }
        self.buf.extend_from_slice(bytes);
        // Trim lazily so steady output doesn't shift the buffer on every read.
        if self.buf.len() > self.cap * 2 {
            let excess = self.buf.len() - self.cap;
            self.buf.drain(..excess);
        }
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// Plain text: escape sequences removed, `\r`-overwritten progress lines collapsed.
    pub fn take_text(&mut self) -> String {
        let start = self.buf.len().saturating_sub(self.cap);
        // Collapse `\r` redraws before stripping escapes, which also removes `\r`.
        let lines: Vec<String> = self.buf[start..]
            .split(|b| *b == b'\n')
            .map(|line| {
                let line = line.strip_suffix(b"\r").unwrap_or(line);
                let visible = line.rsplit(|b| *b == b'\r').next().unwrap_or(line);
                String::from_utf8_lossy(&strip_ansi_escapes::strip(visible)).into_owned()
            })
            .collect();
        self.buf.clear();
        lines.join("\n").trim().to_string()
    }
}

/// Everything the reader thread needs to follow the shell: markers, modes, and
/// the command in flight.
pub struct Recorder {
    parser: MarkerParser,
    pub modes: TerminalModes,
    capture: OutputCapture,
    in_flight: Option<(String, Instant)>,
    session_id: String,
    store_output: bool,
    /// True while the shell waits at its prompt.
    pub at_prompt: bool,
    /// Last directory the shell reported.
    pub cwd: Option<String>,
}

/// What one chunk of PTY output turned into.
#[derive(Debug, Default)]
pub struct Fed {
    /// Bytes to display, split where hints may be inserted.
    pub pieces: Vec<FedPiece>,
}

#[derive(Debug)]
pub enum FedPiece {
    Output(Vec<u8>),
    Finished(CommandRecord),
}

impl Recorder {
    pub fn new(session_id: String, store_output: bool, max_output_bytes: usize) -> Self {
        Self {
            parser: MarkerParser::new(),
            modes: TerminalModes::default(),
            capture: OutputCapture::new(max_output_bytes),
            in_flight: None,
            session_id,
            store_output,
            at_prompt: false,
            cwd: None,
        }
    }

    /// Called when the user presses Enter at the prompt. Bash has no preexec hook, so this
    /// is where its command output starts.
    pub fn user_submitted(&mut self) {
        if self.at_prompt {
            self.at_prompt = false;
            self.capture.clear();
        }
    }

    pub fn feed(&mut self, data: &[u8]) -> Fed {
        let mut fed = Fed::default();
        for piece in self.parser.feed(data) {
            match piece {
                Piece::Output(bytes) => {
                    self.modes.observe(&bytes);
                    self.capture.push(&bytes);
                    fed.pieces.push(FedPiece::Output(bytes));
                }
                Piece::Event(ShellEvent::Started { command }) => {
                    self.at_prompt = false;
                    self.capture.clear();
                    self.in_flight = Some((command, Instant::now()));
                }
                Piece::Event(ShellEvent::Finished {
                    exit_code,
                    command,
                    cwd,
                }) => {
                    let in_flight = self.in_flight.take();
                    let duration_ms = in_flight
                        .as_ref()
                        .map(|(_, started)| started.elapsed().as_millis() as i64);
                    let command = command.or(in_flight.map(|(c, _)| c)).unwrap_or_default();
                    let mut output = self.capture.take_text();
                    // Without a start marker (bash) the capture begins with the echo of the typed
                    // command, twice when typed ahead of the prompt (tty echo, then readline's).
                    if duration_ms.is_none() && !command.trim().is_empty() {
                        let echoes = output
                            .lines()
                            .take_while(|line| line.trim_end().ends_with(command.trim()))
                            .count();
                        if echoes > 0 {
                            output = output.lines().skip(echoes).collect::<Vec<_>>().join("\n");
                        }
                    }
                    if cwd.is_some() {
                        self.cwd.clone_from(&cwd);
                    }
                    // A leading space means "don't record this", as with HISTCONTROL=ignorespace.
                    if command.trim().is_empty() || command.starts_with(' ') {
                        continue;
                    }
                    fed.pieces.push(FedPiece::Finished(CommandRecord {
                        id: None,
                        session_id: Some(self.session_id.clone()),
                        command: command.trim().to_string(),
                        output: (self.store_output && !output.is_empty()).then_some(output),
                        exit_code: Some(exit_code),
                        cwd,
                        duration_ms,
                        timestamp: chrono::Utc::now().timestamp(),
                    }));
                }
                Piece::Event(ShellEvent::Prompt) => {
                    self.at_prompt = true;
                    self.capture.clear();
                }
            }
        }
        fed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s)
    }

    fn output_of(pieces: &[Piece]) -> Vec<u8> {
        pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Output(b) => Some(b.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn events_of(pieces: &[Piece]) -> Vec<ShellEvent> {
        pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Event(e) => Some(e.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn strips_marker_and_keeps_surrounding_output() {
        let mut parser = MarkerParser::new();
        let input = format!("before\x1b]7777;start;{}\x07after", b64("ls -la"));
        let pieces = parser.feed(input.as_bytes());
        assert_eq!(output_of(&pieces), b"beforeafter");
        assert_eq!(
            events_of(&pieces),
            vec![ShellEvent::Started {
                command: "ls -la".into()
            }]
        );
        // Event sits between the two outputs.
        assert!(matches!(pieces[1], Piece::Event(_)));
    }

    #[test]
    fn marker_split_across_every_byte_boundary() {
        let input = format!("x\x1b]7777;done;2;{};{}\x07y", b64("make"), b64("/src"));
        let bytes = input.as_bytes();
        for split in 0..bytes.len() {
            let mut parser = MarkerParser::new();
            let mut pieces = parser.feed(&bytes[..split]);
            pieces.extend(parser.feed(&bytes[split..]));
            assert_eq!(output_of(&pieces), b"xy", "split at {split}");
            assert_eq!(
                events_of(&pieces),
                vec![ShellEvent::Finished {
                    exit_code: 2,
                    command: Some("make".into()),
                    cwd: Some("/src".into())
                }],
                "split at {split}"
            );
        }
    }

    #[test]
    fn other_escape_sequences_pass_through_untouched() {
        let mut parser = MarkerParser::new();
        let input =
            b"\x1b[31mred\x1b[0m \x1b]0;title\x07 \x1b]777;x\x07 \x1b\x1b]7777;prompt\x1b\\";
        let pieces = parser.feed(input);
        assert_eq!(
            output_of(&pieces),
            b"\x1b[31mred\x1b[0m \x1b]0;title\x07 \x1b]777;x\x07 \x1b".to_vec()
        );
        assert_eq!(events_of(&pieces), vec![ShellEvent::Prompt]);
    }

    #[test]
    fn unknown_marker_kind_is_dropped_silently() {
        let mut parser = MarkerParser::new();
        let pieces = parser.feed(b"a\x1b]7777;mystery\x07b");
        assert_eq!(output_of(&pieces), b"ab");
        assert!(events_of(&pieces).is_empty());
    }

    #[test]
    fn tracks_alt_screen_even_when_split() {
        let mut modes = TerminalModes::default();
        modes.observe(b"hello\x1b[?10");
        modes.observe(b"49h vim");
        assert!(modes.alt_screen);
        modes.observe(b"\x1b[?1049l");
        assert!(!modes.alt_screen);
    }

    #[test]
    fn capture_strips_ansi_and_progress_redraws() {
        let mut cap = OutputCapture::new(1024);
        cap.push(b"\x1b[32mok\x1b[0m\r\n10%\r50%\r100%\r\nerror: boom\r\n");
        assert_eq!(cap.take_text(), "ok\n100%\nerror: boom");
    }

    #[test]
    fn capture_keeps_only_the_tail() {
        let mut cap = OutputCapture::new(4);
        cap.push(b"0123456789");
        assert_eq!(cap.take_text(), "6789");
    }

    #[test]
    fn zsh_style_sequence_produces_a_record() {
        let mut rec = Recorder::new("s1".into(), true, 1024);
        rec.feed(b"\x1b]7777;prompt\x07$ ");
        assert!(rec.at_prompt);
        let start = format!("\x1b]7777;start;{}\x07", b64("cargo build"));
        rec.feed(start.as_bytes());
        let done = format!("error[E0425]\r\n\x1b]7777;done;101;;{}\x07", b64("/proj"));
        let fed = rec.feed(done.as_bytes());

        let record = fed
            .pieces
            .iter()
            .find_map(|p| match p {
                FedPiece::Finished(r) => Some(r.clone()),
                _ => None,
            })
            .expect("a record");
        assert_eq!(record.command, "cargo build");
        assert_eq!(record.exit_code, Some(101));
        assert_eq!(record.output.as_deref(), Some("error[E0425]"));
        assert_eq!(record.cwd.as_deref(), Some("/proj"));
        assert!(record.duration_ms.is_some());
        assert_eq!(rec.cwd.as_deref(), Some("/proj"));
    }

    #[test]
    fn bash_style_done_carries_the_command() {
        let mut rec = Recorder::new("s".into(), false, 1024);
        rec.feed(b"\x1b]7777;prompt\x07");
        rec.user_submitted();
        let done = format!("out\r\n\x1b]7777;done;0;{};\x07", b64("echo hi"));
        let fed = rec.feed(done.as_bytes());
        let record = fed
            .pieces
            .iter()
            .find_map(|p| match p {
                FedPiece::Finished(r) => Some(r.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(record.command, "echo hi");
        assert_eq!(record.output, None, "store_output is off");
        assert_eq!(record.duration_ms, None);
    }

    #[test]
    fn bash_echo_of_the_command_is_dropped_from_output() {
        let mut rec = Recorder::new("s".into(), true, 1024);
        rec.feed(b"\x1b]7777;prompt\x07");
        rec.user_submitted();
        let done = format!("echo hi\r\nhi\r\n\x1b]7777;done;0;{};\x07", b64("echo hi"));
        let fed = rec.feed(done.as_bytes());
        let record = fed
            .pieces
            .iter()
            .find_map(|p| match p {
                FedPiece::Finished(r) => Some(r.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(record.output.as_deref(), Some("hi"));
        assert_eq!(record.duration_ms, None);
    }

    #[test]
    fn space_prefixed_commands_are_not_recorded() {
        let mut rec = Recorder::new("s".into(), true, 1024);
        let input = format!(
            "\x1b]7777;start;{}\x07\x1b]7777;done;0;;\x07",
            b64(" export TOKEN=secret")
        );
        let fed = rec.feed(input.as_bytes());
        assert!(!fed
            .pieces
            .iter()
            .any(|p| matches!(p, FedPiece::Finished(_))));
    }
}
