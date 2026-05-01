use std::process::Command;

pub fn get_git_branch() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
 
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout);
        Some(branch.trim().to_string())
    } else {
        None
    }
}
 