//! The interactive session: your shell in a PTY, with Terrek listening.
//!
//! ```text
//!  stdin thread ──bytes──▶ main loop ──bytes──▶ PTY ──▶ shell
//!                            ▲    │
//!           records, exit ───┘    └─ palette (Ctrl+T), jobs, resize
//!  PTY ──▶ reader thread ──▶ Recorder (strip markers, build records) ──▶ stdout
//! ```
//!
//! Input is forwarded byte-for-byte, so every key, paste, and mouse event reaches
//! the shell unchanged. Only the palette key is intercepted, and not while a
//! full-screen program owns the screen.

pub mod models;
pub mod palette;
pub mod recorders;

use anyhow::{bail, Result};
use std::collections::VecDeque;
use std::io::{IsTerminal, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use crate::cli::{command_names, Cli};
use crate::commands::{self, Env};
use crate::config::Config;
use crate::db::{history, worker::HistoryWriter, CommandRecord};
use crate::pty::{default_shell, ShellProcess, SpawnOptions};
use crate::suggestion_engine::{ai_worker::spawn_job, SuggestionEngine};
use crate::terminal::{key_label, parse_key_spec, to_crlf, RawModeGuard};
use palette::{Action, Key, KeyDecoder, LineEditor};
use recorders::{FedPiece, Recorder};

const PROMPT: &str = "\x1b[1;36mterrek\x1b[0m \x1b[2m›\x1b[0m ";
const PROMPT_WIDTH: usize = 9;
/// Key sequence our shell hooks bind to "redraw the prompt".
const REDRAW_SEQ: &[u8] = b"\x1b[9999~";
/// Cap on output held back while the palette is open.
const MAX_HELD: usize = 4 * 1024 * 1024;

enum Msg {
    Input(Vec<u8>),
    Recorded(CommandRecord),
    JobDone(u64, Result<String>),
    ShellExited,
}

struct Shared {
    palette_open: bool,
    held: Vec<u8>,
    recorder: Recorder,
    ends_with_newline: bool,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Start the user's shell with Terrek attached and run until it exits.
pub fn run(config: Config) -> Result<()> {
    if std::env::var_os(crate::SESSION_ENV).is_some() {
        bail!("already inside a Terrek session (open the palette with its key instead)");
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("`terrek shell` needs an interactive terminal");
    }

    let palette_key = parse_key_spec(&config.shell.palette_key)?;
    let program = config.shell.program.clone().unwrap_or_else(default_shell);
    let session_id = uuid::Uuid::new_v4().to_string();
    let (cols, rows) = terminal_size().unwrap_or((80, 24));

    let shell = ShellProcess::spawn(SpawnOptions {
        program: &program,
        integration: config.shell.integration,
        session_id: &session_id,
        cols,
        rows,
    })?;
    let hooks_active = config.shell.integration && shell.kind.is_supported();

    let history = if config.history.enabled {
        match HistoryWriter::start(history::default_path()) {
            Ok(writer) => Some(writer),
            Err(e) => {
                eprintln!("terrek: history disabled for this session: {e:#}");
                None
            }
        }
    } else {
        None
    };

    let banner = if hooks_active {
        format!(
            "terrek · {} opens the palette · `terrek doctor` if anything looks off",
            key_label(palette_key)
        )
    } else {
        format!(
            "terrek · {} opens the palette · command recording needs zsh, bash, or fish ({} detected)",
            key_label(palette_key),
            shell.program
        )
    };
    println!("\x1b[2m{banner}\x1b[0m");

    let shared = Arc::new(Mutex::new(Shared {
        palette_open: false,
        held: Vec::new(),
        recorder: Recorder::new(
            session_id.clone(),
            config.history.store_output,
            config.history.max_output_bytes,
        ),
        ends_with_newline: true,
    }));

    let raw = RawModeGuard::enable()?;
    let (tx, rx) = channel::<Msg>();

    spawn_output_thread(
        shell.reader()?,
        Arc::clone(&shared),
        tx.clone(),
        config.shell.failure_hints.then(|| key_label(palette_key)),
    );
    let input = InputThread::spawn(tx.clone());

    let mut session = Session {
        config,
        shell,
        shared,
        tx,
        raw,
        input,
        history,
        palette_key,
        hooks_active,
        program,
        editor: LineEditor::default(),
        decoder: KeyDecoder::default(),
        engine: SuggestionEngine::new(command_names()),
        job: None,
        next_job: 0,
        produced_output: false,
        shown_tips: false,
        recent: VecDeque::new(),
        last_failed: None,
        size: (cols, rows),
    };

    loop {
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(Msg::Input(bytes)) => session.on_input(&bytes)?,
            Ok(Msg::Recorded(record)) => session.on_record(record),
            Ok(Msg::JobDone(id, result)) => session.on_job_done(id, result),
            Ok(Msg::ShellExited) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        session.check_resize();
    }

    drop(session.raw);
    let _ = session.shell.wait();
    Ok(())
}

fn spawn_output_thread(
    mut reader: Box<dyn Read + Send>,
    shared: Arc<Mutex<Shared>>,
    tx: Sender<Msg>,
    hint_key: Option<String>,
) {
    thread::Builder::new()
        .name("terrek-pty-output".into())
        .spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            let mut stdout = std::io::stdout();
            loop {
                let n = match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let mut guard = lock(&shared);
                let fed = guard.recorder.feed(&buf[..n]);
                let mut out = Vec::with_capacity(n);
                for piece in fed.pieces {
                    match piece {
                        FedPiece::Output(bytes) => out.extend_from_slice(&bytes),
                        FedPiece::Finished(record) => {
                            if let (Some(key), Some(code)) = (&hint_key, record.exit_code) {
                                // 130/131/148: interrupted, quit, or suspended on purpose.
                                let deliberate = matches!(code, 130 | 131 | 148);
                                if code != 0 && !deliberate && !guard.recorder.modes.alt_screen {
                                    let at_line_start = out
                                        .last()
                                        .map_or(guard.ends_with_newline, |b| *b == b'\n');
                                    if !at_line_start {
                                        out.extend_from_slice(b"\r\n");
                                    }
                                    out.extend_from_slice(
                                        format!("\x1b[2m↳ exit {code} · {key} then `fix` for help\x1b[0m\r\n")
                                            .as_bytes(),
                                    );
                                }
                            }
                            let _ = tx.send(Msg::Recorded(record));
                        }
                    }
                }
                if let Some(last) = out.last() {
                    guard.ends_with_newline = *last == b'\n';
                }
                if guard.palette_open {
                    if guard.held.len() + out.len() <= MAX_HELD {
                        guard.held.extend_from_slice(&out);
                    }
                } else {
                    let _ = stdout.write_all(&out);
                    let _ = stdout.flush();
                }
            }
            let _ = tx.send(Msg::ShellExited);
        })
        .expect("spawning output thread");
}

