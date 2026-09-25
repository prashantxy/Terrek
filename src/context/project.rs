use std::path::{Path, PathBuf};

/// Files that identify a project root, with the ecosystem they indicate.
const MARKERS: &[(&str, &str)] = &[
    ("Cargo.toml", "Rust"),
    ("package.json", "Node.js"),
    ("pyproject.toml", "Python"),
    ("requirements.txt", "Python"),
    ("go.mod", "Go"),
    ("pom.xml", "Java (Maven)"),
    ("build.gradle", "JVM (Gradle)"),
    ("build.gradle.kts", "JVM (Gradle)"),
    ("Gemfile", "Ruby"),
    ("composer.json", "PHP"),
    ("mix.exs", "Elixir"),
    ("Package.swift", "Swift"),
    ("CMakeLists.txt", "C/C++ (CMake)"),
    ("Dockerfile", "Docker"),
    ("docker-compose.yml", "Docker Compose"),
    ("terraform.tf", "Terraform"),
    ("main.tf", "Terraform"),
];

/// Walk up from `start` to the nearest directory that looks like a project root.
pub fn detect_project_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|dir| {
        let is_root =
            dir.join(".git").exists() || MARKERS.iter().any(|(file, _)| dir.join(file).is_file());
        is_root.then(|| dir.to_path_buf())
    })
}

/// Ecosystems present at `root`, deduplicated, in marker order.
pub fn detect_kinds(root: &Path) -> Vec<&'static str> {
    let mut kinds: Vec<&'static str> = Vec::new();
    for (file, kind) in MARKERS {
        if root.join(file).is_file() && !kinds.contains(kind) {
            kinds.push(kind);
        }
    }
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_root_from_nested_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::write(dir.path().join("Dockerfile"), "").unwrap();
        let nested = dir.path().join("src/bin");
        fs::create_dir_all(&nested).unwrap();

        let root = detect_project_root(&nested).unwrap();
        assert_eq!(root, dir.path());
        assert_eq!(detect_kinds(&root), vec!["Rust", "Docker"]);
    }
}
