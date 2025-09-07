use alloc::vec::Vec;
use rand::{rngs::SmallRng, RngCore};

use crate::{
    clear_buffer,
    consts::{PIXEL_WIDTH, ROWS},
    get_pixel, set_pixel,
    utils::{fast_cos, fast_sin, rand_range},
};

// Platform structure with position, angle, and previous angle for rotation detection
#[derive(Clone, Copy)]
pub struct Platform {
    pub x: usize,
    pub y: usize,
    pub angle: u32,
    pub prev_angle: u32, // Track previous angle to detect rotation
}

impl Platform {
    pub fn new(x: usize, y: usize, angle: u32) -> Self {
        Platform {
            x,
            y,
            angle,
            prev_angle: angle,
        }
    }

    pub fn update_angle(&mut self, delta: i32) {
        self.prev_angle = self.angle;
        let angle = (self.angle as i32 + delta) % 360;
        self.angle = if angle < 0 {
            (angle + 360) as u32
        } else {
            angle as u32
        };
    }

    pub fn rotation_delta(&self) -> i32 {
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

#[inline(always)]
pub fn is_platform(platform_buffer: &[u8], x: usize, y: usize) -> bool {
    get_pixel(platform_buffer, x, y)
}

// Create initial platforms with random angles
pub fn create_initial_platforms(rng: &mut SmallRng) -> Vec<Platform> {
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

// Draw a single platform with its current angle using DDA-like algorithm
pub fn draw_platform(platform_buffer: &mut [u8], platform: &Platform) {
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

// Redraw all platforms to the platform buffer - ALWAYS CALLED EVERY FRAME
pub fn redraw_platforms(platform_buffer: &mut [u8], platforms: &[Platform]) {
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
