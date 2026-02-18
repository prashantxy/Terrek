use crate::context::ContextState;

pub fn build_gemini_prompt(
    context: &ContextState,
    user_input: &str,
) -> String {

    let project_root = context.project_root
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "None".to_string());

    let git_branch = context.git_branch
        .clone()
        .unwrap_or_else(|| "None".to_string());

    let last_command = context
        .last_commands
        .last()
        .cloned()
        .unwrap_or_else(|| "None".to_string());

    let last_exit_code = context
        .last_exit_code
        .map_or("None".to_string(), |c| c.to_string());

    let last_error = context
        .last_error
        .clone()
        .unwrap_or_else(|| "None".to_string());

    let recent_commands = if context.last_commands.is_empty() {
        "None".to_string()
    } else {
        context.last_commands
            .iter()
            .rev()
            .take(5)
            .enumerate()
            .map(|(i, cmd)| format!("{}. {}", i + 1, cmd))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let failure_hint = if context.last_exit_code.unwrap_or(0) != 0 {
        "NOTE: The last command failed. Prioritize diagnosing the failure.\n"
    } else {
        ""
    };

    format!(r#"
You are Terrek AI — an expert terminal assistant embedded inside a contextual shell.

System Context:
- OS: {}
- Shell: {}
- Project Root: {}
- Git Branch: {}

Recent State:
- Last Command: {}
- Last Exit Code: {}
- Last Error Output: {}

Recent Command History:
{}

{}

User Request:
{}

Be concise and technical.
"#,
        context.os,
        context.shell,
        project_root,
        git_branch,
        last_command,
        last_exit_code,
        last_error,
        recent_commands,
        failure_hint,
        user_input
    )
}
