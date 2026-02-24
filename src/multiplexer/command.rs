#[derive(Debug, Clone)]
pub enum TerrekCommand {
    
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    NextPane,
    PrevPane,
    FocusPane(usize),

    NewWindow,
    CloseWindow,
    NextWindow,
    PrevWindow,
    FocusWindow(usize),

    NewSession { name: String },
    KillSession,
    NextSession,
    PrevSession,
    FocusSession(usize),

    Resize { cols: u16, rows: u16 },
}