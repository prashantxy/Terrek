use portable_pty::{native_pty_system, PtySize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::io::write;
use multiplexer::server::Multiplexer;
use multiplexer::commands::TerrekCommand;

use crossbeam_channel::{unbounded, Receiver as CbReceiver, Sender as CbSender};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

mod ai;
mod db;
mod commands;
mod context;
mod config;
mod renderer;
mod multiplexer;
use commands::{handle_command, TerrekAction};
use context::state::ContextState;

#[derive(Debug)]
enum Mode {
    Shell,
    Terrek,
}

struct App {
    mode: Mode,
    shell_input_buffer: String,
    terrek_buffer: String,

    engine: Arc<Mutex<SuggestionEngine>>,
    context: Arc<Mutex<ContextState>>,

    ai_tx: CbSender<String>,
    ai_out_rx: CbReceiver<Vec<String>>,

    mux: Multiplexer,
    mux_prefix: bool,
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

impl App {
    fn new() -> anyhow::Result<Self> {
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

        // Initialize Multiplexer (this internally creates first PTY)
        let pty_system = native_pty_system();
        let mux = Multiplexer::new(pty_system, PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        Ok(Self {
            mode: Mode::Shell,
            shell_input_buffer: String::new(),
            terrek_buffer: String::new(),
            engine,
            context,
            ai_tx,
            ai_out_rx,
            mux,
            mux_prefix: false,
        })
    }

    fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char(c) => match self.mode {
                Mode::Shell => {
                    self.shell_input_buffer.push(c);
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
                    self.shell_input_buffer.pop();
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
                    self.shell_input_buffer.clear();
                }
                Mode::Terrek => {
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
                    self.draw_prompt();
                }
            },

            KeyCode::Esc => {
                self.mode = Mode::Shell;
            }

            _ => {}
        }

        Ok(true)
    }

    fn draw_prompt(&self) {
        print!("\r[Terrek] > {}", self.terrek_buffer);
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
                let suggestions =
                    resp.lines().map(|l| l.trim().to_string()).collect();
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

    let mut app = App::new()?;

    println!("You are in TERREK SHELL.");
    println!("Ctrl+T → prefix mode");
    println!("Ctrl+T + v → vertical split");
    println!("Ctrl+T + h → horizontal split");
    println!("Ctrl+T + o → next pane");
    println!("Ctrl+T + x → close pane");

    loop {
        app.mux.poll();
        app.handle_ai_updates();

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {

                // Prefix logic
                if key.code == KeyCode::Char('t')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    app.mux_prefix = true;
                    continue;
                }

                if app.mux_prefix {
                    app.mux_prefix = false;

                    match key.code {
                        KeyCode::Char('v') =>
                            app.mux.execute(TerrekCommand::SplitVertical)?,

                        KeyCode::Char('h') =>
                            app.mux.execute(TerrekCommand::SplitHorizontal)?,

                        KeyCode::Char('o') =>
                            app.mux.execute(TerrekCommand::NextPane)?,

                        KeyCode::Char('x') =>
                            app.mux.execute(TerrekCommand::ClosePane)?,

                        _ => {}
                    }

                    continue;
                }

                if !app.handle_key(key)? {
                    break;
                }
            }
        }

        renderer::draw(&app.mux)?;
    }

    Ok(())
}