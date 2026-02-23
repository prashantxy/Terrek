pub struct Multiplexer{
    pub panes : Vec<Pane>,
    pub active : usize,
}

impl Multiplexer{
    pub fn new(pane:Pane) -> Self{
        Self{
            panes : vec![pane],
            active : 0,
        }
    }
    pub fn active_mut_pane(&mut self) -> &mut pane{
        &mut self.panes[self.active];
    }
}