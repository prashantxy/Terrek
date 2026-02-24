use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver as CbReceiver, Sender as CbSender};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use multiplexer::Multiplexer;
use multiplexer::command::TerrekCommand;

mod ai;
mod db;
mod commands;
mod context;
mod config;
mod renderer;
mod multiplexer;

use commands::{handle_command, TerrekAction};
use context::state::ContextState;

#[derive(Debug, PartialEq)] // Needed for comparisons
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
    shell_buffer: String,
    terrek_buffer: String,

    engine: Arc<Mutex<SuggestionEngine>>,
    context: Arc<Mutex<ContextState>>,

    ai_tx: CbSender<String>,
    ai_out_rx: CbReceiver<Vec<String>>,

    mux: Multiplexer,
    mux_prefix: bool,
}

impl App {
    fn new() -> Result<Self> {
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

        // Start AI worker that updates suggestions live
        start_ai_worker(ai_rx, ai_out_tx.clone(), context.clone());

        // Start background thread to listen for AI outputs continuously
        let engine_clone = engine.clone();
        thread::spawn(move || {
            for suggestions in ai_out_rx {
                let mut eng = engine_clone.lock().unwrap();
                eng.ai_cache = suggestions;
            }
        });

        Ok(Self {
            mode: Mode::Shell,
            shell_buffer: String::new(),
            terrek_buffer: String::new(),
            engine,
            context,
            ai_tx,
            ai_out_rx, // still useful for optional polling
            mux: Multiplexer::new("main".to_string())?,
            mux_prefix: false,
        })
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.mode {
            Mode::Shell => self.handle_shell_key(key)?,
            Mode::Terrek => self.handle_terrek_key(key)?,
        }
        Ok(true)
    }

    fn handle_shell_key(&mut self, key: KeyEvent) -> Result<()> {
        if is_ctrl_t(&key) {
            self.mode = Mode::Terrek;
            self.terrek_buffer.clear();
            println!("\n-- TERREK MODE --");
            return Ok(());
        }

        match key.code {
            KeyCode::Char(c) => {
                self.shell_buffer.push(c);
                self.mux.send_key(key)?;
            }
            KeyCode::Enter => {
                self.mux.send_key(key)?;
                self.shell_buffer.clear();
            }
            KeyCode::Backspace => {
                self.shell_buffer.pop();
                self.mux.send_key(key)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_terrek_key(&mut self, key: KeyEvent) -> Result<()> {
        // Exit Terrek mode
        if key.code == KeyCode::Esc {
            self.mode = Mode::Shell;
            return Ok(());
        }

        // Prefix handling (Ctrl+M)
        if is_ctrl_m(&key) {
            self.mux_prefix = true;
            return Ok(());
        }

        if self.mux_prefix {
            self.mux_prefix = false;
            match key.code {
                KeyCode::Char('v') => self.mux.execute(TerrekCommand::SplitVertical)?,
                KeyCode::Char('h') => self.mux.execute(TerrekCommand::SplitHorizontal)?,
                KeyCode::Char('o') => self.mux.execute(TerrekCommand::NextPane)?,
                KeyCode::Char('x') => self.mux.execute(TerrekCommand::ClosePane)?,
                _ => {}
            }
            return Ok(());
        }

        // Typing in Terrek buffer
        match key.code {
            KeyCode::Char(c) => {
                self.terrek_buffer.push(c);
                self.ai_tx.send(self.terrek_buffer.clone()).ok(); // live AI update
            }
            KeyCode::Backspace => {
                self.terrek_buffer.pop();
            }
            KeyCode::Enter => {
                println!();
                let input = self.terrek_buffer.trim();
                if input.starts_with("terrek ") {
                    let stripped = input.trim_start_matches("terrek ").trim();
                    let ctx = self.context.lock().unwrap();
                    let action = handle_command(&ctx, stripped)?;
                    if let TerrekAction::Output(text) = action {
                        println!("[Terrek] {}", text);
                    }
                }
                self.terrek_buffer.clear();
            }
            _ => {}
        }

        self.draw_prompt();
        Ok(())
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

        std::io::Write::flush(&mut std::io::stdout()).ok();
    }
}

fn is_ctrl_t(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('t') | KeyCode::Char('T'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_ctrl_m(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn start_ai_worker(
    rx: CbReceiver<String>,
    tx: CbSender<Vec<String>>,
    context: Arc<Mutex<ContextState>>,
) {
    thread::spawn(move || {
        for input in rx {
            if input.len() < 3 { continue; }
            thread::sleep(Duration::from_millis(400));

            let prompt = format!(
                "User typed: \"{}\". Suggest 5 Terrek commands.",
                input
            );

            let ctx = context.lock().unwrap();
            if let Ok(resp) = ai::gemini::ask_gemini(&ctx, &prompt) {
                let suggestions = resp.lines().map(|l| l.trim().to_string()).collect();
                tx.send(suggestions).ok();
            }
        }
    });
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let _cleanup = scopeguard::guard((), |_| disable_raw_mode().ok());

    let mut app = App::new()?;

    println!("Terrek started. Ctrl+T = enter Terrek, Ctrl+M = prefix, Esc = exit.");

    loop {
        app.mux.poll();
        app.handle_ai_updates(); // optional polling if needed

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if !app.handle_key(key)? {
                    break Ok(());
                }
            }
        }

        renderer::draw(&app.mux)?;
    }
}