#![no_std]

extern crate alloc;
extern crate playdate as pd;

use core::f32::consts::PI;

use alloc::boxed::Box;
use alloc::vec::Vec;
use crankit_game_loop::{game_loop, Game, Playdate};
use pd::controls::buttons::PDButtonsExt;
use pd::controls::peripherals::{Buttons, Crank};
use pd::display::Display;
use pd::graphics::BitmapDrawMode;
use pd::sys::ffi::{LCD_COLUMNS, LCD_ROWS};
use pd::system::System;
use playdate::graphics::Graphics;

use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;

const ROWS: usize = LCD_ROWS as usize;
const COLUMNS: usize = 2 + (LCD_COLUMNS / 8) as usize;
const PIXEL_WIDTH: usize = LCD_COLUMNS as usize;
const BUFFER_SIZE: usize = COLUMNS * ROWS;

// Pre-computed lookup tables for ultra-fast bit operations
static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
static INV_BIT_MASKS: [u8; 8] = [0x7F, 0xBF, 0xDF, 0xEF, 0xF7, 0xFB, 0xFD, 0xFE];

// Platform structure with position, angle, and previous angle for rotation detection
#[derive(Clone, Copy)]
struct Platform {
    x: usize,
    y: usize,
    angle: u32,
    prev_angle: u32, // Track previous angle to detect rotation
}

impl Platform {
    fn new(x: usize, y: usize, angle: u32) -> Self {
        Platform {
            x,
            y,
            angle,
            prev_angle: angle,
        }
    }

    fn update_angle(&mut self, delta: i32) {
        self.prev_angle = self.angle;
        let angle = (self.angle as i32 + delta) % 360;
        self.angle = if angle < 0 {
            (angle + 360) as u32
        } else {
            angle as u32
        };
    }

    fn rotation_delta(&self) -> i32 {
        let mut delta = self.angle as i32 - self.prev_angle as i32;
        if delta > 180 {
            delta -= 360;
        }
        if delta < -180 {
            delta += 360;
        }
        delta
    }
}

// Sand velocity structure for tracking momentum
#[derive(Clone, Copy, Default)]
struct SandVelocity {
    vx: i8, // Horizontal velocity (-127 to 127)
    vy: i8, // Vertical velocity (-127 to 127)
}

fn clear_buffer(buffer: &mut [u8]) {
    for f in buffer.iter_mut().take(BUFFER_SIZE) {
        *f = 0;
    }
}

fn clear_velocity_buffer(buffer: &mut [SandVelocity]) {
    for v in buffer.iter_mut().take(BUFFER_SIZE) {
        *v = SandVelocity::default();
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
fn get_velocity(buffer: &[SandVelocity], x: usize, y: usize) -> SandVelocity {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return SandVelocity::default();
    }
    let index = y * PIXEL_WIDTH + x;
    if index >= buffer.len() {
        return SandVelocity::default();
    }
    buffer[index]
}

#[inline(always)]
fn set_velocity(buffer: &mut [SandVelocity], x: usize, y: usize, velocity: SandVelocity) {
    if x >= PIXEL_WIDTH || y >= ROWS {
        return;
    }
    let index = y * PIXEL_WIDTH + x;
    if index >= buffer.len() {
        return;
    }
    buffer[index] = velocity;
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
            if byte_index < frame.len() && (frame[byte_index] & BIT_MASKS[bit_index]) != 0 {
                set_pixel(sand_buffer, x, y, true);
            }
        }
    }
}

fn rand_range(min: usize, max: usize, rng: &mut SmallRng) -> usize {
    if max <= min {
        return min;
    }
    min + (rng.next_u32() as usize % (max - min))
}

// Fast approximation of sine using libm
fn fast_sin(angle_degrees: u32) -> f32 {
    let angle = (angle_degrees % 360) as f32;
    let radians = angle * PI / 180.0;
    libm::sinf(radians)
}

// Fast approximation of cosine using libm
fn fast_cos(angle_degrees: u32) -> f32 {
    let angle = (angle_degrees % 360) as f32;
    let radians = angle * PI / 180.0;
    libm::cosf(radians)
}

