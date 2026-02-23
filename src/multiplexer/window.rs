pub struct windows{
    pub id : Uuid;
    pub layout : LayoutNode;
    pub panes : HashMap<Uuid,Pane>;
    pub active_panes : Uuid;
}


impl windows{
    pub fn new() -> Self;
    pub fn split_active(&mut self,direction: SplitDirection);
    pub fn close_pane(&mut self, pane_id : Uuid);
    pub fn focus_next(&mut self);
}