use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::constants::SAMPLE_RATE;
use crate::plugin::Plugin;
use crate::synth::{envelope, midi_to_hz, square, triangle};
use crate::{BassNote, MelodyNote, SongNote};

/// Sample of an individual stem
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct StemSample {
    pub value: f32,
    pub byte_index: u64,
}

/// Single sample of the square, triangle, and voice stems
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct SongSample {
    pub square: StemSample,
    pub triangle: StemSample,
    pub voice: StemSample,
}

pub trait SampleStem {
    /// Sample the square stem
    fn sample_square(&self, sample_rate: u32) -> Vec<StemSample>;
    /// Sample the triangle stem
    fn sample_triangle(&self, sample_rate: u32) -> Vec<StemSample>;
}

pub trait Note {
    fn midi(&self) -> f64;
    fn duration(&self) -> f64;
    fn byte_index(&self) -> u64;
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct SquareNote {
    pub midi: f64,
    pub duration: f64,
    pub amp: f32,
    pub duty: f64,
    pub byte_index: u64,
}

impl Note for SquareNote {
    fn midi(&self) -> f64 {
        self.midi
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn byte_index(&self) -> u64 {
        self.byte_index
    }
}

impl SampleStem for SquareNote {
    fn sample_square(&self, sample_rate: u32) -> Vec<StemSample> {
        let sample_rate = sample_rate as f64;
        let total_samples = (sample_rate * self.duration).floor() as usize;
        if total_samples == 0 {
            return Vec::new();
        }

        let dur_sec = total_samples as f64 / sample_rate;
        let chunk = square(
            sample_rate,
            midi_to_hz(self.midi),
            dur_sec,
            self.amp,
            self.duty,
        );
        let chunk = envelope(sample_rate, &chunk, 0.005, 0.05, 0.7, 0.05);
        let mut samples = Vec::new();
        for (i, &s) in chunk.iter().enumerate() {
            if i >= total_samples {
                break;
            }

            samples.push(StemSample {
                value: s,
                byte_index: self.byte_index,
            });
        }

        samples
    }

