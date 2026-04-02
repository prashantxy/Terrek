use crate::ai::gemini::ask_gemini;
use crate::config::load_config;
use crate::context::ContextState;
use anyhow::Result;

pub fn maybe_trigger_ai(context: &ContextState) -> Result<()> {
    let config = load_config();

    if let Some(cfg) = config {
        if !cfg.auto_ai_on_error {
            return Ok(());
        }
    }

    if context.last_exit_code.unwrap_or(0) == 0 {
        return Ok(());
    }

    println!("\n Terrek AI analyzing failure...\n");

    let response = ask_gemini(context, "")?;

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" Terrek AI Suggestion:");
    println!("{}", response);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}
