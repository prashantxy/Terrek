use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{unbounded, Sender as CbSender, Receiver as CbReceiver};
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

#[derive(Debug, PartialEq)]
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

    mux: Multiplexer,
    mux_prefix: bool,
}

impl App {
    fn new() -> Result<Self> {
        let context = Arc::new(Mutex::new(ContextState::new()));

      
        let (ai_tx, ai_rx): (CbSender<String>, CbReceiver<String>) = unbounded();

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

       
        let engine_clone = engine.clone();
        let context_clone = context.clone();
        thread::spawn(move || {
            for input in ai_rx {
                if input.len() < 3 {
                    continue;
                }

                thread::sleep(Duration::from_millis(400));

                let prompt = format!(
                    "User typed: \"{}\". Suggest 5 Terrek commands.",
                    input
                );

                let ctx = context_clone.lock().unwrap();

                if let Ok(resp) = ai::gemini::ask_gemini(&ctx, &prompt) {
                    let suggestions: Vec<String> =
                        resp.lines().map(|l| l.trim().to_string()).collect();
                    let mut eng = engine_clone.lock().unwrap();
                    eng.ai_cache = suggestions;
                }
            }
        });

        Ok(Self {
            mode: Mode::Shell,
            shell_buffer: String::new(),
            terrek_buffer: String::new(),
            engine,
            context,
            ai_tx,
            mux: Multiplexer::new("main".to_string())?,
            mux_prefix: false,
        })
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::Terrek;
                self.terrek_buffer.clear();
                println!("\n-- TERREK MODE --");
            }

            KeyCode::Char(c) => match self.mode {
                Mode::Shell => {
                    self.shell_buffer.push(c);
                    self.mux.send_key(key)?;
                }
                Mode::Terrek => {
                    self.terrek_buffer.push(c);
                    self.ai_tx.send(self.terrek_buffer.clone()).ok();
                    self.draw_prompt();
                }
            },

            KeyCode::Backspace => match self.mode {
                Mode::Shell => {
                    self.shell_buffer.pop();
                    self.mux.send_key(key)?;
                }
                Mode::Terrek => {
                    self.terrek_buffer.pop();
                    self.draw_prompt();
                }
            },

            KeyCode::Enter => match self.mode {
                Mode::Shell => {
                    self.mux.send_key(key)?;
                    self.shell_buffer.clear();
                }
                Mode::Terrek => {
                    println!();
                    let input = self.terrek_buffer.trim();

                    if input.starts_with("terrek ") {
                        let stripped = input.trim_start_matches("terrek ").trim();
                        let ctx = self.context.lock().unwrap();
                        let action = handle_command(&ctx, stripped)?;

                        let TerrekAction::Output(text) = action;
                        println!("[Terrek] {}", text);
                    }

                    self.terrek_buffer.clear();
                    self.mode = Mode::Shell;
                }
            },

            KeyCode::Esc => return Ok(false),

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

        std::io::Write::flush(&mut std::io::stdout()).ok();
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;

    let _cleanup = scopeguard::guard((), |_| { disable_raw_mode().ok(); });

    let mut app = App::new()?;

    println!("Terrek started. Ctrl+T = enter Terrek, Ctrl+M = prefix, Esc = exit.");

    loop {
        app.mux.poll()?;

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {

               
                if key.code == KeyCode::Char('m') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.mux_prefix = true;
                    continue;
                }

                if app.mux_prefix {
                    app.mux_prefix = false;

                    match key.code {
                        KeyCode::Char('v') => app.mux.execute(TerrekCommand::SplitVertical)?,
                        KeyCode::Char('h') => app.mux.execute(TerrekCommand::SplitHorizontal)?,
                        KeyCode::Char('o') => app.mux.execute(TerrekCommand::NextPane)?,
                        KeyCode::Char('x') => app.mux.execute(TerrekCommand::ClosePane)?,
                        _ => {}
                    }

                    continue;
                }

                if !app.handle_key(key)? {
                    break Ok(());
                }
            }
        }

        renderer::draw(&app.mux)?;
    }
}