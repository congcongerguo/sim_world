use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Deterministic random number generator.
/// All random decisions in the simulation are seeded upfront
/// so that the entire timeline is deterministic and replayable.
#[derive(Resource)]
pub struct SimRng(pub StdRng);

impl Default for SimRng {
    fn default() -> Self {
        Self(StdRng::seed_from_u64(42))
    }
}

impl SimRng {
    pub fn new(seed: u64) -> Self {
        Self(StdRng::seed_from_u64(seed))
    }

    pub fn gen_f64(&mut self) -> f64 {
        self.0.random()
    }

    pub fn gen_range(&mut self, min: usize, max: usize) -> usize {
        self.0.random_range(min..max)
    }

    pub fn gen_bool(&mut self, p: f64) -> bool {
        self.0.random_bool(p)
    }
}

pub struct SimRngPlugin;

impl Plugin for SimRngPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimRng>();
    }
}
