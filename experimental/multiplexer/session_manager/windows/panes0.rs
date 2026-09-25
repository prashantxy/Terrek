pub struct Pane {
    pub id : Uuid,
    pub writer : Box<dyn write+send>,
    pub buffer : Arc<Mutex<Vec<String>>>,
    pub x : u16,
    pub y : u16,
    pub width : u16,
    pub height : u16
}