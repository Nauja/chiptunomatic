use std::collections::VecDeque;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::{
    constants::{HAT_PATTERNS, KICK_PATTERNS, SNARE_PATTERNS},
    plugin::{Plugin, SampleStepConfig},
    random::Random,
    SongMetadata,
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrumStep {
    pub kick: bool,
    pub snare: bool,
    pub hat: bool,
    pub open_hat: bool,
    pub offset: usize,
}

/// 16-step drum pattern
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrumPattern {
    pub steps: [DrumStep; 16],
}

impl DrumPattern {
    /// Return a random drum pattern from a 8 bytes seed
    pub fn from_seed(seed: &[u8; 8]) -> DrumPattern {
        let mut kick = KICK_PATTERNS[(seed[0] % 3) as usize];
        let snare = SNARE_PATTERNS[(seed[1] % 3) as usize];
        let hat = HAT_PATTERNS[(seed[2] % 3) as usize];

        if seed[3] & 0x03 == 0 {
            let slots = [6usize, 11, 14];
            kick[slots[(seed[4] % 3) as usize]] = 1;
        }

        let mut steps = [DrumStep::default(); 16];
        for offset in 0..16 {
            steps[offset] = DrumStep {
                kick: kick[offset] == 1,
                snare: snare[offset] == 1,
                hat: hat[offset] == 1,
                open_hat: false,
                offset,
            };
        }

        if seed[5] & 0x01 != 0 {
            steps[8].open_hat = true;
        }

        DrumPattern { steps }
    }

    /// Iterate the steps infinitely
    pub fn steps(self) -> Steps {
        Steps::new(self)
    }

    // Get the n-th step
    pub fn step(&self, n: u64) -> DrumStep {
        self.steps[(n % 16) as usize].clone()
    }
}

/// Yield the steps of a drum pattern infinitely
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Steps {
    pattern: DrumPattern,
    step: u64,
}

impl Steps {
    pub fn new(pattern: DrumPattern) -> Self {
        Self { pattern, step: 0 }
    }
}

impl Iterator for Steps {
    type Item = DrumStep;

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.step;
        self.step = (self.step + 1) % 16;
        Some(self.pattern.step(i))
    }
}

/// Generate infinite samples from a drum pattern
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrumSampler {
    steps: Steps,
}

impl DrumSampler {
    /// Get the drum pattern
    pub fn drum_pattern(&self) -> &DrumPattern {
        &self.steps.pattern
    }

    /// Set the drum pattern
    pub fn set_drum_pattern(&mut self, pattern: DrumPattern) {
        self.steps.pattern = pattern;
    }

    /// Get the current drum step
    pub fn drum_step(&self) -> u64 {
        self.steps.step
    }

    /// Set the current drum step
    pub fn set_drum_step(&mut self, step: u64) {
        self.steps.step = step;
    }

    /// Reset to the first drum step
    pub fn reset(&mut self) {
        self.steps.step = 0;
    }

    /// Sample the current drum step
    pub fn sample(
        &mut self,
        metadata: &SongMetadata,
        plugin: &Box<dyn Plugin>,
        sample_rate: u32,
        random: &mut Box<dyn Random>,
    ) -> Vec<f32> {
        let Some(step) = self.steps.next() else {
            return Default::default();
        };

        let sample_rate = sample_rate as f64;
        let pattern = (step.offset % 16) as usize;
        let step_duration = metadata.timing.sixteenth;
        let color = metadata.seed[pattern % 8];

        // Create enough empty samples in case there is no drum at this step
        let total_samples = (sample_rate * metadata.timing.sixteenth).floor() as usize;
        let mut samples = Vec::with_capacity(total_samples);
        samples.resize_with(total_samples, Default::default);

        plugin.sample_step(
            &step,
            SampleStepConfig {
                sample_rate,
                step_duration,
                pattern,
                color,
                random,
            },
            &mut samples,
        );

        samples
    }
}

/// Allow to iterate the drum samples one by one

#[derive(Default, Debug, Clone)]
pub struct SampleDrum {
    inner: DrumSampler,
    samples: VecDeque<f32>,
}

impl SampleDrum {
    pub fn set_drum_pattern(&mut self, pattern: DrumPattern) {
        self.inner.set_drum_pattern(pattern);
    }

    pub fn with_drum_pattern(mut self, pattern: DrumPattern) -> Self {
        self.inner.set_drum_pattern(pattern);
        self
    }

    pub fn next(
        &mut self,
        metadata: &SongMetadata,
        plugin: &Box<dyn Plugin>,
        sample_rate: u32,
        random: &mut Box<dyn Random>,
    ) -> f32 {
        loop {
            // Drain the available samples first
            if let Some(sample) = self.samples.pop_front() {
                return sample;
            }

            self.samples
                .extend(self.inner.sample(metadata, plugin, sample_rate, random));
        }
    }
}
