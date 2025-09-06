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
    if index >= buffer.len() {
        return false;
    }
    (buffer[index] & BIT_MASKS[x & 7]) != 0
}

#[inline(always)]
fn set_pixel(buffer: &mut [u8], x: usize, y: usize, value: bool) {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return;
    }
    let index = y * COLUMNS + (x >> 3);
    if index >= buffer.len() {
        return;
    }
    let bit_idx = x & 7;
    if value {
        buffer[index] |= BIT_MASKS[bit_idx];
    } else {
        buffer[index] &= INV_BIT_MASKS[bit_idx];
    }
}

#[inline(always)]
fn is_platform(platform_buffer: &[u8], x: usize, y: usize) -> bool {
    get_pixel(platform_buffer, x, y)
}

#[inline(always)]
fn is_sand(sand_buffer: &[u8], x: usize, y: usize) -> bool {
    get_pixel(sand_buffer, x, y)
}

#[inline(always)]
fn is_solid(sand_buffer: &[u8], platform_buffer: &[u8], x: usize, y: usize) -> bool {
    is_sand(sand_buffer, x, y) || is_platform(platform_buffer, x, y)
}

// Convert intro text pixels to sand particles
fn convert_intro_to_sand(frame: &[u8], sand_buffer: &mut [u8]) {
    for y in 0..ROWS {
        for x in 0..PIXEL_WIDTH {
            let byte_index = y * COLUMNS + (x / 8);
            let bit_index = x % 8;
            if byte_index < frame.len() {
                if (frame[byte_index] & BIT_MASKS[bit_index]) != 0 {
                    set_pixel(sand_buffer, x, y, true);
                }
            }
        }
    }
}

// Simple falling sand physics with platform collision
#[inline]
fn update_pixel(sand_buffer: &mut [u8], platform_buffer: &[u8], x: usize, y: usize) -> bool {
    // Only sand can move
    if !is_sand(sand_buffer, x, y) {
        return false;
    }

    if y >= ROWS - 1 {
        return false;
    }

    // Try to move down
    if !is_solid(sand_buffer, platform_buffer, x, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x, y + 1, true);
        return true;
    }

    // Try to move down-left
    if x > 0 && !is_solid(sand_buffer, platform_buffer, x - 1, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x - 1, y + 1, true);
        return true;
    }

    // Try to move down-right
    if x < PIXEL_WIDTH - 1 && !is_solid(sand_buffer, platform_buffer, x + 1, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x + 1, y + 1, true);
        return true;
    }

    false
}

