//! `terrek doctor`: a checklist of everything that has to work.

use super::Env;
use crate::ai::auto::{build, resolve};
use crate::config::config_path;
use crate::db::history::default_path;
use crate::db::HistoryStore;

pub fn run(env: &Env, ping: bool) -> String {
    let mut lines = Vec::new();
    let mut check = |ok: Option<bool>, label: &str, detail: String| {
        let mark = match ok {
            Some(true) => "✓",
            Some(false) => "✗",
            None => "!",
        };
        lines.push(format!("{mark} {label:<14} {detail}"));
    };

    let path = config_path();
    check(
        Some(true),
        "config",
        if path.exists() {
            path.display().to_string()
        } else {
            format!("{} (not created yet; defaults in use)", path.display())
        },
    );

    match resolve(&env.config.ai) {
        Ok(r) => {
            let source = r
                .key_source
                .clone()
                .unwrap_or_else(|| "no key needed".into());
            check(
                Some(true),
                "ai provider",
                format!("{}/{} (key: {source})", r.provider, r.model),
            );
            if ping {
                let result = build(r).and_then(|m| m.complete("Reply with exactly: ok", "ping"));
                match result {
                    Ok(_) => check(Some(true), "ai request", "provider answered".into()),
                    Err(e) => check(Some(false), "ai request", format!("{e:#}")),
                }
            }
        }
        Err(e) => check(Some(false), "ai provider", e.to_string()),
    }

    let shell = env
        .config
        .shell
        .program
        .clone()
        .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()));
    #[cfg(unix)]
    {
        use crate::pty::integration::ShellKind;
        let kind = ShellKind::detect(&shell);
        if !env.config.shell.integration {
            check(
                None,
                "shell hooks",
                format!("{shell}: disabled in config; commands won't be recorded"),
            );
        } else if kind.is_supported() {
            check(Some(true), "shell hooks", format!("{shell} ({kind:?})"));
        } else {
            check(
                None,
                "shell hooks",
                format!("{shell}: unsupported, use zsh, bash, or fish to record commands"),
            );
        }
        let has_base64 = std::process::Command::new("base64")
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok();
        check(
            Some(has_base64),
            "base64",
            if has_base64 {
                "found".into()
            } else {
                "missing: the shell hooks need it".into()
            },
        );
    }
    #[cfg(not(unix))]
    check(
        None,
        "shell",
        format!("{shell}: the interactive wrapper supports macOS and Linux"),
    );

    if env.config.history.enabled {
        match HistoryStore::open_default().and_then(|s| s.count()) {
            Ok(n) => check(
                Some(true),
                "history",
                format!("{n} commands in {}", default_path().display()),
            ),
            Err(e) => check(Some(false), "history", format!("{e:#}")),
        }
    } else {
        check(None, "history", "disabled in config".into());
    }

    let inside = std::env::var(crate::SESSION_ENV).is_ok();
    check(
        Some(true),
        "session",
        if inside {
            "running inside terrek".into()
        } else {
            "not inside terrek (run `terrek` to start)".into()
        },
    );

    lines.join("\n")
}
