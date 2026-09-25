//! `open <app | url | shortcut>`: `open slack`, `open yt lofi`, `open gh rust-lang/rust`.

use anyhow::{Context, Result};
use std::process::Command;

pub fn open(input: &str) -> Result<String> {
    let input = input.trim();
    if let Some(app) = find_app(input) {
        launch_app(&app)?;
        return Ok(format!("Opened {app}"));
    }
    let url = resolve_url(input);
    open_url(&url)?;
    Ok(format!("Opened {url}"))
}

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = Command::new("xdg-open");
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    cmd.arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("could not open {url}"))?;
    Ok(())
}

/// Shortcut keywords that always mean a website, never an app.
const SITE_KEYWORDS: &[&str] = &[
    "youtube",
    "yt",
    "github",
    "gh",
    "stackoverflow",
    "so",
    "reddit",
    "leetcode",
    "lc",
    "gfg",
    "google",
    "g",
];

pub fn resolve_url(input: &str) -> String {
    let lower = input.trim().to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let Some(first) = words.first() else {
        return "https://www.google.com".into();
    };
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return input.trim().to_string();
    }
    if words.len() == 1 && first.contains('.') {
        return format!("https://{first}");
    }

    let rest = &words[1..];
    let query = urlencoding::encode(&rest.join(" ")).into_owned();
    let everything = urlencoding::encode(&words.join(" ")).into_owned();
    match *first {
        "youtube" | "yt" if rest.is_empty() => "https://www.youtube.com".into(),
        "youtube" | "yt" => format!("https://www.youtube.com/results?search_query={query}"),
        "github" | "gh" if rest.is_empty() => "https://github.com".into(),
        "github" | "gh" if rest.len() == 1 => format!("https://github.com/{}", rest[0]),
        "github" | "gh" => format!("https://github.com/search?q={query}"),
        "stackoverflow" | "so" => format!("https://stackoverflow.com/search?q={query}"),
        "reddit" => format!("https://www.reddit.com/search/?q={query}"),
        "leetcode" | "lc" if rest.is_empty() => "https://leetcode.com".into(),
        "leetcode" | "lc" => format!("https://leetcode.com/problemset/?search={query}"),
        "gfg" => format!("https://www.geeksforgeeks.org/search/?gq={query}"),
        "google" | "g" => format!("https://www.google.com/search?q={query}"),
        _ if words.len() == 1 => format!("https://{first}.com"),
        _ => format!("https://www.google.com/search?q={everything}"),
    }
}

#[cfg(target_os = "macos")]
fn find_app(input: &str) -> Option<String> {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;

    let lower = input.to_lowercase();
    let first = lower.split_whitespace().next()?;
    if SITE_KEYWORDS.contains(&first) || lower.contains('.') || lower.contains("://") {
        return None;
    }

    let matcher = SkimMatcherV2::default();
    let mut best: Option<(i64, String)> = None;
    for dir in [
        "/Applications",
        "/System/Applications",
        "/Applications/Utilities",
    ] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str().and_then(|n| n.strip_suffix(".app")) else {
                continue;
            };
            let Some(score) = matcher.fuzzy_match(&name.to_lowercase(), &lower) else {
                continue;
            };
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, name.to_string()));
            }
        }
    }
    // Weak fuzzy matches are more likely a web search than an app.
    best.filter(|(score, _)| *score >= 40).map(|(_, name)| name)
}

#[cfg(not(target_os = "macos"))]
fn find_app(_input: &str) -> Option<String> {
    let _ = SITE_KEYWORDS;
    None
}

#[cfg(target_os = "macos")]
fn launch_app(name: &str) -> Result<()> {
    Command::new("open").args(["-a", name]).spawn()?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn launch_app(_name: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts() {
        assert_eq!(
            resolve_url("yt lofi beats"),
            "https://www.youtube.com/results?search_query=lofi%20beats"
        );
        assert_eq!(
            resolve_url("gh rust-lang/rust"),
            "https://github.com/rust-lang/rust"
        );
        assert_eq!(resolve_url("docs.rs"), "https://docs.rs");
        assert_eq!(
            resolve_url("https://Example.com/A"),
            "https://Example.com/A"
        );
        assert_eq!(resolve_url("figma"), "https://figma.com");
        assert_eq!(
            resolve_url("how to exit vim"),
            "https://www.google.com/search?q=how%20to%20exit%20vim"
        );
    }
}
