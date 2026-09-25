pub struct Pane{
    pub id : Uuid,
    pub key : Uuid,
    pub x : u16,
    pub y : u16,
    pub width : u16,
    pub height : u16,
    pub writer : Box<dyn write+send>
}