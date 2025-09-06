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

// Pre-computed lookup tables for ultra-fast bit operations
static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
static INV_BIT_MASKS: [u8; 8] = [0x7F, 0xBF, 0xDF, 0xEF, 0xF7, 0xFB, 0xFD, 0xFE];

fn clear(frame: &mut [u8]) {
    // Safe and complete clearing of frame buffer
    for f in frame.iter_mut().take(COLUMNS * ROWS) {
        *f = 0;
    }
}

#[inline(always)]
fn get_pixel_ultra_fast(frame: &[u8], x: usize, y: usize) -> bool {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return false;
    }
    let index = y * COLUMNS + (x >> 3);
    (frame[index] & BIT_MASKS[x & 7]) != 0
}

#[inline(always)]
fn set_pixel_ultra_fast(frame: &mut [u8], x: usize, y: usize, value: bool) {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return;
    }
    let index = y * COLUMNS + (x >> 3);
    let bit_idx = x & 7;
    if value {
        frame[index] |= BIT_MASKS[bit_idx];
    } else {
        frame[index] &= INV_BIT_MASKS[bit_idx];
    }
}

#[inline]
fn update_pixel_ultra_fast(frame: &mut [u8], x: usize, y: usize) -> bool {
    if y >= ROWS - 1 || !get_pixel_ultra_fast(frame, x, y) {
        return false;
    }

    let can_move_down = !get_pixel_ultra_fast(frame, x, y + 1);
    if can_move_down {
        set_pixel_ultra_fast(frame, x, y, false);
        set_pixel_ultra_fast(frame, x, y + 1, true);
        return true;
    }

    let can_move_left = x > 0 && !get_pixel_ultra_fast(frame, x - 1, y + 1);
    if can_move_left {
        set_pixel_ultra_fast(frame, x, y, false);
        set_pixel_ultra_fast(frame, x - 1, y + 1, true);
        return true;
    }

    let can_move_right = x < PIXEL_WIDTH - 1 && !get_pixel_ultra_fast(frame, x + 1, y + 1);
    if can_move_right {
        set_pixel_ultra_fast(frame, x, y, false);
        set_pixel_ultra_fast(frame, x + 1, y + 1, true);
        return true;
    }

    false
}

