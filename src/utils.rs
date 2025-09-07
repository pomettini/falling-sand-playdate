use core::f32::consts::PI;

use rand::{rngs::SmallRng, RngCore};

// Fast approximation of sine using libm
pub fn fast_sin(angle_degrees: u32) -> f32 {
    let angle = (angle_degrees % 360) as f32;
    let radians = angle * PI / 180.0;
    libm::sinf(radians)
}

// Fast approximation of cosine using libm
pub fn fast_cos(angle_degrees: u32) -> f32 {
    let angle = (angle_degrees % 360) as f32;
    let radians = angle * PI / 180.0;
    libm::cosf(radians)
}

pub fn rand_range(min: usize, max: usize, rng: &mut SmallRng) -> usize {
    if max <= min {
        return min;
    }
    min + (rng.next_u32() as usize % (max - min))
}
