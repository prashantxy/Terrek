 use::std::path::Pathbuf;

 pub fn Build_Context_For_LLMs(context: &TerrekContext, user_input: &str)->String{
         format!{
            r#"You are Terrek AI — an AI assistant embedded inside a contextual terminal.

System Context:
- Project Root: {}
- Git Branch: {}
- Operating System: {}
- Shell: {}

Recent Terminal Context:
- Last Command: {}
- Last Exit Code: {:?}
- Last Error: {:?}
- Recent Commands:
{}

User Request:
{}

Instructions:
- Be precise and technical.
- Suggest terminal commands when appropriate.
- Do not invent project files.
- Keep response concise.
"#,
        context.project_root.as_deref().unwrap_or("None"),
        context.git_branch.as_deref().unwrap_or("None"),
        context.os,
        context.shell,
        context.last_command.as_deref().unwrap_or("None"),
        context.last_exit_code,
        context.last_error,
        context.recent_commands
            .iter()
            .enumerate()
            .map(|(i, cmd)| format!("  {}. {}", i + 1, cmd))
            .collect::<Vec<_>>()
            .join("\n"),
        user_input
         }
 }