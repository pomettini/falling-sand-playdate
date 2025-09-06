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

// Simple optimization: skip empty bytes entirely
fn update(frame: &mut [u8]) {
    // Process from bottom to top for proper physics
    for y in (0..ROWS - 1).rev() {
        let row_start = y * COLUMNS;

        // Quick check: if entire row is empty, skip it
        let mut row_has_pixels = false;
        for byte_idx in 0..COLUMNS {
            if frame[row_start + byte_idx] != 0 {
                row_has_pixels = true;
                break;
            }
        }

        if !row_has_pixels {
            continue;
        }

        // Process pixels in this row, but skip empty bytes
        for byte_idx in 0..COLUMNS {
            let byte_val = frame[row_start + byte_idx];
            if byte_val == 0 {
                continue; // Skip empty bytes
            }

            // Only check pixels in non-empty bytes
            for bit_idx in 0..8 {
                if (byte_val & (1 << (7 - bit_idx))) != 0 {
                    let x = byte_idx * 8 + bit_idx;
                    if x < PIXEL_WIDTH {
                        update_pixel(frame, x, y);
                    }
                }
            }
        }
    }
}

fn draw_intro(frame: &mut [u8]) {
    let graphics = Graphics::Cached();
    clear(frame);
    let _ = graphics.set_draw_mode(BitmapDrawMode::kDrawModeFillWhite);
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

    // Simple adaptive performance: fewer steps when more sand
    let steps = if game.frame_counter % 3 == 0 { 2 } else { 1 };

    for _ in 0..steps {
        update(frame);
    }

    game.frame_counter += 1;
}

struct FallingSand {
    started: bool,
    position: usize,
    frame_counter: u32,
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
            frame_counter: 0,
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
