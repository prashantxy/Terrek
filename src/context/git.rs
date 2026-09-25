use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct GitState {
    pub branch: String,
    pub changed_files: usize,
}

/// `None` outside a repository or when git is not installed.
pub fn git_state(dir: &Path) -> Option<GitState> {
    let branch = run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let status = run_git(dir, &["status", "--porcelain"]).unwrap_or_default();
    Some(GitState {
        branch: branch.trim().to_string(),
        changed_files: status.lines().filter(|l| !l.trim().is_empty()).count(),
    })
}

fn run_git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
