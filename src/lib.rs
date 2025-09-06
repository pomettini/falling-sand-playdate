#![no_std]

extern crate alloc;
extern crate playdate as pd;

use crankit_game_loop::{game_loop, Game, Playdate};
use pd::controls::buttons::PDButtonsExt;
use pd::display::Display;
use pd::graphics::BitmapDrawMode;
use pd::system::System;
use pd::{
    controls::peripherals::Buttons,
    sys::ffi::{LCD_COLUMNS, LCD_ROWS},
};
use playdate::graphics::Graphics;

const ROWS: usize = LCD_ROWS as usize;
const COLUMNS: usize = 2 + (LCD_COLUMNS / 8) as usize;
const PIXEL_WIDTH: usize = LCD_COLUMNS as usize;

// Clear the canvas
fn clear(frame: &mut [u8]) {
    for f in frame.iter_mut().take(COLUMNS * ROWS) {
        *f = 0;
    }
}

// Set a specific pixel by manipulating bits within bytes
fn set_pixel(frame: &mut [u8], x: usize, y: usize, value: bool) {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return;
    }

    let byte_x = x / 8;
    let bit_x = x % 8;
    let index = y * COLUMNS + byte_x;

    if index < frame.len() {
        if value {
            frame[index] |= 1 << (7 - bit_x);
        } else {
            frame[index] &= !(1 << (7 - bit_x));
        }
    }
}

// Get a specific pixel value
fn get_pixel(frame: &[u8], x: usize, y: usize) -> bool {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return false;
    }

    let byte_x = x / 8;
    let bit_x = x % 8;
    let index = y * COLUMNS + byte_x;

    if index < frame.len() {
        (frame[index] & (1 << (7 - bit_x))) != 0
    } else {
        false
    }
}

fn swap_pixels(frame: &mut [u8], x1: usize, y1: usize, x2: usize, y2: usize) {
    let pixel1 = get_pixel(frame, x1, y1);
    let pixel2 = get_pixel(frame, x2, y2);
    set_pixel(frame, x1, y1, pixel2);
    set_pixel(frame, x2, y2, pixel1);
}

fn update_pixel(frame: &mut [u8], x: usize, y: usize) {
    if !get_pixel(frame, x, y) {
        return;
    }

    if y >= ROWS - 1 {
        return;
    }

    if !get_pixel(frame, x, y + 1) {
        swap_pixels(frame, x, y, x, y + 1);
    } else if x > 0 && !get_pixel(frame, x - 1, y + 1) {
        swap_pixels(frame, x, y, x - 1, y + 1);
    } else if x < PIXEL_WIDTH - 1 && !get_pixel(frame, x + 1, y + 1) {
        swap_pixels(frame, x, y, x + 1, y + 1);
    }
}

fn update(frame: &mut [u8]) {
    for y in (0..ROWS - 1).rev() {
        for x in (0..PIXEL_WIDTH).rev() {
            update_pixel(frame, x, y);
        }
    }
}

// Improved intro with much more visible pattern
fn draw_intro(frame: &mut [u8]) {
    // Use Playdate's native text drawing instead of manual pixels
    let graphics = Graphics::Cached();

    // Clear the frame first
    clear(frame);

    let _ = graphics.set_draw_mode(BitmapDrawMode::kDrawModeFillWhite);

    // Draw text using Playdate's built-in font system
    // This should work with the graphics API
    graphics.draw_text("FALLING SAND", 120, 100).unwrap();
    graphics
        .draw_text("Press any button to start", 80, 130)
        .unwrap();
    graphics
        .draw_text("A: Drop sand  B: Clear", 90, 160)
        .unwrap();
    graphics
        .draw_text("Left/Right: Move cursor", 85, 180)
        .unwrap();
}

const STEPS: usize = 3;
const SAND_BRUSH_SIZE: usize = 5;

fn process_input(game: &mut FallingSand) {
    let frame = Graphics::Cached().get_frame().unwrap();
    let buttons = Buttons::Cached().get();

    if buttons.current.any() {
        game.started = true;
    }

    if buttons.current.a() {
        let half_size = SAND_BRUSH_SIZE / 2;
        for i in 0..SAND_BRUSH_SIZE {
            for j in 0..SAND_BRUSH_SIZE {
                let x = game.position + i - half_size;
                let y = j;
                if x < PIXEL_WIDTH && y < ROWS {
                    set_pixel(frame, x, y, true);
                }
            }
        }
    }

    if buttons.current.left() {
        if game.position > SAND_BRUSH_SIZE {
            game.position -= 5;
        }
    }

    if buttons.current.right() {
        if game.position < PIXEL_WIDTH - SAND_BRUSH_SIZE {
            game.position += 5;
        }
    }

    if buttons.current.b() {
        clear(frame);
        if !game.started {
            draw_intro(frame);
        }
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
            position: PIXEL_WIDTH / 2,
        }
    }

    fn update(&mut self, _playdate: &Playdate) {
        let graphics = Graphics::Cached();
        process_input(self);
        graphics.mark_updated_rows(0, LCD_ROWS as i32);
        System::Cached().draw_fps(0, 228);
    }
}

game_loop!(FallingSand);
