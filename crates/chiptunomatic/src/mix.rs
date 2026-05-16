use alloc::vec::Vec;

use crate::{DrumSample, SongSample};

/// Combine the song and drum samples
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub song: SongSample,
    pub drum: DrumSample,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Mix {
    pub sample: Sample,
    pub frequency: f32,
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

/// Same per-sample peak-normalization behaviour as [`IterMix`], for feeding [`Sample`] values one-by-one from JS/workers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixGenerator {
    peak: f32,
    pub master_output: StemOutput,
    pub voice_output: StemOutput,
    pub square_output: StemOutput,
    pub triangle_output: StemOutput,
    pub noise_output: StemOutput,
}

impl Default for MixGenerator {
    fn default() -> Self {
        Self {
            peak: 0.0,
            master_output: StemOutput::default(),
            voice_output: StemOutput::default(),
            square_output: StemOutput::default(),
            triangle_output: StemOutput::default(),
            noise_output: StemOutput::default(),
        }
    }
}

/// Mix the song and drum samples together
impl MixGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the master volume (applied after peak normalisation).
    pub fn with_master_output(self, output: StemOutput) -> Self {
        Self {
            master_output: output,
            ..self
        }
    }

    /// Set per-stem output for the voice stem.
    pub fn with_voice_output(self, output: StemOutput) -> Self {
        Self {
            voice_output: output,
            ..self
        }
    }

    /// Set per-stem output for the square (melody) stem.
    pub fn with_square_output(self, output: StemOutput) -> Self {
        Self {
            square_output: output,
            ..self
        }
    }

    /// Set per-stem output for the triangle (bass) stem.
    pub fn with_triangle_output(self, output: StemOutput) -> Self {
        Self {
            triangle_output: output,
            ..self
        }
    }

    /// Set per-stem output for the noise (drum) stem.
    pub fn with_noise_output(self, output: StemOutput) -> Self {
        Self {
            noise_output: output,
            ..self
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.peak = 0.0;
    }

    fn any_solo(&self) -> bool {
        self.voice_output.solo
            || self.square_output.solo
            || self.triangle_output.solo
            || self.noise_output.solo
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

    // Mix one sample
    pub fn mix_sample(&mut self, mut sample: Sample) -> Mix {
        sample.song.square.value *= self.effective_square_volume();
        sample.song.triangle.value *= self.effective_triangle_volume();
        sample.song.voice.value *= self.effective_voice_volume();
        sample.drum *= self.effective_noise_volume();

        let mut mixed = sample.song.voice.value
            + sample.song.square.value
            + sample.song.triangle.value
            + sample.drum;

        self.peak = self.peak.max(mixed.abs());
        if self.peak > 0.0 {
            mixed /= self.peak;
        }
        mixed *= 0.9 * self.effective_master_volume();
        Mix {
            sample,
            frequency: mixed,
        }
    }

    // Mix multiple samples
    pub fn mix_samples(&mut self, samples: &[Sample], num_samples: usize) -> Vec<Mix> {
        (0..num_samples.min(samples.len()))
            .map(|i| self.mix_sample(samples[i]))
            .collect()
    }
}

#[cfg(feature = "std")]
mod iter {
    use crate::{DrumSample, Mix, MixGenerator, Sample, SongSample};

    /// Mix the incoming song and drum samples
    pub struct MixSamples<
        S: IntoIterator<Item = std::io::Result<SongSample>>,
        D: IntoIterator<Item = DrumSample>,
    > {
        song_samples: S::IntoIter,
        drum_samples: D::IntoIter,
        mix_generator: MixGenerator,
    }

    impl<
            S: IntoIterator<Item = std::io::Result<SongSample>>,
            D: IntoIterator<Item = DrumSample>,
        > MixSamples<S, D>
    {
        pub fn new(song_samples: S, drum_samples: D) -> Self {
            Self {
                song_samples: song_samples.into_iter(),
                drum_samples: drum_samples.into_iter(),
                mix_generator: Default::default(),
            }
        }

        pub fn with_mix_generator(self, mix_generator: MixGenerator) -> Self {
            Self {
                mix_generator,
                ..self
            }
        }

        pub fn fill_buf(&mut self, buffer: &mut [Mix]) -> std::io::Result<usize> {
            let mut mixes_written = 0;
            for i in 0..buffer.len() {
                match self.next() {
                    None => break,
                    Some(Err(e)) => return Err(e),
                    Some(Ok(m)) => {
                        buffer[i] = m;
                        mixes_written += 1;
                    }
                }
            }

            Ok(mixes_written)
        }
    }

    impl<
            S: IntoIterator<Item = std::io::Result<SongSample>>,
            D: IntoIterator<Item = DrumSample>,
        > Iterator for MixSamples<S, D>
    {
        type Item = std::io::Result<Mix>;

        fn next(&mut self) -> Option<Self::Item> {
            match self.song_samples.next() {
                None => return None,
                Some(Err(e)) => return Some(Err(e)),
                Some(Ok(song)) => match self.drum_samples.next() {
                    None => return None,
                    Some(drum) => Some(Ok(self.mix_generator.mix_sample(Sample { song, drum }))),
                },
            }
        }
    }
}

#[cfg(feature = "std")]
pub use iter::*;
