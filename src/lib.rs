#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use anyhow::Error;
use crankstart::display::Display;
use crankstart::graphics::*;
use crankstart::{crankstart_game, system::*, Game, Playdate};
use crankstart_sys::PDButtons;

const ROWS: usize = LCD_ROWS as usize;
const COLUMNS: usize = 2 + (LCD_COLUMNS / 8) as usize;

// Allow us to clear the canvas
#[inline]
fn clear(frame: &mut [u8]) {
    for f in frame.iter_mut().take(COLUMNS * ROWS) {
        *f = u8::MIN;
    }
}

// Allow us to set a specific particle
#[inline]
fn set(frame: &mut [u8], x: usize, y: usize) {
    frame[y * COLUMNS + x] = u8::MAX;
}

// Allow us to swap two particles (or space)
#[inline]
fn swap(frame: &mut [u8], a: usize, b: usize) {
    frame.swap(a, b);
}

// Check if a particle exists in a space
#[inline]
fn is_empty(frame: &mut [u8], index: usize) -> bool {
    frame[index] == u8::MIN
}

#[inline]
fn update_pixel(frame: &mut [u8], i: usize) {
    // Get the indices of the pixels directly below
    let below = i + COLUMNS;
    let below_left = below - 1;
    let below_right = below + 1;

    // If there are no pixels below, including diagonals, move it accordingly.
    if is_empty(frame, below) {
        swap(frame, i, below);
    } else if is_empty(frame, below_left) {
        swap(frame, i, below_left);
    } else if is_empty(frame, below_right) {
        swap(frame, i, below_right);
    }
}

#[inline]
fn update(frame: &mut [u8]) {
    // Go through each pixel one by one and apply the rule
    for i in (0..(frame.len() - COLUMNS - 1)).rev() {
        update_pixel(frame, i);
    }
}

#[rustfmt::skip]
const INTRO: [[u8; COLUMNS]; 17] = [
    [1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

#[inline]
fn draw_intro(frame: &mut [u8]) {
    let start_x = 7;
    let start_y = 51;
    let font_height = 8;
    for i in 0..COLUMNS {
        for j in 0..INTRO.len() {
            for k in 0..font_height {
                frame[(COLUMNS * start_y)
                    + start_x
                    + i
                    + (COLUMNS * j * font_height)
                    + (COLUMNS * k)] = match INTRO[j][i] {
                    0 => u8::MIN,
                    _ => u8::MAX,
                };
            }
        }
    }
}

const STEPS: usize = 5;

#[inline]
fn process_input(game: &mut FallingSand) -> Result<(), Error> {
    let frame = Graphics::get().get_frame()?;

    let (pushed, _, _) = System::get().get_button_state()?;

    if pushed != PDButtons(0) {
        game.started = true;
    }

    if pushed & PDButtons::kButtonA == PDButtons::kButtonA {
        for i in 0..STEPS {
            set(frame, game.position, i);
        }
    }

    if pushed & PDButtons::kButtonLeft == PDButtons::kButtonLeft {
        if game.position == 0 {
            game.position = COLUMNS;
        }

        game.position -= 1;
    }

    if pushed & PDButtons::kButtonRight == PDButtons::kButtonRight {
        if game.position >= COLUMNS {
            game.position = 0;
        }

        game.position += 1;
    }

    if pushed & PDButtons(64) == PDButtons(64) {
        clear(frame);
    }

    if !game.started {
        return Ok(());
    }

    for _ in 0..STEPS {
        update(frame);
    }

    Ok(())
}

struct FallingSand {
    started: bool,
    position: usize,
}

impl FallingSand {
    pub fn new(_playdate: &Playdate) -> Result<Box<Self>, Error> {
        Display::get().set_refresh_rate(50.0)?;
        let frame = Graphics::get().get_frame()?;
        clear(frame);
        draw_intro(frame);
        Ok(Box::new(Self {
            started: false,
            position: 0,
        }))
    }
}

impl Game for FallingSand {
    fn update(&mut self, _playdate: &mut Playdate) -> Result<(), Error> {
        let graphics = Graphics::get();
        process_input(self)?;
        graphics.mark_updated_rows(0..=(LCD_ROWS as i32))?;
        Ok(())
    }
}

crankstart_game!(FallingSand);
