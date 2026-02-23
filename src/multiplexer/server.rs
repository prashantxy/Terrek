pub struct Multiplexer{
    pub sessions : Vec<Session>
    pub active_session : usize
   
}

impl Multiplexer {
    pub fn new() -> Self;
    pub fn createsession(&mut self, name: String);
    pub fn kill_session(&mut self,id : Uuid);
    pub fn active_session_mut(&mut self) -> &mut Session;
}