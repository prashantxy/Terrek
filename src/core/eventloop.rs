use std::time::{Duration, Instant};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};

use crate::multiplexer::server::Multiplexer;
use crate::renderer;
use crate::core::input::InputHandler;

const FRAME_TIME: Duration = Duration::from_millis(16); // ~60 FPS
const INPUT_POLL_TIME: Duration = Duration::from_millis(1);

pub fn run(mut mux: Multiplexer) -> Result<()> {
    let mut input_handler = InputHandler::new();
    let mut last_frame = Instant::now();

    loop {

        while event::poll(INPUT_POLL_TIME)? {
            match event::read()? {
                Event::Key(key) => {

                    // Optional global exit
                    if key.code == KeyCode::Char('q') && key.modifiers.is_empty() {
                        return Ok(());
                    }

                    input_handler.handle_key(key, &mut mux)?;
                }

                Event::Resize(cols, rows) => {
                    mux.handle_resize(cols, rows)?;
                }

                _ => {}
            }
        }

        
        mux.poll()?;   // <- VERY IMPORTANT

        if last_frame.elapsed() >= FRAME_TIME {
            renderer::draw(&mux)?;
            last_frame = Instant::now();
        }
    }
}