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

/// Same per-sample peak-normalization behaviour as [`IterMix`], for feeding [`Sample`] values one-by-one from JS/workers.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct MixGenerator {
    peak: f32,
}

/// Mix the song and drum samples together
impl MixGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn reset(&mut self) {
        self.peak = 0.0;
    }

    // Mix one sample
    pub fn mix_sample(&mut self, sample: Sample) -> Mix {
        let mut mixed = sample.song.square.value + sample.song.triangle.value + sample.drum;
        self.peak = self.peak.max(mixed.abs());
        if self.peak > 0.0 {
            mixed /= self.peak;
        }
        mixed *= 0.9;
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
