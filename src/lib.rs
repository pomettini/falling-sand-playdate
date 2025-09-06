#![no_std]

extern crate alloc;
extern crate playdate as pd;

use alloc::boxed::Box;
use crankit_game_loop::{game_loop, Game, Playdate};
use pd::controls::buttons::PDButtonsExt;
use pd::controls::peripherals::Buttons;
use pd::display::Display;
use pd::graphics::BitmapDrawMode;
use pd::sys::ffi::{LCD_COLUMNS, LCD_ROWS};
use pd::system::System;
use playdate::graphics::Graphics;

const ROWS: usize = LCD_ROWS as usize;
const COLUMNS: usize = 2 + (LCD_COLUMNS / 8) as usize;
const PIXEL_WIDTH: usize = LCD_COLUMNS as usize;
const BUFFER_SIZE: usize = COLUMNS * ROWS;

// Pre-computed lookup tables for ultra-fast bit operations
static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
static INV_BIT_MASKS: [u8; 8] = [0x7F, 0xBF, 0xDF, 0xEF, 0xF7, 0xFB, 0xFD, 0xFE];

fn clear_buffer(buffer: &mut [u8]) {
    // Safe and complete clearing of buffer
    for f in buffer.iter_mut().take(BUFFER_SIZE) {
        *f = 0;
    }
}

#[inline(always)]
fn get_pixel(buffer: &[u8], x: usize, y: usize) -> bool {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return false;
    }
    let index = y * COLUMNS + (x >> 3);
    (buffer[index] & BIT_MASKS[x & 7]) != 0
}

#[inline(always)]
fn set_pixel(buffer: &mut [u8], x: usize, y: usize, value: bool) {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return;
    }
    let index = y * COLUMNS + (x >> 3);
    let bit_idx = x & 7;
    if value {
        buffer[index] |= BIT_MASKS[bit_idx];
    } else {
        buffer[index] &= INV_BIT_MASKS[bit_idx];
    }
}

// Simple falling sand physics
#[inline]
fn update_pixel(buffer: &mut [u8], x: usize, y: usize) -> bool {
    if y >= ROWS - 1 || !get_pixel(buffer, x, y) {
        return false;
    }

    // Try to move down
    let can_move_down = !get_pixel(buffer, x, y + 1);
    if can_move_down {
        set_pixel(buffer, x, y, false);
        set_pixel(buffer, x, y + 1, true);
        return true;
    }

    // Try to move down-left
    let can_move_left = x > 0 && !get_pixel(buffer, x - 1, y + 1);
    if can_move_left {
        set_pixel(buffer, x, y, false);
        set_pixel(buffer, x - 1, y + 1, true);
        return true;
    }

    // Try to move down-right
    let can_move_right = x < PIXEL_WIDTH - 1 && !get_pixel(buffer, x + 1, y + 1);
    if can_move_right {
        set_pixel(buffer, x, y, false);
        set_pixel(buffer, x + 1, y + 1, true);
        return true;
    }

    false
}

