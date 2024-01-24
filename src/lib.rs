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
    frame[y * ROWS + x] = u8::MAX;
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

struct FallingSand {}

impl FallingSand {
    pub fn new(_playdate: &Playdate) -> Result<Box<Self>, Error> {
        Display::get().set_refresh_rate(50.0)?;
        clear(Graphics::get().get_frame()?);
        Ok(Box::new(Self {}))
    }
}

const STEPS: usize = 5;

impl Game for FallingSand {
    fn update(&mut self, _playdate: &mut Playdate) -> Result<(), Error> {
        let graphics = Graphics::get();

        let crank_step = ((System::get().get_crank_angle()? / 360.0) * COLUMNS as f32) as usize;

        let frame = graphics.get_frame()?;

        let (pushed, _, _) = System::get().get_button_state()?;

        if (pushed & PDButtons::kButtonA) == PDButtons::kButtonA {
            set(frame, crank_step, 0);
        }

        for _ in 0..STEPS {
            update(frame);
        }

        graphics.mark_updated_rows(0..=(LCD_ROWS as i32) - 1)?;

        Ok(())
    }
}

crankstart_game!(FallingSand);
