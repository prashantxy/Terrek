pub struct windows{
    pub id : Uuid;
    pub layout : LayoutNode;
    pub panes : HashMap<Uuid,pane>
    pub active_panes : Uuid;
}