// Draw a single platform with its current angle using DDA-like algorithm
fn draw_platform(platform_buffer: &mut [u8], platform: &Platform) {
    let platform_length = 25;

    let cos_angle = fast_cos(platform.angle);
    let sin_angle = fast_sin(platform.angle);

    let start_x = platform.x as f32;
    let start_y = platform.y as f32;

    let mut x = start_x;
    let mut y = start_y;

    for _ in 0..platform_length {
        let pixel_x = libm::roundf(x) as i32;
        let pixel_y = libm::roundf(y) as i32;

        if pixel_x >= 0
            && pixel_y >= 0
            && (pixel_x as usize) < PIXEL_WIDTH
            && (pixel_y as usize) < ROWS
        {
            set_pixel(platform_buffer, pixel_x as usize, pixel_y as usize, true);

            if pixel_x + 1 >= 0 && ((pixel_x + 1) as usize) < PIXEL_WIDTH {
                set_pixel(
                    platform_buffer,
                    (pixel_x + 1) as usize,
                    pixel_y as usize,
                    true,
                );
            }
            if pixel_y + 1 >= 0 && ((pixel_y + 1) as usize) < ROWS {
                set_pixel(
                    platform_buffer,
                    pixel_x as usize,
                    (pixel_y + 1) as usize,
                    true,
                );
            }
            if pixel_x + 1 >= 0
                && pixel_y + 1 >= 0
                && ((pixel_x + 1) as usize) < PIXEL_WIDTH
                && ((pixel_y + 1) as usize) < ROWS
            {
                set_pixel(
                    platform_buffer,
                    (pixel_x + 1) as usize,
                    (pixel_y + 1) as usize,
                    true,
                );
            }
        }

        x += cos_angle;
        y += sin_angle;
    }
}

// NEW FEATURE: Apply push forces from rotating platforms to nearby sand
fn apply_platform_forces(
    sand_buffer: &mut [u8],
    velocity_buffer: &mut [SandVelocity],
    platforms: &[Platform],
) {
    for platform in platforms {
        let rotation_delta = platform.rotation_delta();

        // Only apply forces if platform is rotating
        if rotation_delta.abs() < 1 {
            continue;
        }

        let cos_angle = fast_cos(platform.angle);
        let sin_angle = fast_sin(platform.angle);

        let platform_length = 30; // Slightly longer detection range
        let force_radius = 8; // Radius around platform to apply forces

        // Calculate platform line points for collision detection
        let start_x = platform.x as f32;
        let start_y = platform.y as f32;

        let mut px = start_x;
        let mut py = start_y;

        for _ in 0..platform_length {
            let platform_x = libm::roundf(px) as i32;
            let platform_y = libm::roundf(py) as i32;

            if platform_x >= 0
                && platform_y >= 0
                && (platform_x as usize) < PIXEL_WIDTH
                && (platform_y as usize) < ROWS
            {
                // Check area around this platform pixel for sand
                for dy in -force_radius..force_radius {
                    for dx in -force_radius..force_radius {
                        let sand_x = platform_x + dx;
                        let sand_y = platform_y + dy;

                        if sand_x >= 0
                            && sand_y >= 0
                            && (sand_x as usize) < PIXEL_WIDTH
                            && (sand_y as usize) < ROWS
                            && is_sand(sand_buffer, sand_x as usize, sand_y as usize)
                        {
                            // Calculate distance from platform
                            let dist_sq = dx * dx + dy * dy;
                            if dist_sq <= force_radius * force_radius {
                                // Calculate perpendicular force direction based on rotation
                                let force_strength = (force_radius * force_radius - dist_sq) / 4;
                                let perpendicular_x = -sin_angle
                                    * rotation_delta as f32
                                    * force_strength as f32
                                    * 0.1;
                                let perpendicular_y =
                                    cos_angle * rotation_delta as f32 * force_strength as f32 * 0.1;

                                // Apply velocity to sand particle
                                let mut velocity =
                                    get_velocity(velocity_buffer, sand_x as usize, sand_y as usize);
                                velocity.vx =
                                    (f32::from(velocity.vx) + perpendicular_x).clamp(-20.0, 20.0) as i8;
                                velocity.vy =
                                    (f32::from(velocity.vy) + perpendicular_y).clamp(-20.0, 20.0) as i8;
                                set_velocity(
                                    velocity_buffer,
                                    sand_x as usize,
                                    sand_y as usize,
                                    velocity,
                                );
                            }
                        }
                    }
                }
            }

            px += cos_angle;
            py += sin_angle;
        }
    }
}

