use std::path::{Path, PathBuf};

use super::git::{git_state, GitState};
use super::project::{detect_kinds, detect_project_root};
use crate::db::CommandRecord;

#[derive(Debug, Clone, Default)]
pub struct ContextState {
    pub os: String,
    pub shell: String,
    pub cwd: PathBuf,
    pub project_root: Option<PathBuf>,
    pub project_kinds: Vec<&'static str>,
    pub git: Option<GitState>,
    /// Oldest first.
    pub recent_commands: Vec<CommandRecord>,
}

impl ContextState {
    pub fn capture(cwd: &Path, shell: &str, mut recent_newest_first: Vec<CommandRecord>) -> Self {
        let project_root = detect_project_root(cwd);
        let project_kinds = project_root
            .as_deref()
            .map(detect_kinds)
            .unwrap_or_default();
        recent_newest_first.reverse();
        Self {
            os: std::env::consts::OS.to_string(),
            shell: shell.to_string(),
            cwd: cwd.to_path_buf(),
            git: git_state(cwd),
            project_root,
            project_kinds,
            recent_commands: recent_newest_first,
        }
    }
}
