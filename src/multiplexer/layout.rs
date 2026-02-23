pub enum LayoutNode{
   Leaf : {pane_id : Uuid};
   split{
     direction: SplitDirection;
     ratio: f32;
     first:  Box<LayoutNode>;
     second:  Box<LayoutNode>;
   },
}

pub enum SplitDirection{
    Horizontal;
    Vertical;
}

impl LayoutNode {
    pub fn split_leaf(
        &mut self,
        target: Uuid,
        new_pane: Uuid,
        direction: SplitDirection,
    );
}