/// Reads stdin with a timeout so it can be paused while an editor or prompt owns the terminal.
struct InputThread {
    paused: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
}

impl InputThread {
    fn spawn(tx: Sender<Msg>) -> Self {
        let paused = Arc::new(AtomicBool::new(false));
        let idle = Arc::new(AtomicBool::new(false));
        let (p, i) = (Arc::clone(&paused), Arc::clone(&idle));
        thread::Builder::new()
            .name("terrek-stdin".into())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    if p.load(Ordering::SeqCst) {
                        i.store(true, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    i.store(false, Ordering::SeqCst);
                    let mut fds = libc::pollfd {
                        fd: libc::STDIN_FILENO,
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // SAFETY: one valid pollfd, and a buffer we own for the read.
                    let ready = unsafe { libc::poll(&mut fds, 1, 50) };
                    if ready <= 0 {
                        continue;
                    }
                    let n = unsafe {
                        libc::read(libc::STDIN_FILENO, buf.as_mut_ptr().cast(), buf.len())
                    };
                    if n <= 0 {
                        break;
                    }
                    if tx.send(Msg::Input(buf[..n as usize].to_vec())).is_err() {
                        break;
                    }
                }
            })
            .expect("spawning input thread");
        Self { paused, idle }
    }

    /// Stop reading stdin until the returned guard is dropped.
    fn pause(&self) -> impl Drop + '_ {
        self.paused.store(true, Ordering::SeqCst);
        while !self.idle.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(5));
        }
        scopeguard(move || self.paused.store(false, Ordering::SeqCst))
    }
}

struct Guard<F: FnMut()>(F);
impl<F: FnMut()> Drop for Guard<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}
fn scopeguard<F: FnMut()>(f: F) -> Guard<F> {
    Guard(f)
}

struct Session {
    config: Config,
    shell: ShellProcess,
    shared: Arc<Mutex<Shared>>,
    tx: Sender<Msg>,
    raw: RawModeGuard,
    input: InputThread,
    history: Option<HistoryWriter>,
    palette_key: u8,
    hooks_active: bool,
    program: String,
    editor: LineEditor,
    decoder: KeyDecoder,
    engine: SuggestionEngine,
    job: Option<u64>,
    next_job: u64,
    /// The palette printed something, so the shell's prompt needs redrawing on close.
    produced_output: bool,
    shown_tips: bool,
    /// Newest last.
    recent: VecDeque<CommandRecord>,
    last_failed: Option<CommandRecord>,
    size: (u16, u16),
}

