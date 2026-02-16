use std::path::Pathbuf;

pub struct ContextState{
    pub project_root : Option<Pathbuf>,
    pub git_branch : Option<String>,
    pub file_tree_snapshot : Option<Pathbuf>,
    pub 

}
