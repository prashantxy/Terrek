pub struct Pane {
    pub id : Uuid,
    pub writer : Box<dyn write+send>,
    pub buffer : Arc<Mutex<Vec<String>>>,

}