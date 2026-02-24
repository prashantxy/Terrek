use std::collections::HashMap;
use uuid::Uuid;

use crate::multiplexer::layout::{LayoutNode, SplitDirection};

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

pub fn compute_layout(
    node: &LayoutNode,
    area: Rect,
    map: &mut HashMap<Uuid, Rect>,
) {
    match node {
        LayoutNode::Leaf { pane_id } => {
            map.insert(*pane_id, area);
        }

        LayoutNode::Split {
            direction,
            first,
            second,
        } => match direction {
            SplitDirection::Vertical => {
                let half_width = area.width / 2;

                let left_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: half_width,
                    height: area.height,
                };

                let right_rect = Rect {
                    x: area.x + half_width,
                    y: area.y,
                    width: area.width - half_width,
                    height: area.height,
                };

                compute_layout(first, left_rect, map);
                compute_layout(second, right_rect, map);
            }

            SplitDirection::Horizontal => {
                let half_height = area.height / 2;

                let top_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: half_height,
                };

                let bottom_rect = Rect {
                    x: area.x,
                    y: area.y + half_height,
                    width: area.width,
                    height: area.height - half_height,
                };

                compute_layout(first, top_rect, map);
                compute_layout(second, bottom_rect, map);
            }
        },
    }
}