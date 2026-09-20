//! A seeded generator of our own: the whole point of the simulator is that a seed replays exactly, which a
//! system generator cannot promise across machines.

/// xorshift64*, good enough for aim scatter and target ties.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        // Any seed including 0 must give a usable state.
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// The horizontal part of a vector drawn uniformly from a ball of the given radius — the engine's
    /// `NextVector`, which is what every weapon's aim error is scaled from.
    pub fn in_ball(&mut self, radius: f32) -> (f32, f32) {
        if radius <= 0.0 {
            return (0.0, 0.0);
        }
        loop {
            let (x, y, z) = (self.unit() * 2.0 - 1.0, self.unit() * 2.0 - 1.0, self.unit() * 2.0 - 1.0);
            if x * x + y * y + z * z <= 1.0 {
                return (x * radius, z * radius);
            }
        }
    }
}
