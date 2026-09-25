use std::fmt::Write;

use super::ContextState;

/// Render the context as a compact block for an AI prompt.
pub fn build_context(ctx: &ContextState) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "OS: {}", ctx.os);
    let _ = writeln!(out, "Shell: {}", ctx.shell);
    let _ = writeln!(out, "Working directory: {}", ctx.cwd.display());
    if let Some(root) = &ctx.project_root {
        let kinds = if ctx.project_kinds.is_empty() {
            String::new()
        } else {
            format!(" ({})", ctx.project_kinds.join(", "))
        };
        let _ = writeln!(out, "Project root: {}{kinds}", root.display());
    }
    if let Some(git) = &ctx.git {
        let _ = writeln!(
            out,
            "Git: branch {}, {} changed file(s)",
            git.branch, git.changed_files
        );
    }
    if !ctx.recent_commands.is_empty() {
        let _ = writeln!(out, "Recent commands (oldest first):");
        for record in &ctx.recent_commands {
            let status = record
                .exit_code
                .map(|c| format!("exit {c}"))
                .unwrap_or_else(|| "exit ?".into());
            let _ = writeln!(out, "  $ {}   [{status}]", record.command);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::CommandRecord;

    #[test]
    fn renders_recent_commands_with_status() {
        let ctx = ContextState {
            os: "macos".into(),
            shell: "zsh".into(),
            cwd: "/tmp/p".into(),
            recent_commands: vec![CommandRecord {
                command: "cargo test".into(),
                exit_code: Some(101),
                ..Default::default()
            }],
            ..Default::default()
        };
        let text = build_context(&ctx);
        assert!(text.contains("Shell: zsh"));
        assert!(text.contains("$ cargo test   [exit 101]"));
        assert!(!text.contains("Git:"));
    }
}
