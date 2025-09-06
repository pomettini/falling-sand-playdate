#![no_std]

extern crate alloc;
extern crate playdate as pd;

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use crankit_game_loop::{game_loop, Game, Playdate};
use pd::controls::buttons::PDButtonsExt;
use pd::controls::peripherals::{Buttons, Crank};
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

// Platform structure with position and angle
#[derive(Clone, Copy)]
struct Platform {
    x: usize,
    y: usize,
    angle: u32,
}

impl Platform {
    fn new(x: usize, y: usize, angle: u32) -> Self {
        Platform { x, y, angle }
    }

    fn update_angle(&mut self, delta: i32) {
        let angle = (self.angle as i32 + delta) % 360;
        self.angle = if angle < 0 {
            (angle + 360) as u32
        } else {
            angle as u32
        };
    }
}

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

// Simple pseudo-random number generator (Linear Congruential Generator)
static mut RAND_SEED: u32 = 1;

fn simple_rand() -> u32 {
    unsafe {
        RAND_SEED = RAND_SEED.wrapping_mul(1103515245).wrapping_add(12345);
        RAND_SEED
    }
}

fn rand_range(min: usize, max: usize) -> usize {
    if max <= min {
        return min;
    }
    min + (simple_rand() as usize % (max - min))
}

// Convert angle to directional steps for drawing
fn angle_to_steps(angle_degrees: u32) -> (i32, i32) {
    let angle = angle_degrees % 360;

    match angle {
        0..=22 | 338..=359 => (1, 0), // East
        23..=67 => (1, 1),            // Northeast
        68..=112 => (0, 1),           // North
        113..=157 => (-1, 1),         // Northwest
        158..=202 => (-1, 0),         // West
        203..=247 => (-1, -1),        // Southwest
        248..=292 => (0, -1),         // South
        293..=337 => (1, -1),         // Southeast
        _ => (1, 0),                  // fallback
    }
}

// Draw a single platform with its current angle
fn draw_platform(platform_buffer: &mut [u8], platform: &Platform) {
    let (dx, dy) = angle_to_steps(platform.angle);
    let platform_length = 25;

    let start_x = platform.x as i32;
    let start_y = platform.y as i32;

    for step in 0..platform_length {
        let x = start_x + (step as i32 * dx);
        let y = start_y + (step as i32 * dy);

        if x >= 0 && y >= 0 && (x as usize) < PIXEL_WIDTH && (y as usize) < ROWS {
            // Make platforms 2-3 pixels thick
            set_pixel(platform_buffer, x as usize, y as usize, true);

            if x + 1 >= 0 && ((x + 1) as usize) < PIXEL_WIDTH {
                set_pixel(platform_buffer, (x + 1) as usize, y as usize, true);
            }
            if y + 1 >= 0 && ((y + 1) as usize) < ROWS {
                set_pixel(platform_buffer, x as usize, (y + 1) as usize, true);
            }
            if x + 1 >= 0
                && y + 1 >= 0
                && ((x + 1) as usize) < PIXEL_WIDTH
                && ((y + 1) as usize) < ROWS
            {
                set_pixel(platform_buffer, (x + 1) as usize, (y + 1) as usize, true);
            }
        }
    }
}

// Redraw all platforms to the platform buffer - ALWAYS CALLED EVERY FRAME
fn redraw_platforms(platform_buffer: &mut [u8], platforms: &[Platform]) {
    clear_buffer(platform_buffer);

    // Draw static horizontal platforms
    for x in 50..350 {
        set_pixel(platform_buffer, x, ROWS - 20, true);
    }

    for x in 80..200 {
        set_pixel(platform_buffer, x, ROWS - 60, true);
    }

    for x in 250..370 {
        set_pixel(platform_buffer, x, ROWS - 80, true);
    }

    for x in 150..300 {
        set_pixel(platform_buffer, x, ROWS - 120, true);
    }

    // Draw all rotating diagonal platforms
    for platform in platforms {
        draw_platform(platform_buffer, platform);
    }
}

// Simple falling sand physics with platform collision
#[inline]
fn update_pixel(sand_buffer: &mut [u8], platform_buffer: &[u8], x: usize, y: usize) -> bool {
    if !is_sand(sand_buffer, x, y) {
        return false;
    }

    if y >= ROWS - 1 {
        return false;
    }

    if !is_solid(sand_buffer, platform_buffer, x, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x, y + 1, true);
        return true;
    }

    if x > 0 && !is_solid(sand_buffer, platform_buffer, x - 1, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x - 1, y + 1, true);
        return true;
    }

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

