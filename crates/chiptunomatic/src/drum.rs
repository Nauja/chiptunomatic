use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::{
    constants::{HAT_PATTERNS, KICK_PATTERNS, SAMPLE_RATE, SNARE_PATTERNS},
    plugin::{Plugin, Random, SampleStepConfig},
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

/// 16-step drum grid and seed bytes
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

pub type DrumSample = f32;

/// Generate drum samples from the steps
pub struct DrumSampleGenerator {
    metadata: SongMetadata,
    plugin: Rc<dyn Plugin>,
    sample_rate: f64,
    random: Rc<dyn Random>,
}

impl DrumSampleGenerator {
    pub fn new(metadata: SongMetadata, plugin: Rc<dyn Plugin>, random: Rc<dyn Random>) -> Self {
        Self {
            metadata,
            plugin,
            sample_rate: SAMPLE_RATE as f64,
            random,
        }
    }

    pub fn with_sample_rate(self, sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate as f64,
            ..self
        }
    }

    // Iterate the samples infinitely
    pub fn samples(self) -> SampleDrumSteps {
        SampleDrumSteps {
            drum_pattern: self.metadata.drum_pattern.clone(),
            sample_generator: self,
            step: 0,
            samples: Default::default(),
        }
    }

    /// Sample a step
    pub fn sample_step(&mut self, step: DrumStep) -> Vec<DrumSample> {
        let pattern = (step.offset % 16) as usize;
        let step_duration = self.metadata.timing.sixteenth;
        let color = self.metadata.seed[pattern % 8];

        // Create enough empty samples in case there is no drum at this step
        let total_samples = (self.sample_rate * self.metadata.timing.sixteenth).floor() as usize;
        let mut samples = Vec::with_capacity(total_samples);
        samples.resize_with(total_samples, Default::default);

        self.plugin.sample_step(
            step,
            SampleStepConfig {
                sample_rate: self.sample_rate,
                step_duration,
                pattern,
                color,
                random: &self.random,
            },
            &mut samples,
        );

        samples
    }
}

/// Iterator the sample the drum steps
pub struct SampleDrumSteps {
    sample_generator: DrumSampleGenerator,
    drum_pattern: DrumPattern,
    // Current drum step
    step: u64,
    // Samples of current drum step
    samples: VecDeque<DrumSample>,
}

impl SampleDrumSteps {
    pub fn new(metadata: SongMetadata, plugin: Rc<dyn Plugin>, random: Rc<dyn Random>) -> Self {
        Self::from_generator(
            metadata.clone(),
            DrumSampleGenerator::new(metadata, plugin, random),
        )
    }

    pub fn from_generator(metadata: SongMetadata, sample_generator: DrumSampleGenerator) -> Self {
        Self {
            drum_pattern: metadata.drum_pattern.clone(),
            sample_generator,
            step: 0,
            samples: Default::default(),
        }
    }

    pub fn with_sample_rate(self, sample_rate: u32) -> Self {
        Self {
            sample_generator: self.sample_generator.with_sample_rate(sample_rate),
            ..self
        }
    }
}

impl Iterator for SampleDrumSteps {
    type Item = DrumSample;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Drain the already generated samples
            if let Some(sample) = self.samples.pop_front() {
                return Some(sample);
            }

            // Generate the samples for the next step
            self.samples.extend(
                self.sample_generator
                    .sample_step(self.drum_pattern.step(self.step)),
            );
            self.step += 1;
        }
    }
}