fn update_optimized(buffer: &mut [u8], changed_rows: &mut [bool; ROWS], skip_pattern: usize) {
    for y in (0..ROWS - 1).rev() {
        if y % skip_pattern != 0 {
            continue;
        }

        let row_start = y * COLUMNS;
        let mut row_changed = false;

        for byte_idx in 0..COLUMNS {
            let byte_val = buffer[row_start + byte_idx];
            if byte_val == 0 {
                continue;
            }

            let base_x = byte_idx << 3;

            // Unrolled bit processing for maximum performance
            if (byte_val & 0x80) != 0 && update_pixel(buffer, base_x, y) {
                row_changed = true;
            }
            if (byte_val & 0x40) != 0 && update_pixel(buffer, base_x + 1, y) {
                row_changed = true;
            }
            if (byte_val & 0x20) != 0 && update_pixel(buffer, base_x + 2, y) {
                row_changed = true;
            }
            if (byte_val & 0x10) != 0 && update_pixel(buffer, base_x + 3, y) {
                row_changed = true;
            }
            if (byte_val & 0x08) != 0 && update_pixel(buffer, base_x + 4, y) {
                row_changed = true;
            }
            if (byte_val & 0x04) != 0 && update_pixel(buffer, base_x + 5, y) {
                row_changed = true;
            }
            if (byte_val & 0x02) != 0 && update_pixel(buffer, base_x + 6, y) {
                row_changed = true;
            }
            if (byte_val & 0x01) != 0 && update_pixel(buffer, base_x + 7, y) {
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

fn calculate_screen_density(buffer: &[u8]) -> u8 {
    let mut pixel_count = 0u32;
    for i in (0..buffer.len()).step_by(8) {
        if buffer[i] != 0 {
            pixel_count += buffer[i].count_ones();
        }
    }
    ((pixel_count * 100) / ((buffer.len() / 8) * 8) as u32).min(100) as u8
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

fn draw_intro() {
    let graphics = Graphics::Cached();
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
                    set_pixel(&mut *game.logic_buffer, x, y, true); // Fixed: dereference Box
                }
            }
        }
    }

    // Arrow key movement
    if buttons.current.left() && game.position_x > SAND_BRUSH_SIZE {
        game.position_x -= 5;
    }

    if buttons.current.right() && game.position_x < PIXEL_WIDTH - SAND_BRUSH_SIZE {
        game.position_x += 5;
    }

    if buttons.current.up() && game.position_y > SAND_BRUSH_SIZE {
        game.position_y -= 5;
    }

    if buttons.current.down() && game.position_y < ROWS - SAND_BRUSH_SIZE {
        game.position_y += 5;
    }

    if buttons.current.b() {
        clear_buffer(&mut *game.logic_buffer); // Fixed: dereference Box

        // Clear frame buffer and mark all rows for update
        for f in frame.iter_mut() {
            *f = 0;
        }
        let graphics = Graphics::Cached();
        graphics.mark_updated_rows(0, LCD_ROWS as i32);

        if !game.started {
            draw_intro();
        }

        game.screen_density = 0;
        return;
    }

    if !game.started {
        // Copy logic buffer to frame buffer
        game.copy_logic_to_frame(frame);
        draw_intro();
        return;
    }

    // Calculate density every 16 frames to reduce overhead
    if game.frame_counter % 16 == 0 {
        game.screen_density = calculate_screen_density(&*game.logic_buffer); // Fixed: dereference Box
    }

    let mut changed_rows = [false; ROWS];

    // Performance scaling based on screen density
    let (steps, skip_pattern) = match game.screen_density {
        0..=25 => (3, 1),  // Light density: full quality
        26..=50 => (2, 1), // Medium density: fewer steps
        51..=75 => (2, 2), // High density: skip every other row
        _ => (1, 3),       // Extreme density: minimal simulation
    };

    for _ in 0..steps {
        update_optimized(&mut *game.logic_buffer, &mut changed_rows, skip_pattern);
        // Fixed: dereference Box
    }

    // Copy logic buffer to frame buffer for rendering
    game.copy_logic_to_frame(frame);

    update_screen_efficiently(&changed_rows);
    game.frame_counter += 1;
}

struct FallingSand {
    started: bool,
    position_x: usize,
    position_y: usize,
    frame_counter: u32,
    screen_density: u8,
    logic_buffer: Box<[u8; BUFFER_SIZE]>, // Heap-allocated buffer to avoid stack overflow
}

impl FallingSand {
    fn copy_logic_to_frame(&self, frame: &mut [u8]) {
        // Copy logic buffer to frame buffer for rendering
        let copy_len = BUFFER_SIZE.min(frame.len());
        for i in 0..copy_len {
            frame[i] = self.logic_buffer[i]; // Direct indexing works fine
        }
    }
}

impl Game for FallingSand {
    fn new(_playdate: &Playdate) -> Self {
        Display::Cached().set_refresh_rate(50.0);
        let frame = Graphics::Cached().get_frame().unwrap();

        // Clear frame buffer
        for f in frame.iter_mut() {
            *f = 0;
        }

        // Show intro
        draw_intro();

        Self {
            started: false,
            position_x: PIXEL_WIDTH / 2,
            position_y: ROWS / 4,
            frame_counter: 0,
            screen_density: 0,
            logic_buffer: Box::new([0; BUFFER_SIZE]), // Heap allocation to avoid stack overflow
        }
    }

    fn update(&mut self, _playdate: &Playdate) {
        process_input(self);

        // Draw UI elements on top of the game (after logic buffer copy)
        System::Cached().draw_fps(0, 0);
    }
}

game_loop!(FallingSand);
