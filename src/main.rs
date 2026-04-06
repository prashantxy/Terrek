use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Instant, Duration};

use crossbeam_channel::{Receiver as CbReceiver, Sender as CbSender, unbounded};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

use uuid::Uuid;

mod ai;
mod commands;
mod config;
mod context;
mod db;

use commands::{TerrekAction, handle_command};
use context::state::ContextState;
use db::worker::{DbEvent, start_db_worker};

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

// ✅ Safe AI rate limiter
struct AiLimiter {
    last_call: Mutex<Option<Instant>>,
}

impl AiLimiter {
    fn new() -> Self {
        Self {
            last_call: Mutex::new(None),
        }
    }

    fn can_call(&self) -> bool {
        let mut last = self.last_call.lock().unwrap();
        let now = Instant::now();

        match *last {
            Some(prev) if now.duration_since(prev) < Duration::from_secs(10) => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}

// 🔥 Error detection (improved)
fn is_error(exit_code: i32, output: &str) -> bool {
    let out = output.to_lowercase();
    exit_code != 0
        || out.contains("error")
        || out.contains("not found")
        || out.contains("command not found")
        || out.contains("failed")
}

// 🔥 Extract exit code
fn extract_exit_code(buffer: &str) -> Option<(i32, String)> {
    if let Some(idx) = buffer.find("__TERREK_EXIT__") {
        let (before, after) = buffer.split_at(idx);

        if let Some(code_str) = after.split("__TERREK_EXIT__").nth(1) {
            if let Ok(code) = code_str.trim().parse::<i32>() {
                return Some((code, before.to_string()));
            }
        }
    }
    None
}

// 🔥 AI worker
fn start_ai_worker(
    rx: CbReceiver<String>,
    tx: CbSender<Vec<String>>,
    context: Arc<Mutex<ContextState>>,
) {
    thread::spawn(move || {
        let mut last_input = String::new();

        for input in rx {
            if input.len() < 3 || input == last_input {
                continue;
            }

            last_input = input.clone();
            thread::sleep(std::time::Duration::from_millis(400));

            let prompt = format!(
                "You are a CLI suggestion engine.
User typed: \"{}\"
Suggest 5 possible Terrek commands.
Return only command list.",
                input
            );

            let ctx = context.lock().unwrap();

            if let Ok(resp) = ai::gemini::ask_gemini(&ctx, &prompt) {
                let suggestions = resp
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>();

                tx.send(suggestions).ok();
            }
        }
    });
}

// UI
fn draw_prompt(buf: &str, suggestion: Option<&String>) {
    print!("\r\x1B[K[Terrek] > {}", buf);

    if let Some(s) = suggestion {
        if s.starts_with(buf) {
            let ghost = &s[buf.len()..];
            print!("\x1B[90m{}\x1B[0m", ghost);
        }
    }

    std::io::stdout().flush().ok();
}

fn clear_prompt() {
    print!("\r\x1B[K");
    std::io::stdout().flush().ok();
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let _cleanup = scopeguard::guard((), |_| {
        disable_raw_mode().ok();
    });

    let _db_tx: Sender<DbEvent> = start_db_worker();
    let _session_id = Uuid::new_v4().to_string();

    let context = Arc::new(Mutex::new(ContextState::new()));
    let ai_limiter = Arc::new(AiLimiter::new());

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

    // ✅ FIXED PTY SHELL
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or("/bin/bash".to_string());

    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-i"); // 🔥 interactive
    cmd.env("TERM", "xterm-256color");
    cmd.env(
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );

    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (out_tx, out_rx) = channel::<String>();

    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buffer[..n]).to_string();
                    out_tx.send(text).ok();
                }
                Err(_) => break,
            }
        }
    });

    let mut mode = Mode::Shell;
    let mut shell_input_buffer = String::new();
    let mut terrek_buffer = String::new();
    let mut current_output = String::new();

    println!("You are in SHELL session. Press Ctrl+T for TERREK mode.");

    loop {
        if let Ok(ai_suggestions) = ai_out_rx.try_recv() {
            let mut eng = engine.lock().unwrap();
            eng.ai_cache = ai_suggestions;
        }

        while let Ok(text) = out_rx.try_recv() {
            print!("{}", text);
            std::io::stdout().flush().ok();

            current_output.push_str(&text);

            if let Some((code, clean_output)) = extract_exit_code(&current_output) {
                current_output.clear();

                if is_error(code, &clean_output) {
                    println!("\n[Terrek] ⚠️ Command failed (code {})", code);

                    if ai_limiter.can_call() {
                        let ctx = context.lock().unwrap();

                        let trimmed = clean_output.chars().take(800).collect::<String>();

                        let prompt = format!(
                            "Command failed in terminal.

Output:
{}

Suggest a fix command only.",
                            trimmed
                        );

                        match ai::gemini::ask_gemini(&ctx, &prompt) {
                            Ok(resp) => println!("\n[Terrek AI Suggestion]:\n{}", resp),
                            Err(_) => println!("\n[Terrek] AI failed"),
                        }
                    }
                }
            }
        }

        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {
                KeyCode::Char('t') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = Mode::Terrek;
                    terrek_buffer.clear();
                    println!("\n-- TERREK MODE --");
                    draw_prompt("", None);
                }

                KeyCode::Char(c) => match mode {
                    Mode::Shell => {
                        shell_input_buffer.push(c);
                        print!("{}", c);
                        std::io::stdout().flush().ok();
                    }
                    Mode::Terrek => {
                        terrek_buffer.push(c);
                        ai_tx.send(terrek_buffer.clone()).ok();

                        let suggestions = {
                            let eng = engine.lock().unwrap();
                            eng.suggest(&terrek_buffer)
                        };

                        draw_prompt(&terrek_buffer, suggestions.first());
                    }
                },

                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        shell_input_buffer.pop();
                        print!("\x08 \x08");
                    }
                    Mode::Terrek => {
                        terrek_buffer.pop();
                        let suggestions = {
                            let eng = engine.lock().unwrap();
                            eng.suggest(&terrek_buffer)
                        };
                        draw_prompt(&terrek_buffer, suggestions.first());
                    }
                },

                KeyCode::Enter => match mode {
                    Mode::Shell => {
                        println!();
                        current_output.clear();

                        let command = shell_input_buffer.trim();

                        if !command.is_empty() {
                            let wrapped =
                                format!("{}; echo __TERREK_EXIT__$?\r", command);
                            writer.write_all(wrapped.as_bytes())?;
                            writer.flush()?;

                            let mut ctx = context.lock().unwrap();
                            ctx.add_command(command.to_string());
                        } else {
                            writer.write_all(b"\r")?;
                            writer.flush()?;
                        }

                        shell_input_buffer.clear();
                    }

                    Mode::Terrek => {
                        println!();
                        let input = terrek_buffer.trim();

                        if input.starts_with("terrek ") {
                            let stripped = input.trim_start_matches("terrek ").trim();
                            let ctx = context.lock().unwrap();
                            let action = handle_command(&ctx, stripped)?;

                            if let TerrekAction::Output(text) = action {
                                println!("[Terrek] {}", text);
                            }
                        }

                        terrek_buffer.clear();
                        draw_prompt("", None);
                    }
                },

                KeyCode::Esc => match mode {
                    Mode::Terrek => {
                        clear_prompt();
                        mode = Mode::Shell;
                        println!("\n-- SHELL MODE --");
                    }
                    Mode::Shell => break Ok(()),
                },

                _ => {}
            }
        }
    }
}