 use::std::path::Pathbuf;

 pub fn build_gemini_prompt(
    context: &TerrekContext,
    user_input: &str,
) -> String {

    let project_root = context.project_root
        .as_deref()
        .unwrap_or("None");

    let git_branch = context.git_branch
        .as_deref()
        .unwrap_or("None");

    let last_command = context.last_command
        .as_deref()
        .unwrap_or("None");

    let last_exit_code = context.last_exit_code
        .map(|c| c.to_string())
        .unwrap_or("None".into());

    let last_error = context.last_error
        .as_deref()
        .unwrap_or("None");

    let recent_commands = if context.recent_commands.is_empty() {
        "None".to_string()
    } else {
        context.recent_commands
            .iter()
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

Your role:
- Help debug commands
- Suggest correct terminal commands
- Diagnose build errors
- Improve developer productivity
- Stay precise and technical

System Context:
- Operating System: {}
- Shell: {}
- Project Root: {}
- Git Branch: {}

Recent Terminal State:
- Last Command: {}
- Last Exit Code: {}
- Last Error Output: {}

Recent Command History:
{}

{}

User Request:
{}

Response Rules:
- Be concise and technical.
- Suggest exact terminal commands when helpful.
- Do NOT invent project files.
- Do NOT assume frameworks unless stated.
- If information is missing, ask a short clarifying question.
- Avoid motivational or conversational fluff.
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