impl Session {
    fn print(&self, text: &str) {
        let mut out = std::io::stdout();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    }

    fn on_input(&mut self, bytes: &[u8]) -> Result<()> {
        if lock(&self.shared).palette_open {
            return self.palette_input(bytes);
        }
        let intercept = !lock(&self.shared).recorder.modes.alt_screen;
        match bytes
            .iter()
            .position(|b| *b == self.palette_key)
            .filter(|_| intercept)
        {
            Some(pos) => {
                self.forward(&bytes[..pos])?;
                self.open_palette();
                self.palette_input(&bytes[pos + 1..])
            }
            None => self.forward(bytes),
        }
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.contains(&b'\r') {
            lock(&self.shared).recorder.user_submitted();
        }
        self.shell.write(bytes)
    }

    fn open_palette(&mut self) {
        lock(&self.shared).palette_open = true;
        self.produced_output = false;
        // Newline + up lands on the prompt's cursor even if the newline scrolled; save it there.
        let mut text = String::from("\n\x1b[A\x1b7\r\n");
        if !self.shown_tips {
            self.shown_tips = true;
            text.push_str(
                "\x1b[2mAsk anything, or: fix · history · search <text> · help · !<cmd> runs in the shell · Esc closes\x1b[0m\r\n",
            );
        }
        text.push_str(&self.editor.render(PROMPT, PROMPT_WIDTH, &self.engine));
        self.print(&text);
    }

    fn close_palette(&mut self) {
        self.editor.clear();
        self.job = None;
        let (held, at_prompt) = {
            let mut shared = lock(&self.shared);
            shared.palette_open = false;
            (std::mem::take(&mut shared.held), shared.recorder.at_prompt)
        };
        if self.produced_output {
            self.print("\r\x1b[K");
        } else {
            // Nothing new on screen: go back to where the cursor was and erase the palette.
            self.print("\x1b8\x1b[J");
        }
        if !held.is_empty() {
            let mut out = std::io::stdout();
            let _ = out.write_all(&held);
            let _ = out.flush();
        }
        if self.produced_output && at_prompt && self.hooks_active {
            let _ = self.shell.write(REDRAW_SEQ);
        }
    }

    fn redraw_palette(&self) {
        self.print(&self.editor.render(PROMPT, PROMPT_WIDTH, &self.engine));
    }

