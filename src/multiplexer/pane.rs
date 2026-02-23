pub struct Pane {
    pub id: Uuid,
    pub writer: Box<dyn Write + Send>,
    pub buffer: ScrollbackBuffer,
}

impl Pane{
    pub fn new() -> Result<Self>;
    pub fn write(&mut self, bytes : &u[8]) -> Result<()>;
    pub fn resize(&mut self , row:u16, cols:u16);
}