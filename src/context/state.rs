use std::path::Pathbuf;

pub struct ContextState{
    pub project_root : Option<Pathbuf>,
    pub git_branch : Option<String>,
    pub file_tree_snapshot : Option<Pathbuf>,
    pub last_command : Option<String>,
    pub last_error : Option<String>,
    pub last_exit_command : Option<i32>,
    pub last
}