    fn palette_input(&mut self, bytes: &[u8]) -> Result<()> {
        for key in self.decoder.decode(bytes, self.palette_key) {
            if self.job.is_some() {
                if matches!(key, Key::Esc | Key::CtrlC | Key::Toggle) {
                    self.job = None;
                    self.print("\r\x1b[K\x1b[2m(cancelled)\x1b[0m\r\n");
                    self.redraw_palette();
                }
                continue;
            }
            match self.editor.handle(key, &self.engine) {
                Action::None => {}
                Action::Redraw => self.redraw_palette(),
                Action::Close => {
                    self.close_palette();
                    return Ok(());
                }
                Action::Cancel => {
                    if self.editor.text().is_empty() {
                        self.close_palette();
                        return Ok(());
                    }
                    self.editor.clear();
                    self.redraw_palette();
                }
                Action::Submit(text) => {
                    if !self.submit(text)? {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns false when the palette closed.
    fn submit(&mut self, text: String) -> Result<bool> {
        // Leave the submitted line on screen, without its ghost text.
        self.print(&format!("\r\x1b[K{PROMPT}{text}\r\n"));
        self.produced_output = true;
        self.engine.remember(&text);

        match text.as_str() {
            "" | "exit" | "quit" | "q" => {
                self.close_palette();
                return Ok(false);
            }
            "help" | "?" => {
                self.show(&palette_help());
                return Ok(true);
            }
            _ => {}
        }
        if let Some(command) = text.strip_prefix('!') {
            self.close_palette();
            self.forward(format!("{}\r", command.trim()).as_bytes())?;
            return Ok(false);
        }

        let command = match parse_palette_line(&text) {
            Parsed::Command(cmd) => cmd,
            Parsed::Message(msg) => {
                self.show(&msg);
                return Ok(true);
            }
        };

        let env = self.env();
        if commands::is_interactive(&command) {
            let result = {
                let _paused = self.input.pause();
                self.raw.suspend(|| commands::execute(command, &env, None))
            };
            self.show_result(result);
            return Ok(true);
        }

        self.next_job += 1;
        self.job = Some(self.next_job);
        self.print("\x1b[2m  working… (Esc to cancel)\x1b[0m");
        spawn_job(
            self.next_job,
            self.tx.clone(),
            move || commands::execute(command, &env, None),
            Msg::JobDone,
        );
        Ok(true)
    }

    fn on_job_done(&mut self, id: u64, result: Result<String>) {
        if self.job != Some(id) {
            return; // cancelled
        }
        self.job = None;
        self.print("\r\x1b[K");
        self.show_result(result);
    }

    fn show_result(&mut self, result: Result<String>) {
        match result {
            Ok(text) => self.show(&text),
            Err(e) => self.show(&format!("\x1b[31merror:\x1b[0m {e:#}")),
        }
    }

    fn show(&mut self, text: &str) {
        self.produced_output = true;
        let body = to_crlf(text.trim_end());
        self.print(&format!("{body}\r\n\r\n"));
        self.redraw_palette();
    }

    fn on_record(&mut self, record: CommandRecord) {
        if let Some(history) = &self.history {
            history.record(record.clone());
        }
        if record.failed() {
            self.last_failed = Some(record.clone());
        }
        self.recent.push_back(record);
        while self.recent.len() > 10 {
            self.recent.pop_front();
        }
    }

    fn env(&self) -> Env {
        let cwd = lock(&self.shared)
            .recorder
            .cwd
            .clone()
            .map(Into::into)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| ".".into());
        Env {
            config: self.config.clone(),
            cwd,
            shell: self.program.clone(),
            recent: self.recent.iter().rev().cloned().collect(),
            last_failed: self.last_failed.clone(),
        }
    }

    fn check_resize(&mut self) {
        if let Some(size) = terminal_size() {
            if size != self.size {
                self.size = size;
                let _ = self.shell.resize(size.0, size.1);
            }
        }
    }
}

/// Terminals attached without a size (some CI and remote setups) report 0x0.
fn terminal_size() -> Option<(u16, u16)> {
    crossterm::terminal::size()
        .ok()
        .filter(|(cols, rows)| *cols > 0 && *rows > 0)
}

enum Parsed {
    Command(crate::cli::Command),
    /// Help, usage errors, and the like: show and stay in the palette.
    Message(String),
}

/// Palette input uses the CLI grammar; anything that isn't a command is a question.
fn parse_palette_line(text: &str) -> Parsed {
    use clap::error::ErrorKind;
    use clap::Parser;

    let Ok(words) = shell_words::split(text) else {
        return Parsed::Command(ask(text));
    };
    let args = std::iter::once("terrek".to_string()).chain(words);
    match Cli::try_parse_from(args) {
        Ok(Cli { command: Some(cmd) }) => Parsed::Command(cmd),
        Ok(Cli { command: None }) => Parsed::Message(String::new()),
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::InvalidSubcommand | ErrorKind::UnknownArgument
            ) =>
        {
            Parsed::Command(ask(text))
        }
        Err(e) => Parsed::Message(e.render().ansi().to_string()),
    }
}

fn ask(text: &str) -> crate::cli::Command {
    crate::cli::Command::Ask {
        question: vec![text.to_string()],
    }
}

fn palette_help() -> String {
    use clap::CommandFactory;
    let mut help = Cli::command()
        .help_template("{subcommands}")
        .render_help()
        .ansi()
        .to_string();
    help.push_str(
        "\nIn the palette you can also:\n  <question>  anything that isn't a command is sent to the AI\n  !<cmd>      run a command in your shell\n  Tab / →     accept the suggestion · ↑/↓ previous entries · Esc closes\n",
    );
    help
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Command;

    #[test]
    fn natural_language_becomes_ask() {
        match parse_palette_line("why is my build slow?") {
            Parsed::Command(Command::Ask { question }) => {
                assert_eq!(question, ["why is my build slow?"])
            }
            _ => panic!("expected ask"),
        }
    }

    #[test]
    fn commands_parse_with_arguments() {
        assert!(matches!(
            parse_palette_line("fix"),
            Parsed::Command(Command::Fix)
        ));
        assert!(matches!(
            parse_palette_line("history -n 5 --failed"),
            Parsed::Command(Command::History {
                limit: 5,
                failed: true
            })
        ));
    }

    #[test]
    fn unbalanced_quotes_still_ask() {
        assert!(matches!(
            parse_palette_line("what's wrong"),
            Parsed::Command(Command::Ask { .. })
        ));
    }

    #[test]
    fn usage_errors_are_shown_not_sent_to_ai() {
        assert!(matches!(
            parse_palette_line("history -n abc"),
            Parsed::Message(_)
        ));
    }
}
