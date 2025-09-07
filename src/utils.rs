use rand::{rngs::SmallRng, RngCore};

pub fn rand_range(min: usize, max: usize, rng: &mut SmallRng) -> usize {
    if max <= min {
        return min;
    }
    min + (rng.next_u32() as usize % (max - min))
}
