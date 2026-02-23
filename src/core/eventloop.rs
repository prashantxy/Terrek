use std::time::{Duration, Instant};
use anyhow::Result;
use crossterm::event::{self, Event};

use crate::multiplexer::server::Multiplexer;
use crate::renderer;
use crate::core::input::InputHandler;

const FRAME_TIME: Duration = Duration::from_millis(16); // ~60 FPS

pub fn run(mut mux: Multiplexer) -> Result<()> {
    let mut input_handler = InputHandler::new();
    let mut last_frame = Instant::now();

    loop {
        // Non-blocking input polling
        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key) => {
                    input_handler.handle_key(key, &mut mux)?;
                }
                Event::Resize(cols, rows) => {
                    mux.handle_resize(cols, rows)?;
                }
                _ => {}
            }
        }

        //  Render at fixed frame rate
        if last_frame.elapsed() >= FRAME_TIME {
            renderer::draw(&mux)?;
            last_frame = Instant::now();
        }
    }
}