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
       if self.last_commands.len >= 5 {
        self.last_commands.remove(0);
       }
       self.last_commands.push(commands);
    }
    pub fn update_exit_code(&mut self,code : i32,stderr: Option<String>){
        Self.update_exit_code=Some(code);

        if code != 0 {
            self.last_error = stderr;
        }
        else{
            self.last_error = None;
        }
    }
    pub fn set_project_root(&mut self, path:Pathbuf){
        self.project_root = Some(path);
    }
    
}



