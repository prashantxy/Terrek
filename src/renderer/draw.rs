use anyhow::Result;
use crossterm::{
    cursor,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
    ExecutableCommand,
};
use std::io::{stdout, Write};

use crate::multiplexer::server::Multiplexer;
use crate::multiplexer::pane::Pane;

pub fn draw(mux: &Multiplexer) -> Result<()> {
    let mut stdout = stdout();
    let (cols, rows) = terminal::size()?;

    // =========================
    // Clear screen
    // =========================
    stdout.execute(Clear(ClearType::All))?;
    stdout.execute(cursor::MoveTo(0, 0))?;

    // =========================
    // Draw Panes
    // =========================
    if let Some(session) = mux.active_session() {
        if let Some(window) = session.active_window() {
            let pane_count = window.panes.len().max(1);
            let pane_height = (rows - 1) / pane_count as u16; // reserve last row for status

            for (i, pane) in window.panes.iter().enumerate() {
                let y_offset = i as u16 * pane_height;

                draw_pane(
                    &mut stdout,
                    pane,
                    i == window.active_pane,
                    0,
                    y_offset,
                    cols,
                    pane_height,
                )?;
            }
        }
    }

    // =========================
    // Status Bar
    // =========================
    draw_status_bar(&mut stdout, mux, cols, rows)?;

    stdout.flush()?;
    Ok(())
}

fn draw_pane(
    stdout: &mut impl Write,
    pane: &Pane,
    active: bool,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> Result<()> {
    let lines = pane.buffer.lines();

    for row in 0..height {
        stdout.execute(cursor::MoveTo(x, y + row))?;

        if let Some(line) = lines.get(row as usize) {
            if active {
                stdout.execute(SetForegroundColor(Color::White))?;
            } else {
                stdout.execute(SetForegroundColor(Color::DarkGrey))?;
            }

            stdout.execute(Print(truncate(line, width as usize)))?;
        } else {
            stdout.execute(Print(" ".repeat(width as usize)))?;
        }

        stdout.execute(ResetColor)?;
    }

    Ok(())
}

fn draw_status_bar(
    stdout: &mut impl Write,
    mux: &Multiplexer,
    cols: u16,
    rows: u16,
) -> Result<()> {
    stdout.execute(cursor::MoveTo(0, rows - 1))?;
    stdout.execute(SetBackgroundColor(Color::DarkBlue))?;
    stdout.execute(SetForegroundColor(Color::White))?;

    let session_name = mux
        .active_session()
        .map(|s| s.name.as_str())
        .unwrap_or("No Session");

    let window_count = mux
        .active_session()
        .map(|s| s.windows.len())
        .unwrap_or(0);

    let status = format!(
        " Terrek | Session: {} | Windows: {} ",
        session_name,
        window_count
    );

    stdout.execute(Print(pad(status, cols as usize)))?;
    stdout.execute(ResetColor)?;

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn pad(mut s: String, width: usize) -> String {
    while s.len() < width {
        s.push(' ');
    }
    s
}