use super::state::ContextState;

pub fn build_context(context: &ContextState) -> String {
    format!(
        "
=== TERREK CONTEXT ===

Project Root: {}
Git Branch: {}
Recent Commands: {}
Last Exit Code: {}
Last Error: {}

=======================
",
        context
            .project_root
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "None".into()),
        context.git_branch.clone().unwrap_or_else(|| "None".into()),
        if context.last_commands.is_empty() {
            "None".into()
        } else {
            context.last_commands.join(" | ")
        },
        context
            .last_exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "None".into()),
        context.last_error.clone().unwrap_or_else(|| "None".into()),
    )
}
