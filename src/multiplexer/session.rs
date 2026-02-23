pub struct Session{
    pub id : Uuid;
    pub name : String;
    pub windows : Vec<String>;
    pub active_windows : usize;
}