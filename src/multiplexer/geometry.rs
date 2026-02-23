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
);