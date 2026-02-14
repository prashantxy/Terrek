use std::path::{Path,Pathbuf};
pub fn detect_project_root(start : &Path) -> Option<Pathbuf>{
    let mut current = start.to_path_buf();

    loop{
        if current.join(".git").exists()
        || current.join("package.json").exists()
        || current.join("cargo.toml").exists()
        || current.join("pyproject.toml").exists()
        {
            return Some(current)
        }

        if !current.pop()
        {
            break;
        }
    }
    None
}