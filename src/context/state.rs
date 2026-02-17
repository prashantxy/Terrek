use std::path::PathBuf;

pub struct ContextState {
    pub project_root: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub os : String,
    pub shell : String,
    pub file_tree_snapshot: Option<String>,
    pub last_commands: Vec<String>,
    pub last_error: Option<String>,
    pub last_exit_code: Option<i32>,
    pub active_file: Option<PathBuf>,
}

impl ContextState {
    pub fn new() -> Self {
        Self {
            project_root: None,
            git_branch: None,
            file_tree_snapshot: None,
            last_commands: Vec::new(),
            last_error: None,
            last_exit_code: None,
            active_file: None,
        }
    }

    pub fn add_command(&mut self, command: String) {
        if self.last_commands.len() >= 5 {
            self.last_commands.remove(0);
        }
        self.last_commands.push(command);
    }

    pub fn update_exit_code(&mut self, code: i32, stderr: Option<String>) {
        self.last_exit_code = Some(code);

        if code != 0 {
            self.last_error = stderr;
        } else {
            self.last_error = None;
        }
    }

    pub fn set_project_root(&mut self, path: PathBuf) {
        self.project_root = Some(path);
    }

    pub fn set_git_branch(&mut self, branch: Option<String>) {
        self.git_branch = branch;
    }

    pub fn debug_print(&self) {
        println!("----------------------------");
        println!("TERREK CONTEXT DEBUG");
        println!("Project Root: {:?}", self.project_root);
        println!("Git Branch: {:?}", self.git_branch);
        println!("Last Commands: {:?}", self.last_commands);
        println!("Last Exit Code: {:?}", self.last_exit_code);
        println!("Last Error: {:?}", self.last_error);
        println!("----------------------------");
    }
}
