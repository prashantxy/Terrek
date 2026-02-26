use crate::commands;

pub struct Workspace {
    pub id: String,
    pub time: i32,
    pub command: String,
}

impl Workspace {
    pub fn new(id: String, time: i32, command: String) -> Self {
        Self { id, time, command }
    }

    pub fn workspace_command(&mut self, command: String) {
        self.command = command;
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }
}