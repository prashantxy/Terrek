/// Events decoded from the shell hooks' OSC markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    /// A command is about to run (zsh/fish).
    Started { command: String },
    /// A command finished. Bash includes the command text here instead of in `Started`.
    Finished {
        exit_code: i32,
        command: Option<String>,
        cwd: Option<String>,
    },
    /// The shell drew a prompt and is waiting for input.
    Prompt,
}

/// Output split around the markers, in stream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Output(Vec<u8>),
    Event(ShellEvent),
}