// Enhanced sand physics with velocity-based movement
fn update_sand_with_velocity(
    sand_buffer: &mut [u8],
    platform_buffer: &[u8],
    velocity_buffer: &mut [SandVelocity],
    changed_rows: &mut [bool; ROWS],
) {
    // Process sand with velocity first (from bottom to top)
    for y in (0..ROWS).rev() {
        for x in 0..PIXEL_WIDTH {
            if !is_sand(sand_buffer, x, y) {
                continue;
            }

            let mut velocity = get_velocity(velocity_buffer, x, y);
            let mut moved = false;

            // Apply horizontal velocity
            if velocity.vx.abs() > 2 {
                let target_x = if velocity.vx > 0 {
                    if x + 1 < PIXEL_WIDTH {
                        x + 1
                    } else {
                        x
                    }
                } else if x > 0 {
                    x - 1
                } else {
                    x
                };

                if target_x != x && !is_solid(sand_buffer, platform_buffer, target_x, y) {
                    // Move sand horizontally
                    set_pixel(sand_buffer, x, y, false);
                    set_pixel(sand_buffer, target_x, y, true);
                    set_velocity(velocity_buffer, x, y, SandVelocity::default());

                    // Reduce velocity with friction
                    velocity.vx = (f32::from(velocity.vx) * 0.7) as i8;
                    set_velocity(velocity_buffer, target_x, y, velocity);

                    changed_rows[y] = true;
                    moved = true;
                }
            }

            // Apply normal falling physics if not moved horizontally
            if !moved {
                let fell =
                    update_pixel_with_velocity(sand_buffer, platform_buffer, velocity_buffer, x, y);
                if fell {
                    changed_rows[y] = true;
                }
            }

            // Apply velocity decay
            if velocity.vx.abs() > 0 || velocity.vy.abs() > 0 {
                velocity.vx = (f32::from(velocity.vx) * 0.9) as i8;
                velocity.vy = (f32::from(velocity.vy) * 0.9) as i8;
                set_velocity(velocity_buffer, x, y, velocity);
            }
        }
    }
}

// Enhanced falling sand physics with velocity
#[inline]
fn update_pixel_with_velocity(
    sand_buffer: &mut [u8],
    platform_buffer: &[u8],
    velocity_buffer: &mut [SandVelocity],
    x: usize,
    y: usize,
) -> bool {
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

        // Transfer velocity
        let velocity = get_velocity(velocity_buffer, x, y);
        set_velocity(velocity_buffer, x, y, SandVelocity::default());
        set_velocity(velocity_buffer, x, y + 1, velocity);

        return true;
    }

    // Try to move down-left
    if x > 0 && !is_solid(sand_buffer, platform_buffer, x - 1, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x - 1, y + 1, true);

        // Transfer velocity with slight leftward bias
        let mut velocity = get_velocity(velocity_buffer, x, y);
        velocity.vx = (velocity.vx - 1).max(-10);
        set_velocity(velocity_buffer, x, y, SandVelocity::default());
        set_velocity(velocity_buffer, x - 1, y + 1, velocity);

        return true;
    }

    // Try to move down-right
    if x < PIXEL_WIDTH - 1 && !is_solid(sand_buffer, platform_buffer, x + 1, y + 1) {
        set_pixel(sand_buffer, x, y, false);
        set_pixel(sand_buffer, x + 1, y + 1, true);

        // Transfer velocity with slight rightward bias
        let mut velocity = get_velocity(velocity_buffer, x, y);
        velocity.vx = (velocity.vx + 1).min(10);
        set_velocity(velocity_buffer, x, y, SandVelocity::default());
        set_velocity(velocity_buffer, x + 1, y + 1, velocity);

        return true;
    }

    false
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

    // Draw all rotating diagonal platforms with precise angles
    for platform in platforms {
        draw_platform(platform_buffer, platform);
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
        .draw_text("Crank: Rotate platforms", 75, 180)
        .unwrap();
    graphics.draw_text("Platforms push sand!", 95, 200).unwrap();
}

