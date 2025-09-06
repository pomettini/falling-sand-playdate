#![no_std]

extern crate alloc;

extern crate playdate as pd;

use crankit_game_loop::{game_loop, Game, Playdate};
use pd::controls::buttons::PDButtonsExt;
use pd::display::Display;
use pd::{
    controls::peripherals::Buttons,
    sys::ffi::{LCD_COLUMNS, LCD_ROWS},
};
use playdate::graphics::Graphics;

const ROWS: usize = LCD_ROWS as usize;
const COLUMNS: usize = 2 + (LCD_COLUMNS / 8) as usize;

// Allow us to clear the canvas
fn clear(frame: &mut [u8]) {
    for f in frame.iter_mut().take(COLUMNS * ROWS) {
        *f = u8::MIN;
    }
}

// Allow us to set a specific particle
fn set(frame: &mut [u8], x: usize, y: usize) {
    frame[y * COLUMNS + x] = u8::MAX;
}

// Allow us to swap two particles (or space)
fn swap(frame: &mut [u8], a: usize, b: usize) {
    frame.swap(a, b);
}

// Check if a particle exists in a space
fn is_empty(frame: &mut [u8], index: usize) -> bool {
    frame[index] == u8::MIN
}

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

const STEPS: usize = 3;

#[inline]
fn process_input(game: &mut FallingSand) {
    let frame = Graphics::Cached().get_frame().unwrap();
    let buttons = Buttons::Cached().get();

    if buttons.current.any() {
        game.started = true;
    }

    if buttons.current.a() {
        set(frame, game.position, 0);
        for i in 0..3 {
            for j in 0..3 {
                set(frame, game.position + (i - 1), 1 + j);
            }
        }
        set(frame, game.position, 4);
    }

    if buttons.current.left() {
        if game.position == 0 {
            game.position = COLUMNS;
        }

        game.position -= 1;
    }

    if buttons.current.right() {
        if game.position >= COLUMNS {
            game.position = 0;
        }

        game.position += 1;
    }

    if buttons.current.b() {
        clear(frame);
    }

    if !game.started {
        return;
    }

    for _ in 0..STEPS {
        update(frame);
    }
}

struct FallingSand {
    started: bool,
    position: usize,
}

impl Game for FallingSand {
    fn new(_playdate: &Playdate) -> Self {
        Display::Cached().set_refresh_rate(50.0);
        let frame = Graphics::Cached().get_frame().unwrap();
        clear(frame);
        draw_intro(frame);
        Self {
            started: false,
            position: 0,
        }
    }
    fn update(&mut self, _playdate: &Playdate) {
        let graphics = Graphics::Cached();
        process_input(self);
        graphics.mark_updated_rows(0, LCD_ROWS as i32);
    }
}

game_loop!(FallingSand);
