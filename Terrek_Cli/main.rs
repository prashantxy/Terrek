use crossterm::event::{self, Event, KeyCode}; // Import terminal event handling
use crossterm::terminal::{disable_raw_mode, enable_raw_mode}; // For raw terminal mode
use std::io::{self, stdout}; // Input-output handling
#[allow(unused_imports)]
use tui::backend::{CrosstermBackend, Backend};

use tui::layout::{Layout, Constraint, Direction}; // For arranging UI components
use tui::widgets::{Block, Borders, Paragraph}; // Widgets for the UI
use tui::Terminal; // Terminal rendering

fn main() -> Result<(), io::Error> {
    // Enable raw mode for better terminal control
    enable_raw_mode()?;

    // Create a terminal backend with Crossterm
    let stdout = stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Application logic: keep running until 'q' is pressed
    loop {
        // Draw the UI
        terminal.draw(|f| {
            // Create a vertical layout with 2 sections
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(f.size());

            // Create a block with borders
            let block = Block::default()
                .title("Block 1")
                .borders(Borders::ALL);

            let paragraph = Paragraph::new("Press 'q' to quit").block(Block::default().borders(Borders::ALL));

            // Render the widgets in their respective chunks
            f.render_widget(block, chunks[0]);
            f.render_widget(paragraph, chunks[1]);
        })?;

        // Handle key events
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') {
                break; // Exit the loop on 'q'
            }
        }
    }

    // Restore terminal to normal mode
    disable_raw_mode()?;
    Ok(())
}
