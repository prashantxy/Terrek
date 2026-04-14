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


static mut LAST_AI_CALL: Option<Instant> = None;

fn can_call_ai() -> bool {
    let now = Instant::now();
    unsafe {
        match LAST_AI_CALL {
            Some(last) if now.duration_since(last) < Duration::from_secs(10) => false,
            _ => {
                LAST_AI_CALL = Some(now);
                true
            }
        }
    }
}

fn is_error(exit_code: i32, output: &str) -> bool {
    exit_code != 0
        || output.to_lowercase().contains("error")
        || output.contains("not found")
        || output.contains("failed")
}

fn extract_exit_code(buffer: &str) -> Option<(i32, String)> {
    if let Some(idx) = buffer.find("__TERREK_EXIT__") {
        let (before, after) = buffer.split_at(idx);
        if let Some(code_str) = after.split("__TERREK_EXIT__").nth(1) {
            if let Ok(code) = code_str.trim().parse::<i32>() {
                return Some((code, before.trim().to_string()));
            }
        }
    }
    None
}

// AI Worker
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
            thread::sleep(Duration::from_millis(400));

            let prompt = format!(
                "You are a CLI suggestion engine.\nUser typed: \"{}\"\nSuggest 5 possible Terrek commands.\nReturn only the list, one per line.",
                input
            );

            let ctx = context.lock().unwrap();
            if let Ok(resp) = ai::gemini::ask_gemini(&ctx, &prompt) {
                let suggestions: Vec<String> = resp
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                let _ = tx.send(suggestions);
            }
        }
    });
}

fn draw_prompt(buf: &str, suggestion: Option<&String>) {
    print!("\r\x1B[K[Terrek] > {}", buf);
    if let Some(s) = suggestion {
        if s.starts_with(buf) {
            print!("\x1B[90m{}\x1B[0m", &s[buf.len()..]);
        }
    }
    let _ = std::io::stdout().flush();
}

fn clear_prompt() {
    print!("\r\x1B[K");
    let _ = std::io::stdout().flush();
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let _cleanup = scopeguard::guard((), |_| {
        let _ = disable_raw_mode();
    });

    let _db_tx: Sender<DbEvent> = start_db_worker();
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
            "terrek help".into(),
        ],
        history: vec![],
        ai_cache: vec![],
    }));

    start_ai_worker(ai_rx, ai_out_tx, context.clone());

    // ==================== PTY Setup ====================
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-i");
    cmd.env("TERM", "xterm-256color");

    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (out_tx, out_rx) = channel::<String>();

    // Shell output reader
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if n > 0 {
                        let text = String::from_utf8_lossy(&buffer[0..n]).to_string();
                        let _ = out_tx.send(text);
                    }
                }
                Err(_) => break,
            }
        }
    });

    // ==================== Main Loop ====================
    let mut mode = Mode::Shell;
    let mut terrek_buffer = String::new();
    let mut current_output = String::new();

    println!("Terrek Shell started. Press Ctrl+T to enter TERREK mode.\n");

    loop {
        // Handle AI suggestions
        if let Ok(suggestions) = ai_out_rx.try_recv() {
            let mut eng = engine.lock().unwrap();
            eng.ai_cache = suggestions;
        }

        // Handle shell output + exit code detection
        while let Ok(text) = out_rx.try_recv() {
            print!("{}", text);
            let _ = std::io::stdout().flush();
            current_output.push_str(&text);

            if let Some((code, clean_output)) = extract_exit_code(&current_output) {
                current_output.clear();
                if is_error(code, &clean_output) && can_call_ai() {
                    let ctx = context.lock().unwrap();
                    let prompt = format!(
                        "Command failed:\n{}\nSuggest only a fix command.",
                        clean_output.chars().take(800).collect::<String>()
                    );
                    if let Ok(resp) = ai::gemini::ask_gemini(&ctx, &prompt) {
                        println!("\n[Terrek AI Suggestion]:\n{}", resp);
                    }
                }
            }
        }

        // Keyboard input
        if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
            match code {
                // Ctrl + T - Toggle Terrek / Shell mode
                KeyCode::Char('t') if modifiers.contains(KeyModifiers::CONTROL) => {
                    mode = match mode {
                        Mode::Shell => Mode::Terrek,
                        Mode::Terrek => Mode::Shell,
                    };

                    if let Mode::Terrek = mode {
                        terrek_buffer.clear();
                        println!("\n-- TERREK MODE --");
                        println!("You can type normal commands (ls, echo, cd...) or Terrek commands starting with 'terrek'");
                        draw_prompt("", None);
                    } else {
                        println!("\n-- SHELL MODE --");
                    }
                    continue;
                }

                // Character typed
                KeyCode::Char(c) => match mode {
                    Mode::Shell => {
                        let _ = writer.write_all(&[c as u8]);
                        let _ = writer.flush();
                    }
                    Mode::Terrek => {
                        terrek_buffer.push(c);
                        let _ = ai_tx.send(terrek_buffer.clone());
                        let suggestions = {
                            let eng = engine.lock().unwrap();
                            eng.suggest(&terrek_buffer)
                        };
                        draw_prompt(&terrek_buffer, suggestions.first());
                    }
                },

                // Backspace
                KeyCode::Backspace => match mode {
                    Mode::Shell => {
                        let _ = writer.write_all(b"\x7f");
                        let _ = writer.flush();
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

                // Enter pressed
                KeyCode::Enter => match mode {
                    Mode::Shell => {
                        println!();
                        let _ = writer.write_all(b"\r\n");
                        let _ = writer.flush();
                    }
                    Mode::Terrek => {
                        println!();
                        let input = terrek_buffer.trim();

                        if !input.is_empty() {
                            if input.starts_with("terrek ") || input == "terrek" {
                                // Terrek internal command
                                let stripped = if input == "terrek" {
                                    ""
                                } else {
                                    input.trim_start_matches("terrek ").trim()
                                };

                                let ctx = context.lock().unwrap();
                                match handle_command(&ctx, stripped) {
                                    Ok(TerrekAction::Output(text)) => println!("[Terrek] {}", text),
                                    Err(e) => println!("[Terrek] Error: {}", e),
                                }
                            } else {
                                // Normal shell command
                                let wrapped = format!("{}; echo __TERREK_EXIT__$?\r\n", input);
                                let _ = writer.write_all(wrapped.as_bytes());
                                let _ = writer.flush();

                                let mut ctx = context.lock().unwrap();
                                ctx.add_command(input.to_string());
                            }
                        }

                        terrek_buffer.clear();
                        draw_prompt("", None);
                    }
                },

                // Esc key
                KeyCode::Esc => match mode {
                    Mode::Terrek => {
                        clear_prompt();
                        mode = Mode::Shell;
                        println!("\n-- SHELL MODE --");
                    }
                    Mode::Shell => {
                        println!("\nExiting Terrek...");
                        break Ok(());
                    }
                },

                _ => {}
            }
        }
    }
}