// Create initial platforms with random angles
fn create_initial_platforms(rng: &mut SmallRng) -> Vec<Platform> {
    let mut platforms = Vec::new();

    // Generate 8 random diagonal platforms with random angles (0-360 degrees)
    for _ in 0..8 {
        let x = rand_range(50, PIXEL_WIDTH - 50, rng);
        let y = rand_range(30, ROWS - 50, rng);
        let angle = rng.next_u32() % 360;

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

    // Endless sand rain from the top!
    if game.started {
        let rain_rate = 6; // Reduced rate for better platform interaction
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

    // Crank rotation affects ALL platforms - SMOOTH ROTATION
    let crank_change = crank.change();
    if crank_change.abs() > 0.3 {
        let angle_delta = (crank_change / 2.0) as i32;

        // Apply rotation to ALL platforms
        for platform in &mut game.platforms {
            platform.update_angle(angle_delta);
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

    // B button clears sand and velocities
    if buttons.current.b() {
        clear_buffer(&mut *game.sand_buffer);
        clear_velocity_buffer(&mut *game.velocity_buffer);

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

    if game.frame_counter.is_multiple_of(16) {
        game.screen_density = calculate_screen_density(&*game.sand_buffer);
    }

    // NEW: Apply platform forces to push sand
    apply_platform_forces(
        &mut *game.sand_buffer,
        &mut *game.velocity_buffer,
        &game.platforms,
    );

    let mut changed_rows = [false; ROWS];

    let steps = match game.screen_density {
        0..=25 => 2, // Reduced steps for velocity calculations
        26..=50 => 2,
        51..=75 => 1,
        _ => 1,
    };

    for _ in 0..steps {
        // NEW: Use velocity-based sand physics
        update_sand_with_velocity(
            &mut *game.sand_buffer,
            &*game.platform_buffer,
            &mut *game.velocity_buffer,
            &mut changed_rows,
        );
    }

    copy_to_frame(&*game.sand_buffer, &*game.platform_buffer, frame);
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
    platform_buffer: Box<[u8; BUFFER_SIZE]>,
    velocity_buffer: Box<[SandVelocity; PIXEL_WIDTH * ROWS]>, // NEW: Velocity tracking
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
        let velocity_buffer = Box::new([SandVelocity::default(); PIXEL_WIDTH * ROWS]);

        let time = System::Cached().seconds_since_epoch();
        let mut rng = SmallRng::seed_from_u64(u64::from(time));

        let platforms = create_initial_platforms(&mut rng);
        redraw_platforms(&mut *platform_buffer, &platforms);

        copy_to_frame(&*sand_buffer, &*platform_buffer, frame);
        draw_intro();

        // Display::Cached().set_scale(scale);

        Self {
            rng,
            started: false,
            position_x: PIXEL_WIDTH / 2,
            position_y: ROWS / 4,
            frame_counter: 0,
            screen_density: 0,
            sand_buffer,
            platform_buffer,
            velocity_buffer,
            platforms,
        }
    }

    fn update(&mut self, _playdate: &Playdate) {
        // Always redraw platforms EVERY frame before processing input
        redraw_platforms(&mut *self.platform_buffer, &self.platforms);

        // Process all input
        process_input(self);

        // Draw UI elements
        System::Cached().draw_fps(0, 0);
    }
}

game_loop!(FallingSand);