fn update_optimized(
    sand_buffer: &mut [u8],
    platform_buffer: &[u8],
    changed_rows: &mut [bool; ROWS],
    skip_pattern: usize,
) {
    for y in (0..ROWS - 1).rev() {
        if y % skip_pattern != 0 {
            continue;
        }

        let row_start = y * COLUMNS;
        let mut row_changed = false;

        for byte_idx in 0..COLUMNS {
            if row_start + byte_idx >= sand_buffer.len() {
                break;
            }

            let byte_val = sand_buffer[row_start + byte_idx];
            if byte_val == 0 {
                continue;
            }

            let base_x = byte_idx << 3;

            // Unrolled bit processing for maximum performance
            if (byte_val & 0x80) != 0 && update_pixel(sand_buffer, platform_buffer, base_x, y) {
                row_changed = true;
            }
            if (byte_val & 0x40) != 0 && update_pixel(sand_buffer, platform_buffer, base_x + 1, y) {
                row_changed = true;
            }
            if (byte_val & 0x20) != 0 && update_pixel(sand_buffer, platform_buffer, base_x + 2, y) {
                row_changed = true;
            }
            if (byte_val & 0x10) != 0 && update_pixel(sand_buffer, platform_buffer, base_x + 3, y) {
                row_changed = true;
            }
            if (byte_val & 0x08) != 0 && update_pixel(sand_buffer, platform_buffer, base_x + 4, y) {
                row_changed = true;
            }
            if (byte_val & 0x04) != 0 && update_pixel(sand_buffer, platform_buffer, base_x + 5, y) {
                row_changed = true;
            }
            if (byte_val & 0x02) != 0 && update_pixel(sand_buffer, platform_buffer, base_x + 6, y) {
                row_changed = true;
            }
            if (byte_val & 0x01) != 0 && update_pixel(sand_buffer, platform_buffer, base_x + 7, y) {
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

fn calculate_screen_density(sand_buffer: &[u8]) -> u8 {
    let mut pixel_count = 0u32;
    for i in (0..sand_buffer.len()).step_by(8) {
        if sand_buffer[i] != 0 {
            pixel_count += sand_buffer[i].count_ones();
        }
    }
    ((pixel_count * 100) / ((sand_buffer.len() / 8) * 8) as u32).min(100) as u8
}

// Combine sand and platform buffers into frame buffer for rendering
fn copy_to_frame(sand_buffer: &[u8], platform_buffer: &[u8], frame: &mut [u8]) {
    let copy_len = BUFFER_SIZE
        .min(frame.len())
        .min(sand_buffer.len())
        .min(platform_buffer.len());
    for i in 0..copy_len {
        frame[i] = sand_buffer[i] | platform_buffer[i]; // Combine both buffers
    }
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

// Create some initial platforms
fn create_initial_platforms(platform_buffer: &mut [u8]) {
    // Bottom platform
    for x in 50..350 {
        set_pixel(platform_buffer, x, ROWS - 20, true);
    }

    // Left platform
    for x in 80..200 {
        set_pixel(platform_buffer, x, ROWS - 60, true);
    }

    // Right platform
    for x in 250..370 {
        set_pixel(platform_buffer, x, ROWS - 80, true);
    }

    // Middle platform
    for x in 150..300 {
        set_pixel(platform_buffer, x, ROWS - 120, true);
    }
}

const SAND_BRUSH_SIZE: usize = 5;

fn process_input(game: &mut FallingSand) {
    let frame = Graphics::Cached().get_frame().unwrap();
    let buttons = Buttons::Cached().get();

    // Convert intro text to sand when game starts
    if buttons.current.any() && !game.started {
        game.started = true;
        // Convert any pixels currently in frame buffer (intro text) to sand particles
        convert_intro_to_sand(frame, &mut *game.sand_buffer);
    }

    // Sand placement
    if buttons.current.a() {
        let half_size = SAND_BRUSH_SIZE / 2;
        for i in 0..SAND_BRUSH_SIZE {
            for j in 0..SAND_BRUSH_SIZE {
                let x = game.position_x + i - half_size;
                let y = game.position_y + j - half_size;
                if x < PIXEL_WIDTH && y < ROWS {
                    set_pixel(&mut *game.sand_buffer, x, y, true);
                }
            }
        }
    }

    // Arrow key movement - now works freely while placing sand
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
        clear_buffer(&mut *game.sand_buffer);
        clear_buffer(&mut *game.platform_buffer);

        // Re-create initial platforms after clearing
        create_initial_platforms(&mut *game.platform_buffer);

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
        // Copy both buffers to frame buffer and show intro
        copy_to_frame(&*game.sand_buffer, &*game.platform_buffer, frame);
        draw_intro();
        return;
    }

    // Calculate density every 16 frames to reduce overhead
    if game.frame_counter % 16 == 0 {
        game.screen_density = calculate_screen_density(&*game.sand_buffer);
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
        update_optimized(
            &mut *game.sand_buffer,
            &*game.platform_buffer,
            &mut changed_rows,
            skip_pattern,
        );
    }

    // Copy both buffers to frame buffer for rendering
    copy_to_frame(&*game.sand_buffer, &*game.platform_buffer, frame);

    update_screen_efficiently(&changed_rows);
    game.frame_counter += 1;
}

struct FallingSand {
    started: bool,
    position_x: usize,
    position_y: usize,
    frame_counter: u32,
    screen_density: u8,
    sand_buffer: Box<[u8; BUFFER_SIZE]>, // Sand particles buffer
    platform_buffer: Box<[u8; BUFFER_SIZE]>, // Platform locations buffer
}

impl Game for FallingSand {
    fn new(_playdate: &Playdate) -> Self {
        Display::Cached().set_refresh_rate(50.0);
        let frame = Graphics::Cached().get_frame().unwrap();

        // Clear frame buffer
        for f in frame.iter_mut() {
            *f = 0;
        }

        // Create buffers
        let sand_buffer = Box::new([0; BUFFER_SIZE]);
        let mut platform_buffer = Box::new([0; BUFFER_SIZE]);

        // Add initial platforms
        create_initial_platforms(&mut *platform_buffer);

        // Show intro with platforms
        copy_to_frame(&*sand_buffer, &*platform_buffer, frame);
        draw_intro();

        Self {
            started: false,
            position_x: PIXEL_WIDTH / 2,
            position_y: ROWS / 4,
            frame_counter: 0,
            screen_density: 0,
            sand_buffer,
            platform_buffer,
        }
    }

    fn update(&mut self, _playdate: &Playdate) {
        process_input(self);

        // Draw UI elements on top of the game
        System::Cached().draw_fps(0, 228);
    }
}

game_loop!(FallingSand);