fn update_optimized(frame: &mut [u8], changed_rows: &mut [bool; ROWS], skip_pattern: usize) {
    for y in (0..ROWS - 1).rev() {
        if y % skip_pattern != 0 {
            continue;
        }

        let row_start = y * COLUMNS;
        let mut row_changed = false;

        for byte_idx in 0..COLUMNS {
            let byte_val = frame[row_start + byte_idx];
            if byte_val == 0 {
                continue;
            }

            let base_x = byte_idx << 3;

            // Unrolled bit processing for maximum performance
            if (byte_val & 0x80) != 0 && update_pixel_ultra_fast(frame, base_x, y) {
                row_changed = true;
            }
            if (byte_val & 0x40) != 0 && update_pixel_ultra_fast(frame, base_x + 1, y) {
                row_changed = true;
            }
            if (byte_val & 0x20) != 0 && update_pixel_ultra_fast(frame, base_x + 2, y) {
                row_changed = true;
            }
            if (byte_val & 0x10) != 0 && update_pixel_ultra_fast(frame, base_x + 3, y) {
                row_changed = true;
            }
            if (byte_val & 0x08) != 0 && update_pixel_ultra_fast(frame, base_x + 4, y) {
                row_changed = true;
            }
            if (byte_val & 0x04) != 0 && update_pixel_ultra_fast(frame, base_x + 5, y) {
                row_changed = true;
            }
            if (byte_val & 0x02) != 0 && update_pixel_ultra_fast(frame, base_x + 6, y) {
                row_changed = true;
            }
            if (byte_val & 0x01) != 0 && update_pixel_ultra_fast(frame, base_x + 7, y) {
                row_changed = true;
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

fn calculate_screen_density(frame: &[u8]) -> u8 {
    let mut pixel_count = 0u32;
    for i in (0..frame.len()).step_by(8) {
        if frame[i] != 0 {
            pixel_count += frame[i].count_ones();
        }
    }
    ((pixel_count * 100) / ((frame.len() / 8) * 8) as u32).min(100) as u8
}

fn update_screen_efficiently(changed_rows: &[bool; ROWS]) {
    let graphics = Graphics::Cached();
    let mut batch_start: Option<usize> = None;

    for (y, &changed) in changed_rows.iter().enumerate() {
        if changed && batch_start.is_none() {
            batch_start = Some(y);
        } else if !changed && batch_start.is_some() {
            graphics.mark_updated_rows(batch_start.unwrap() as i32, y as i32);
            batch_start = None;
        }
    }

    if let Some(start) = batch_start {
        graphics.mark_updated_rows(start as i32, ROWS as i32);
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
    graphics.draw_text("Arrows: Move cursor", 95, 180).unwrap();
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
                let x = game.position_x + i - half_size;
                let y = game.position_y + j - half_size;
                if x < PIXEL_WIDTH && y < ROWS {
                    set_pixel_ultra_fast(frame, x, y, true);
                }
            }
        }
    }

    // Horizontal movement
    if buttons.current.left() && game.position_x > SAND_BRUSH_SIZE {
        game.position_x -= 5;
    }

    if buttons.current.right() && game.position_x < PIXEL_WIDTH - SAND_BRUSH_SIZE {
        game.position_x += 5;
    }

    // Vertical movement - NEW!
    if buttons.current.up() && game.position_y > SAND_BRUSH_SIZE {
        game.position_y -= 5;
    }

    if buttons.current.down() && game.position_y < ROWS - SAND_BRUSH_SIZE {
        game.position_y += 5;
    }

    if buttons.current.b() {
        clear(frame);

        // CRITICAL FIX: Mark all rows as updated after clearing to prevent artifacts
        let graphics = Graphics::Cached();
        graphics.mark_updated_rows(0, LCD_ROWS as i32);

        if !game.started {
            draw_intro(frame);
        }

        // Reset performance tracking after clear
        game.screen_density = 0;
        return; // Skip physics simulation this frame
    }

    if !game.started {
        return;
    }

    // Calculate density every 16 frames to reduce overhead
    if game.frame_counter % 16 == 0 {
        game.screen_density = calculate_screen_density(frame);
    }

    let mut changed_rows = [false; ROWS];

    // Ultra-aggressive performance scaling based on screen density
    let (steps, skip_pattern) = match game.screen_density {
        0..=25 => (3, 1),  // Light density: full quality
        26..=50 => (2, 1), // Medium density: fewer steps
        51..=75 => (2, 2), // High density: skip every other row
        _ => (1, 3),       // Extreme density: minimal simulation
    };

    for _ in 0..steps {
        update_optimized(frame, &mut changed_rows, skip_pattern);
    }

    update_screen_efficiently(&changed_rows);
    game.frame_counter += 1;
}

struct FallingSand {
    started: bool,
    position_x: usize, // Horizontal cursor position
    position_y: usize, // Vertical cursor position - NEW!
    frame_counter: u32,
    screen_density: u8,
}

impl Game for FallingSand {
    fn new(_playdate: &Playdate) -> Self {
        Display::Cached().set_refresh_rate(50.0);
        let frame = Graphics::Cached().get_frame().unwrap();
        clear(frame);
        draw_intro(frame);
        Self {
            started: false,
            position_x: PIXEL_WIDTH / 2, // Start in horizontal center
            position_y: ROWS / 4,        // Start in upper portion of screen
            frame_counter: 0,
            screen_density: 0,
        }
    }

    fn update(&mut self, _playdate: &Playdate) {
        process_input(self);
        System::Cached().draw_fps(0, 228);
    }
}

game_loop!(FallingSand);
