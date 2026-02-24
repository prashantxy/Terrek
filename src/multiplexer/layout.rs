use uuid::Uuid;

#[derive(Clone)]
pub enum LayoutNode {
    Leaf {
        pane_id: Uuid,
    },

    Split {
        direction: SplitDirection,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

#[derive(Clone, Copy)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

impl LayoutNode {
    pub fn split_leaf(
        &mut self,
        target: Uuid,
        new_pane: Uuid,
        direction: SplitDirection,
    ) {
        match self {
            LayoutNode::Leaf { pane_id } if *pane_id == target => {
                let old = *pane_id;

                *self = LayoutNode::Split {
                    direction,
                    first: Box::new(LayoutNode::Leaf { pane_id: old }),
                    second: Box::new(LayoutNode::Leaf { pane_id: new_pane }),
                };
            }

            LayoutNode::Split { first, second, .. } => {
                first.split_leaf(target, new_pane, direction);
                second.split_leaf(target, new_pane, direction);
            }

            _ => {}
        }
    }
}