    fn sample_triangle(&self, _sample_rate: u32) -> Vec<StemSample> {
        Default::default()
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct TriangleNote {
    pub midi: f64,
    pub duration: f64,
    pub byte_index: u64,
}

impl Note for TriangleNote {
    fn midi(&self) -> f64 {
        self.midi
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn byte_index(&self) -> u64 {
        self.byte_index
    }
}

impl SampleStem for TriangleNote {
    fn sample_square(&self, _sample_rate: u32) -> Vec<StemSample> {
        Default::default()
    }

    fn sample_triangle(&self, sample_rate: u32) -> Vec<StemSample> {
        let sample_rate = sample_rate as f64;
        let total_beats = (sample_rate as f64 * self.duration).floor() as usize;
        if total_beats == 0 {
            return Vec::new();
        }

        let dur_sec = total_beats as f64 / sample_rate;
        let chunk = triangle(sample_rate, midi_to_hz(self.midi), dur_sec, 0.25);
        let chunk = envelope(sample_rate, &chunk, 0.005, 0.05, 0.7, 0.05);
        let mut samples = Vec::new();
        for (i, &s) in chunk.iter().enumerate() {
            if i >= total_beats {
                break;
            }

            samples.push(StemSample {
                value: s,
                byte_index: self.byte_index,
            });
        }

        samples
    }
}

#[derive(Debug, Clone)]
pub struct SongSampleGenerator {
    plugin: Rc<dyn Plugin>,
    sample_rate: u32,
    melody_notes: VecDeque<MelodyNote>,
    bass_notes: VecDeque<BassNote>,
    square_samples: VecDeque<StemSample>,
    triangle_samples: VecDeque<StemSample>,
    voice_samples: VecDeque<StemSample>,
}

impl SongSampleGenerator {
    pub fn new(plugin: Rc<dyn Plugin>) -> Self {
        Self {
            plugin,
            sample_rate: SAMPLE_RATE,
            melody_notes: Default::default(),
            bass_notes: Default::default(),
            square_samples: Default::default(),
            triangle_samples: Default::default(),
            voice_samples: Default::default(),
        }
    }

    pub fn with_sample_rate(self, sample_rate: u32) -> Self {
        Self {
            sample_rate,
            ..self
        }
    }

    // Add a single note to the buffer
    pub fn push(&mut self, note: SongNote) {
        match note {
            SongNote::Melody(n) => self.melody_notes.push_back(n),
            SongNote::Bass(n) => self.bass_notes.push_back(n),
        }
    }

    // Add notes to the buffer
    pub fn extend(&mut self, notes: &[SongNote]) {
        for note in notes {
            self.push(*note);
        }
    }

    // Number of available samples
    pub fn len(&self) -> usize {
        self.square_samples.len().min(self.triangle_samples.len())
    }

    // If there are no available samples
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Sample the notes currently in the buffer
    pub fn sample(&mut self) -> Vec<SongSample> {
        let mut samples = Vec::new();

        loop {
            // Drain the current available samples
            let available_samples = self.square_samples.len().min(self.triangle_samples.len());
            for _ in 0..available_samples {
                let Some(square) = self.square_samples.pop_front() else {
                    // Can't happen
                    break;
                };

                let Some(triangle) = self.triangle_samples.pop_front() else {
                    // Can't happen
                    break;
                };

                let voice = self.voice_samples.pop_front().unwrap_or_default();
                samples.push(SongSample { square, triangle, voice });
            }

            // Sample melody notes if there are too much bass samples
            while self.triangle_samples.len() > self.square_samples.len() {
                let Some(note) = self.melody_notes.pop_front() else {
                    // Not enough melody notes to continue
                    return samples;
                };

                self.square_samples
                    .extend(self.plugin.sample_melody_note(note, self.sample_rate));
                self.voice_samples
                    .extend(self.plugin.sample_voice_note(note, self.sample_rate));
            }

            // Sample the next bass note if any
            if let Some(note) = self.bass_notes.pop_front() {
                self.triangle_samples
                    .extend(self.plugin.sample_bass_note(note, self.sample_rate));
            }

            // Stop here if we can't generate enough bass samples
            if self.triangle_samples.is_empty() {
                return samples;
            }
        }
    }
}

#[cfg(feature = "std")]
mod iter {
    use alloc::rc::Rc;
    use std::collections::VecDeque;

    use crate::{
        plugin::Plugin, DrumSample, MixSamples, SongNote, SongSample, SongSampleGenerator,
    };

    /// Iterator that samples the song notes
    pub struct SampleSongNotes<I: IntoIterator<Item = std::io::Result<SongNote>>> {
        inner: I::IntoIter,
        sample_generator: SongSampleGenerator,
        samples: VecDeque<SongSample>,
    }

    impl<I: IntoIterator<Item = std::io::Result<SongNote>>> SampleSongNotes<I> {
        pub fn new(inner: I, plugin: Rc<dyn Plugin>) -> Self {
            Self::from_generator(inner, SongSampleGenerator::new(plugin))
        }

        pub fn from_generator(inner: I, sample_generator: SongSampleGenerator) -> Self {
            Self {
                inner: inner.into_iter(),
                sample_generator,
                samples: Default::default(),
            }
        }

        pub fn with_sample_rate(self, sample_rate: u32) -> Self {
            Self {
                sample_generator: self.sample_generator.with_sample_rate(sample_rate),
                ..self
            }
        }

        // Mix the song samples with drum samples
        pub fn mix<D: IntoIterator<Item = DrumSample>>(
            self,
            drum_samples: D,
        ) -> MixSamples<Self, D> {
            MixSamples::new(self, drum_samples)
        }
    }

    impl<I: IntoIterator<Item = std::io::Result<SongNote>>> Iterator for SampleSongNotes<I> {
        type Item = std::io::Result<SongSample>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                // Drain the generated samples first
                if let Some(sample) = self.samples.pop_front() {
                    return Some(Ok(sample));
                }

                // Try to generate samples for the next note
                match self.inner.next() {
                    // Not enough notes to generate samples
                    None => return None,
                    Some(Err(e)) => return Some(Err(e)),
                    Some(Ok(n)) => {
                        // Push the note to the generator
                        self.sample_generator.push(n);
                        // Generate samples
                        self.samples.extend(self.sample_generator.sample());
                    }
                };
            }
        }
    }
}

#[cfg(feature = "std")]
pub use iter::*;
