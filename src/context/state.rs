use std::path::Pathbuf;

pub struct ContextState{
    pub project_root : Option<Pathbuf>,
    pub git_branch : Option<String>,
    pub file_tree_snapshot : Option<Pathbuf>,
    pub last_command : Option<String>,
    pub last_error : Option<String>,
    pub last_exit_command : Option<i32>,
    pub active_file : Option<Pathbuf>,
}

impl ContextState{
    pub fn new () -> Self {
         Self{
            project_root:None,
            git_branch:None,
            file_tree_snapshot: Vec::new(),
            last-command: Vec::new(),
            last_error:None,
            last_exit_command:None,
            active_file:None,
         }
    }

    pub fn add_command(&mut self,command:String)->{
       
    }
}



