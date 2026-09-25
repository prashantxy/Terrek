use anyhow::{bail, Result};
use std::process::Command;

use crate::commands::open::open_url;

/// Posts with the `xmaster` CLI when installed; otherwise opens X's compose page.
pub fn post_to_x(message: &str) -> Result<String> {
    match Command::new("xmaster").arg("post").arg(message).output() {
        Ok(out) if out.status.success() => Ok("Posted to X".into()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let lower = stderr.to_lowercase();
            if lower.contains("402") || lower.contains("payment") || lower.contains("credits") {
                open_compose(message)?;
                Ok("The X API has no credits left; opened the compose page instead".into())
            } else {
                bail!("xmaster failed: {}", stderr.trim())
            }
        }
        Err(_) => {
            open_compose(message)?;
            Ok("Opened the X compose page (install `xmaster` to post directly)".into())
        }
    }
}

fn open_compose(message: &str) -> Result<()> {
    open_url(&format!(
        "https://x.com/intent/post?text={}",
        urlencoding::encode(message)
    ))
}

pub fn reddit(subreddit: &str, title: &str, body: &str) -> Result<String> {
    let subreddit = subreddit.trim_start_matches("r/");
    open_url(&format!(
        "https://www.reddit.com/r/{subreddit}/submit?title={}&text={}",
        urlencoding::encode(title),
        urlencoding::encode(body)
    ))?;
    Ok(format!("Opened a pre-filled post for r/{subreddit}"))
}
