use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver as CbReceiver, Sender as CbSender};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

use uuid::Uuid;

mod ai;
mod db;
mod commands;
mod context;
mod config;

use db::worker::{start_db_worker, DbEvent};
use commands::{handle_command, TerrekAction};
use context::state::ContextState;

#[derive(Debug)]
enum Mode {
    Shell,
    Terrek,
}

struct SuggestionEngine {
    static_commands: Vec<String>,
    history: Vec<String>,
    ai_cache: Vec<String>,
}

impl SuggestionEngine {
    fn suggest(&self, input: &str) -> Vec<String> {
        if input.is_empty() {
            return vec![];
        }

        let matcher = SkimMatcherV2::default();
        let mut scored = Vec::new();

        let sources = self
            .static_commands
            .iter()
            .chain(self.history.iter())
            .chain(self.ai_cache.iter());

        for item in sources {
            if let Some(score) = matcher.fuzzy_match(item, input) {
                scored.push((score, item.clone()));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, s)| s).take(5).collect()
    }
}

struct App {
    mode: Mode,
    shell_input_buffer: String,
    terrek_buffer: String,
    current_output: String,

    engine: Arc<Mutex<SuggestionEngine>>,
    context: Arc<Mutex<ContextState>>,

    ai_tx: CbSender<String>,
    ai_out_rx: CbReceiver<Vec<String>>,

    writer: Box<dyn Write + Send>,
}

impl App {
    fn new(writer: Box<dyn Write + Send>) -> Self {
        let context = Arc::new(Mutex::new(ContextState::new()));

        let (ai_tx, ai_rx) = unbounded();
        let (ai_out_tx, ai_out_rx) = unbounded();

        let engine = Arc::new(Mutex::new(SuggestionEngine {
            static_commands: vec![
                "terrek hello".into(),
                "terrek time".into(),
                "terrek history".into(),
                "terrek ai".into(),
                "terrek exit".into(),
            ],
            history: vec![],
            ai_cache: vec![],
        }));

        start_ai_worker(ai_rx, ai_out_tx, context.clone());

        Self {
            mode: Mode::Shell,
            shell_input_buffer: String::new(),
            terrek_buffer: String::new(),
            current_output: String::new(),
            engine,
            context,
            ai_tx,
            ai_out_rx,
            writer,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {

            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::Terrek;
                self.terrek_buffer.clear();
                println!("\n-- TERREK MODE --");
                self.draw_prompt();
            }

            KeyCode::Char(c) => match self.mode {
                Mode::Shell => {
                    self.shell_input_buffer.push(c);
                    self.writer.write_all(&[c as u8])?;
                    self.writer.flush()?;
                }
                Mode::Terrek => {
                    self.terrek_buffer.push(c);
                    self.ai_tx.send(self.terrek_buffer.clone()).ok();
                    self.draw_prompt();
                }
            },

            KeyCode::Backspace => match self.mode {
                Mode::Shell => {
                    self.shell_input_buffer.pop();
                    self.writer.write_all(b"\x7f")?;
                    self.writer.flush()?;
                }
                Mode::Terrek => {
                    self.terrek_buffer.pop();
                    self.draw_prompt();
                }
            },

            KeyCode::Enter => match self.mode {
                Mode::Shell => {
                    let cmd = format!("{}\n", self.shell_input_buffer);
                    self.writer.write_all(cmd.as_bytes())?;
                    self.writer.flush()?;
                    self.shell_input_buffer.clear();
                }

                Mode::Terrek => {
                    println!();
                    let input = self.terrek_buffer.trim();

                    {
                        let mut eng = self.engine.lock().unwrap();
                        eng.history.push(input.to_string());
                    }

                    if input.starts_with("terrek ") {
                        let stripped =
                            input.trim_start_matches("terrek ").trim();

                        let ctx = self.context.lock().unwrap();
                        let action = handle_command(&ctx, stripped)?;

                        match action {
                            TerrekAction::Output(text) => {
                                println!("[Terrek] {}", text);
                            }
                        }
                    } else {
                        self.writer
                            .write_all(format!("{}\n", input).as_bytes())?;
                        self.writer.flush()?;
                    }

                    self.terrek_buffer.clear();
                    self.draw_prompt();
                }
            },

            KeyCode::Esc => match self.mode {
                Mode::Terrek => {
                    self.mode = Mode::Shell;
                    println!("\n-- SHELL MODE --");
                }
                Mode::Shell => return Ok(false),
            },

            _ => {}
        }

        Ok(true)
    }

    fn draw_prompt(&self) {
        let suggestions = {
            let eng = self.engine.lock().unwrap();
            eng.suggest(&self.terrek_buffer)
        };

        print!("\r\x1B[K[Terrek] > {}", self.terrek_buffer);

        if let Some(s) = suggestions.first() {
            if s.starts_with(&self.terrek_buffer) {
                let ghost = &s[self.terrek_buffer.len()..];
                print!("\x1B[90m{}\x1B[0m", ghost);
            }
        }

        std::io::stdout().flush().ok();
    }

    fn handle_ai_updates(&mut self) {
        if let Ok(ai_suggestions) = self.ai_out_rx.try_recv() {
            let mut eng = self.engine.lock().unwrap();
            eng.ai_cache = ai_suggestions;
        }
    }
}

fn start_ai_worker(
    rx: CbReceiver<String>,
    tx: CbSender<Vec<String>>,
    context: Arc<Mutex<ContextState>>,
) {
    thread::spawn(move || {
        for input in rx {
            if input.len() < 3 {
                continue;
            }

            thread::sleep(Duration::from_millis(400));

            let prompt = format!(
                "User typed: \"{}\". Suggest 5 Terrek commands.",
                input
            );

            let ctx = context.lock().unwrap();

            if let Ok(resp) = ai::gemini::ask_gemini(&ctx, &prompt) {
                let suggestions = resp
                    .lines()
                    .map(|l| l.trim().to_string())
                    .collect();

                tx.send(suggestions).ok();
            }
        }
    });
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let _cleanup = scopeguard::guard((), |_| {
        disable_raw_mode().ok();
    });

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or("/bin/bash".to_string());
    let cmd = CommandBuilder::new(shell);
    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    let mut app = App::new(Box::new(writer));

    println!("You are in SHELL session. Press Ctrl+T for TERREK mode.");

    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            if let Ok(n) = reader.read(&mut buffer) {
                if n == 0 { break; }
                print!("{}", String::from_utf8_lossy(&buffer[..n]));
                std::io::stdout().flush().ok();
            }
        }
    });

    loop {
        app.handle_ai_updates();

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if !app.handle_key(key)? {
                    break;
                }
            }
        }
    }

    Ok(())
}