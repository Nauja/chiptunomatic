use alloc::vec::Vec;
use getset::{Getters, MutGetters, Setters, WithSetters};

use crate::{plugin::StemMask, StemsSample};

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub stems: StemsSample,
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MasterOutput {
    pub volume: f32,
    pub muted: bool,
}

impl Default for MasterOutput {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StemOutput {
    pub volume: f32,
    pub muted: bool,
    pub solo: bool,
}

impl Default for StemOutput {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
            solo: false,
        }
    }
}

/// Configure the volumes
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct MixerConfig {
    pub master_output: MasterOutput,
    pub voice_output: StemOutput,
    pub square_output: StemOutput,
    pub triangle_output: StemOutput,
    pub noise_output: StemOutput,
    pub sfx_output: StemOutput,
}

impl MixerConfig {
    /// Return if any solo is true for any stem
    pub fn any_solo(&self) -> bool {
        self.voice_output.solo
            || self.square_output.solo
            || self.triangle_output.solo
            || self.noise_output.solo
            || self.sfx_output.solo
    }

    pub fn effective_master_volume(&self) -> f32 {
        if self.master_output.muted {
            0.0
        } else {
            self.master_output.volume
        }
    }

    pub fn effective_voice_volume(&self) -> f32 {
        if self.voice_output.muted || (self.any_solo() && !self.voice_output.solo) {
            0.0
        } else {
            self.voice_output.volume
        }
    }

    pub fn effective_square_volume(&self) -> f32 {
        if self.square_output.muted || (self.any_solo() && !self.square_output.solo) {
            0.0
        } else {
            self.square_output.volume
        }
    }

    pub fn effective_triangle_volume(&self) -> f32 {
        if self.triangle_output.muted || (self.any_solo() && !self.triangle_output.solo) {
            0.0
        } else {
            self.triangle_output.volume
        }
    }

    pub fn effective_noise_volume(&self) -> f32 {
        if self.noise_output.muted || (self.any_solo() && !self.noise_output.solo) {
            0.0
        } else {
            self.noise_output.volume
        }
    }

    pub fn effective_sfx_volume(&self) -> f32 {
        if self.sfx_output.muted || (self.any_solo() && !self.sfx_output.solo) {
            0.0
        } else {
            self.sfx_output.volume
        }
    }
}

/// Same per-sample peak-normalization behaviour as [`IterMix`], for feeding [`Sample`] values one-by-one from JS/workers.
#[derive(Debug, Clone, Copy, PartialEq, Getters, MutGetters, Setters, WithSetters)]
pub struct Mixer {
    #[getset(get = "pub", get_mut = "pub", set = "pub", set_with = "pub")]
    config: MixerConfig,
    /// Volume peak reached
    #[getset(get = "pub")]
    peak: f32,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            config: Default::default(),
            peak: 0.0,
        }
    }
}

/// Mix the song and drum samples together
impl Mixer {
    /// Reset the peak volume
    pub fn reset(&mut self) {
        self.peak = 0.0;
    }

    // Mix one sample
    pub fn mix_sample(&mut self, mut sample: StemsSample, mask: &StemMask) -> Sample {
        // Mix as if no mask to compute the correct peak
        sample.square.value *= self.config.effective_square_volume();
        sample.triangle.value *= self.config.effective_triangle_volume();
        sample.voice.value *= self.config.effective_voice_volume();
        sample.noise *= self.config.effective_noise_volume();
        sample.sfx.value *= self.config.effective_sfx_volume();

        let mixed = sample.voice.value
            + sample.square.value
            + sample.triangle.value
            + sample.noise
            + sample.sfx.value;

        self.peak = self.peak.max(mixed.abs());

        // Mute the stems masked out
        if !mask.voice {
            sample.voice.value = 0.0;
        }

        if !mask.square {
            sample.square.value = 0.0
        }

        if !mask.triangle {
            sample.triangle.value = 0.0
        }

        if !mask.noise {
            sample.noise = 0.0
        }

        if !mask.sfx {
            sample.sfx.value = 0.0;
        }

        // Mix again
        let mut mixed = sample.voice.value
            + sample.square.value
            + sample.triangle.value
            + sample.noise
            + sample.sfx.value;

        // Apply the peak
        if self.peak > 0.0 {
            mixed /= self.peak;
        }
        mixed *= 0.9 * self.config.effective_master_volume();

        Sample {
            stems: sample,
            value: mixed,
        }
    }

    // Mix multiple samples
    pub fn mix_samples(&mut self, samples: &[StemsSample], mask: &StemMask) -> Vec<Sample> {
        samples.iter().map(|s| self.mix_sample(*s, mask)).collect()
    }
}