fn copy_to_frame(sand_buffer: &[u8], platform_buffer: &[u8], frame: &mut [u8]) {
    let copy_len = BUFFER_SIZE
        .min(frame.len())
        .min(sand_buffer.len())
        .min(platform_buffer.len());
    for i in 0..copy_len {
        frame[i] = sand_buffer[i] | platform_buffer[i];
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
    graphics
        .draw_text("Crank: Rotate all platforms", 75, 180)
        .unwrap();
    graphics.draw_text("Arrows: Move cursor", 95, 200).unwrap();
}

// Create initial platforms with random angles
fn create_initial_platforms() -> Vec<Platform> {
    unsafe {
        RAND_SEED = 12345;
    }

    let mut platforms = Vec::new();

    // Generate 10 random diagonal platforms with random angles (0-360 degrees)
    for _ in 0..10 {
        let x = rand_range(50, PIXEL_WIDTH - 50);
        let y = rand_range(30, ROWS - 50);
        let angle = simple_rand() % 360;

        platforms.push(Platform::new(x, y, angle));
    }

    platforms
}

const SAND_BRUSH_SIZE: usize = 5;

fn process_input(game: &mut FallingSand) {
    let frame = Graphics::Cached().get_frame().unwrap();
    let buttons = Buttons::Cached().get();
    let crank = Crank::Cached();

    if buttons.current.any() && !game.started {
        game.started = true;
        convert_intro_to_sand(frame, &mut *game.sand_buffer);
    }

    // NEW FEATURE: Endless sand rain from the top!
    if game.started {
        let rain_rate = 8; // particles per frame
        for _ in 0..rain_rate {
            let x = rand_range(0, PIXEL_WIDTH);
            set_pixel(&mut *game.sand_buffer, x, 0, true);
        }
    }

    // Sand placement with A button (additional sand on top of rain)
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

    // Crank rotation affects ALL platforms - processed every frame
    let crank_change = crank.change();
    if crank_change.abs() > 1.0 {
        let angle_delta = (crank_change / 5.0) as i32;

        // Apply rotation to ALL platforms
        for platform in &mut game.platforms {
            platform.update_angle(angle_delta);
        }
    }

    // Arrow key movement - works all the time now
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

    // B button only clears sand, platforms stay visible
    if buttons.current.b() {
        // Only clear sand buffer, keep platforms
        clear_buffer(&mut *game.sand_buffer);

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
        copy_to_frame(&*game.sand_buffer, &*game.platform_buffer, frame);
        draw_intro();
        return;
    }

    if game.frame_counter % 16 == 0 {
        game.screen_density = calculate_screen_density(&*game.sand_buffer);
    }

    let mut changed_rows = [false; ROWS];

    let (steps, skip_pattern) = match game.screen_density {
        0..=25 => (3, 1),
        26..=50 => (2, 1),
        51..=75 => (2, 2),
        _ => (1, 3),
    };

    for _ in 0..steps {
        update_optimized(
            &mut *game.sand_buffer,
            &*game.platform_buffer,
            &mut changed_rows,
            skip_pattern,
        );
    }

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
    sand_buffer: Box<[u8; BUFFER_SIZE]>,
    platform_buffer: Box<[u8; BUFFER_SIZE]>,
    platforms: Vec<Platform>,
}

impl Game for FallingSand {
    fn new(_playdate: &Playdate) -> Self {
        Display::Cached().set_refresh_rate(50.0);
        let frame = Graphics::Cached().get_frame().unwrap();

        for f in frame.iter_mut() {
            *f = 0;
        }

        let sand_buffer = Box::new([0; BUFFER_SIZE]);
        let mut platform_buffer = Box::new([0; BUFFER_SIZE]);

        let platforms = create_initial_platforms();
        redraw_platforms(&mut *platform_buffer, &platforms);

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
            platforms,
        }
    }

    fn update(&mut self, _playdate: &Playdate) {
        // Always redraw platforms EVERY frame before processing input
        redraw_platforms(&mut *self.platform_buffer, &self.platforms);

        // Process all input
        process_input(self);

        // Draw UI elements
        System::Cached().draw_fps(0, 228);
        if self.started {
            let graphics = Graphics::Cached();
            let _ = graphics.set_draw_mode(BitmapDrawMode::kDrawModeFillWhite);
            let text = format!("Platforms: {} - Rain Mode", self.platforms.len());
            graphics.draw_text(&text, 10, 10).unwrap();
        }
    }
}

game_loop!(FallingSand);
