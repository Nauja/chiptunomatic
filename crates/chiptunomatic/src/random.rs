use core::fmt::Debug;

use dyn_clone::DynClone;

/// Hide the generic Rng object
pub trait Random: Debug + DynClone + Send {
    fn set_seed(&mut self, seed: u64);
    fn next_float(&mut self) -> f32;
}

dyn_clone::clone_trait_object!(Random);

/// Always return 1.0 so the randomness is disabled
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoRandom {}

impl Random for NoRandom {
    fn set_seed(&mut self, _seed: u64) {}

    fn next_float(&mut self) -> f32 {
        1.0
    }
}

#[cfg(feature = "std")]
mod std {
    use rand::{rngs::StdRng, Rng, SeedableRng};

    use crate::random::Random;

    /// Random implementation using StdRng
    #[derive(Debug, Clone)]
    pub struct StdRandom {
        rng: StdRng,
    }

    impl Random for StdRandom {
        fn set_seed(&mut self, seed: u64) {
            self.rng = StdRng::seed_from_u64(seed);
        }

        fn next_float(&mut self) -> f32 {
            self.rng.gen::<f32>()
        }
    }

    impl StdRandom {
        pub fn new() -> Self {
            Self {
                rng: StdRng::from_entropy(),
            }
        }

        pub fn with_seed(self, seed: u64) -> Self {
            Self {
                rng: StdRng::seed_from_u64(seed),
            }
        }
    }
}

#[cfg(feature = "std")]
pub use std::*;
