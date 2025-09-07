#![no_std]

extern crate alloc;
extern crate playdate as pd;

use alloc::boxed::Box;
use crankit_game_loop::{game_loop, Game, Playdate};
use pd::controls::buttons::PDButtonsExt;
use pd::controls::peripherals::Buttons;
use pd::display::Display;
use pd::graphics::BitmapDrawMode;
use pd::sys::ffi::LCD_ROWS;
use pd::system::System;
use playdate::graphics::Graphics;
use rand::rngs::SmallRng;
use rand::SeedableRng;
mod consts;
mod utils;
use consts::*;
use utils::*;

// Pre-computed lookup tables for ultra-fast bit operations
static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
static INV_BIT_MASKS: [u8; 8] = [0x7F, 0xBF, 0xDF, 0xEF, 0xF7, 0xFB, 0xFD, 0xFE];

fn clear_buffer(buffer: &mut [u8]) {
    for f in buffer.iter_mut().take(BUFFER_SIZE) {
        *f = 0;
    }
}

#[inline(always)]
pub fn get_pixel(buffer: &[u8], x: usize, y: usize) -> bool {
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
fn is_sand(sand_buffer: &[u8], x: usize, y: usize) -> bool {
    get_pixel(sand_buffer, x, y)
}

// Convert intro text pixels to sand particles
fn convert_intro_to_sand(frame: &[u8], sand_buffer: &mut [u8]) {
    for y in 0..ROWS {
        for x in 0..PIXEL_WIDTH {
            let byte_index = y * COLUMNS + (x / 8);
            let bit_index = x % 8;
            if byte_index < frame.len() && (frame[byte_index] & BIT_MASKS[bit_index]) != 0 {
                set_pixel(sand_buffer, x, y, true);
            }
        }
    }
}

// Simple falling sand physics
fn update_sand(sand_buffer: &mut [u8], changed_rows: &mut [bool; ROWS]) {
    // Process sand from bottom to top
    for y in (0..ROWS).rev() {
        for x in 0..PIXEL_WIDTH {
            if !is_sand(sand_buffer, x, y) {
                continue;
            }

            let fell = update_pixel(sand_buffer, x, y);
            if fell {
                changed_rows[y] = true;
                if y + 1 < ROWS {
                    changed_rows[y + 1] = true;
                }
            }
        }
    }
}

// Basic falling sand physics
#[inline]
fn update_pixel(sand_buffer: &mut [u8], x: usize, y: usize) -> bool {
    if !is_sand(sand_buffer, x, y) {
        return false;
    }

    if y >= ROWS - 1 {
        return false;
    }

    // Try to move down
    if !is_sand(sand_buffer, x, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x, y + 1, true);
        return true;
    }

    // Try to move down-left
    if x > 0 && !is_sand(sand_buffer, x - 1, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x - 1, y + 1, true);
        return true;
    }

    // Try to move down-right
    if x < PIXEL_WIDTH - 1 && !is_sand(sand_buffer, x + 1, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x + 1, y + 1, true);
        return true;
    }

    false
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

fn copy_to_frame(sand_buffer: &[u8], frame: &mut [u8]) {
    let copy_len = BUFFER_SIZE.min(frame.len()).min(sand_buffer.len());
    for i in 0..copy_len {
        frame[i] = sand_buffer[i];
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
        .draw_text("A: Drop sand B: Clear", 90, 160)
        .unwrap();
}

fn process_input(game: &mut FallingSand) {
    let frame = Graphics::Cached().get_frame().unwrap();
    let buttons = Buttons::Cached().get();

    if buttons.current.any() && !game.started {
        game.started = true;
        convert_intro_to_sand(frame, &mut *game.sand_buffer);
    }

    // Endless sand rain from the top!
    if game.started {
        let rain_rate = 6;
        for _ in 0..rain_rate {
            let x = rand_range(0, PIXEL_WIDTH, &mut game.rng);
            set_pixel(&mut *game.sand_buffer, x, 0, true);
        }
    }

    // Sand placement with A button
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

    // B button clears sand
    if buttons.current.b() {
        clear_buffer(&mut *game.sand_buffer);
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
        copy_to_frame(&*game.sand_buffer, frame);
        draw_intro();
        return;
    }

    if game.frame_counter.is_multiple_of(16) {
        game.screen_density = calculate_screen_density(&*game.sand_buffer);
    }

    let mut changed_rows = [false; ROWS];
    let steps = match game.screen_density {
        0..=25 => 2,
        26..=50 => 2,
        51..=75 => 1,
        _ => 1,
    };

    for _ in 0..steps {
        update_sand(&mut *game.sand_buffer, &mut changed_rows);
    }

    copy_to_frame(&*game.sand_buffer, frame);
    update_screen_efficiently(&changed_rows);
    game.frame_counter += 1;
}

struct FallingSand {
    rng: SmallRng,
    started: bool,
    position_x: usize,
    position_y: usize,
    frame_counter: u32,
    screen_density: u8,
    sand_buffer: Box<[u8; BUFFER_SIZE]>,
}

impl Game for FallingSand {
    fn new(_playdate: &Playdate) -> Self {
        Display::Cached().set_refresh_rate(50.0);
        let frame = Graphics::Cached().get_frame().unwrap();
        for f in frame.iter_mut() {
            *f = 0;
        }

        let sand_buffer = Box::new([0; BUFFER_SIZE]);
        let time = System::Cached().seconds_since_epoch();
        let rng = SmallRng::seed_from_u64(u64::from(time));

        copy_to_frame(&*sand_buffer, frame);
        draw_intro();

        Self {
            rng,
            started: false,
            position_x: PIXEL_WIDTH / 2,
            position_y: ROWS / 4,
            frame_counter: 0,
            screen_density: 0,
            sand_buffer,
        }
    }

    fn update(&mut self, _playdate: &Playdate) {
        process_input(self);
        System::Cached().draw_fps(0, 0);
    }
}

game_loop!(FallingSand);
