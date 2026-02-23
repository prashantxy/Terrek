pub struct Session{
    pub id : Uuid;
    pub name : String;
    pub windows : Vec<String>;
    pub active_windows : usize;
}

impl Session{
    pub fn new(name: String) -> Self;
    pub fn create_windows(&mut self)
}