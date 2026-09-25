use crate::context::context_builder::build_context;
use crate::context::ContextState;
use crate::db::CommandRecord;

pub const SYSTEM_PROMPT: &str = "\
You are Terrek, an assistant built into the user's terminal. The message starts with a \
snapshot of their environment (OS, shell, directory, project, git, recent commands); tailor \
every answer to it, and don't invent files, flags, or output it doesn't show.

Keep answers short and practical: lead with the command(s) to run in a fenced code block, \
then at most a few lines of explanation. Say so explicitly when a command is destructive or \
irreversible. If the answer depends on something you can't see, name the one thing to check.";

/// Largest piped-stdin excerpt sent to the model.
const MAX_STDIN_CHARS: usize = 24_000;

pub fn ask_prompt(ctx: &ContextState, question: &str, stdin: Option<&str>) -> String {
    let mut prompt = format!("<environment>\n{}</environment>\n\n", build_context(ctx));
    if let Some(input) = stdin.filter(|s| !s.trim().is_empty()) {
        let (excerpt, truncated) = tail_chars(input, MAX_STDIN_CHARS);
        let note = if truncated {
            " (truncated to the last part)"
        } else {
            ""
        };
        prompt.push_str(&format!(
            "<piped_input{note}>\n{excerpt}\n</piped_input>\n\n"
        ));
    }
    prompt.push_str(&format!("Question: {}", question.trim()));
    prompt
}

pub fn fix_prompt(ctx: &ContextState, failed: &CommandRecord) -> String {
    let mut prompt = format!("<environment>\n{}</environment>\n\n", build_context(ctx));
    prompt.push_str(&format!(
        "This command failed with exit code {}:\n$ {}\n",
        failed
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".into()),
        failed.command
    ));
    if let Some(cwd) = &failed.cwd {
        prompt.push_str(&format!("(run in {cwd})\n"));
    }
    match failed.output.as_deref().filter(|o| !o.trim().is_empty()) {
        Some(output) => prompt.push_str(&format!("\n<output>\n{output}\n</output>\n")),
        None => prompt.push_str("\n(Its output was not captured.)\n"),
    }
    prompt.push_str(
        "\nExplain the cause in one or two sentences, then give the corrected command or the \
         steps to fix it.",
    );
    prompt
}

/// Keep the last `max` chars; the end of a log is usually what matters.
fn tail_chars(text: &str, max: usize) -> (&str, bool) {
    let count = text.chars().count();
    if count <= max {
        return (text, false);
    }
    let start = text
        .char_indices()
        .nth(count - max)
        .map(|(i, _)| i)
        .unwrap_or(0);
    (&text[start..], true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_prompt_includes_command_output_and_code() {
        let failed = CommandRecord {
            command: "cargo build".into(),
            exit_code: Some(101),
            output: Some("error[E0425]: cannot find value `x`".into()),
            cwd: Some("/work".into()),
            ..Default::default()
        };
        let prompt = fix_prompt(&ContextState::default(), &failed);
        assert!(prompt.contains("exit code 101"));
        assert!(prompt.contains("$ cargo build"));
        assert!(prompt.contains("E0425"));
        assert!(prompt.contains("(run in /work)"));
    }

    #[test]
    fn ask_prompt_truncates_long_stdin_from_the_front() {
        let big = format!("{}END", "x".repeat(MAX_STDIN_CHARS + 50));
        let prompt = ask_prompt(&ContextState::default(), "why?", Some(&big));
        assert!(prompt.contains("truncated"));
        assert!(prompt.contains("END"));
        assert!(prompt.ends_with("Question: why?"));
    }

    #[test]
    fn tail_chars_respects_utf8() {
        assert_eq!(tail_chars("héllo", 3), ("llo", true));
        assert_eq!(tail_chars("hé", 5), ("hé", false));
    }
}
