use super::state::ContextState;

pub fn build_context(context: &ContextState) -> String {
    format!(
        "
=== TERREK CONTEXT ===

Project Root: {:?}
Git Branch: {:?}
Recent Commands: {:?}
Last Exit Code: {:?}
Last Error: {:?}

=======================
",
        context.project_root,
        context.git_branch,
        context.last_commands,
        context.last_exit_code,
        context.last_error,
    )
}
