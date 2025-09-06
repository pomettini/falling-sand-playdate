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

fn clear(frame: &mut [u8]) {
    for f in frame.iter_mut().take(COLUMNS * ROWS) {
        *f = 0;
    }
}

#[inline(always)]
fn get_pixel_fast(frame: &[u8], x: usize, y: usize) -> bool {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return false;
    }
    let index = y * COLUMNS + (x >> 3);
    if index < frame.len() {
        (frame[index] & (1 << (7 - (x & 7)))) != 0
    } else {
        false
    }
}

#[inline(always)]
fn set_pixel_fast(frame: &mut [u8], x: usize, y: usize, value: bool) {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return;
    }
    let index = y * COLUMNS + (x >> 3);
    if index < frame.len() {
        let bit_mask = 1 << (7 - (x & 7));
        if value {
            frame[index] |= bit_mask;
        } else {
            frame[index] &= !bit_mask;
        }
    }
}

#[inline]
fn update_pixel_fast(frame: &mut [u8], x: usize, y: usize) -> bool {
    if y >= ROWS - 1 || !get_pixel_fast(frame, x, y) {
        return false;
    }

    if !get_pixel_fast(frame, x, y + 1) {
        set_pixel_fast(frame, x, y, false);
        set_pixel_fast(frame, x, y + 1, true);
        true
    } else if x > 0 && !get_pixel_fast(frame, x - 1, y + 1) {
        set_pixel_fast(frame, x, y, false);
        set_pixel_fast(frame, x - 1, y + 1, true);
        true
    } else if x < PIXEL_WIDTH - 1 && !get_pixel_fast(frame, x + 1, y + 1) {
        set_pixel_fast(frame, x, y, false);
        set_pixel_fast(frame, x + 1, y + 1, true);
        true
    } else {
        false
    }
}

fn update_with_dirty_tracking(frame: &mut [u8], changed_rows: &mut [bool; ROWS]) {
    const CHUNK_SIZE: usize = 8;

    for chunk_start in (0..ROWS.saturating_sub(1)).step_by(CHUNK_SIZE).rev() {
        let chunk_end = (chunk_start + CHUNK_SIZE).min(ROWS - 1);

        for y in (chunk_start..chunk_end).rev() {
            let row_start = y * COLUMNS;
            let mut row_changed = false;

            for byte_idx in 0..COLUMNS {
                let byte_val = frame[row_start + byte_idx];
                if byte_val == 0 {
                    continue;
                }

                let base_x = byte_idx << 3; // * 8
                for bit_idx in 0..8 {
                    if (byte_val & (1 << (7 - bit_idx))) != 0 {
                        let x = base_x + bit_idx;
                        if x < PIXEL_WIDTH && update_pixel_fast(frame, x, y) {
                            row_changed = true;
                        }
                    }
                }
            }

            if row_changed {
                changed_rows[y] = true;
                if y > 0 {
                    changed_rows[y - 1] = true;
                }
                if y < ROWS - 1 {
                    changed_rows[y + 1] = true;
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

const SAND_BRUSH_SIZE: usize = 33;

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
                    set_pixel_fast(frame, x, y, true);
                }
            }
        }
    }

    if buttons.current.left() && game.position > SAND_BRUSH_SIZE {
        game.position -= 5;
    }

    if buttons.current.right() && game.position < PIXEL_WIDTH - SAND_BRUSH_SIZE {
        game.position += 5;
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

    let mut changed_rows = [false; ROWS];
    let steps = if game.frame_counter % 4 == 0 { 2 } else { 1 };

    for _ in 0..steps {
        update_with_dirty_tracking(frame, &mut changed_rows);
    }

    // Update only changed rows
    let graphics = Graphics::Cached();
    let mut start_row = None;
    for (y, &changed) in changed_rows.iter().enumerate() {
        if changed && start_row.is_none() {
            start_row = Some(y);
        } else if !changed && start_row.is_some() {
            graphics.mark_updated_rows(start_row.unwrap() as i32, y as i32);
            start_row = None;
        }
    }
    if let Some(start) = start_row {
        graphics.mark_updated_rows(start as i32, ROWS as i32);
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
        process_input(self);
        System::Cached().draw_fps(0, 228);
    }
}

game_loop!(FallingSand);
