pub enum LayoutNode{
   Leaf : {pane_id : Uuid};
   split{
     direction: SplitDirection;
     ratio: f32;
     first:  
     second:
   },
}