use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::consumer::Consume;
use crate::plugin::Plugin;
use crate::synth::{envelope, midi_to_hz, square, triangle};
use crate::{BassNote, MelodyNote, Note};

/// Sample of an individual stem
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct StemSample {
    pub value: f32,
    pub byte_index: u64,
}

/// Single sample of the stems
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct StemsSample {
    pub voice: StemSample,
    pub square: StemSample,
    pub triangle: StemSample,
    pub noise: f32,
    pub sfx: StemSample,
}

pub trait SampleStem {
    /// Sample the square stem
    fn sample_square(&self, sample_rate: u32) -> Vec<StemSample>;
    /// Sample the triangle stem
    fn sample_triangle(&self, sample_rate: u32) -> Vec<StemSample>;
}

pub trait DescribeNote {
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

impl DescribeNote for SquareNote {
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

impl DescribeNote for TriangleNote {
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

/// Sample the voice, square and triangle stems
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Sampler {
    melody_notes: VecDeque<MelodyNote>,
    bass_notes: VecDeque<BassNote>,
    voice_samples: VecDeque<StemSample>,
    sfx_samples: VecDeque<StemSample>,
    square_samples: VecDeque<StemSample>,
    triangle_samples: VecDeque<StemSample>,
}

impl Consume for Sampler {
    type Item = Note;

    fn push(&mut self, note: Note) {
        match note {
            Note::Melody(n) => self.melody_notes.push_back(n),
            Note::Bass(n) => self.bass_notes.push_back(n),
        }
    }

    fn extend(&mut self, notes: &[Note]) {
        for note in notes {
            self.push(*note);
        }
    }

    fn capacity(&self) -> usize {
        self.melody_notes.capacity().min(self.bass_notes.capacity())
    }

    fn len(&self) -> usize {
        self.melody_notes.len().min(self.bass_notes.len())
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Sampler {
    /// Return the next sample
    pub fn sample(&mut self, plugin: &Box<dyn Plugin>, sample_rate: u32) -> Option<StemsSample> {
        loop {
            // Drain the current available samples
            if self.square_samples.len() > 0 && self.triangle_samples.len() > 0 {
                let Some(square) = self.square_samples.pop_front() else {
                    // Can't happen
                    return None;
                };

                let Some(triangle) = self.triangle_samples.pop_front() else {
                    // Can't happen
                    return None;
                };

                let voice = self.voice_samples.pop_front().unwrap_or_default();
                let sfx = self.sfx_samples.pop_front().unwrap_or_default();
                return Some(StemsSample {
                    voice,
                    square,
                    triangle,
                    noise: 0.0,
                    sfx,
                });
            }

            // Sample melody notes if there are too much bass samples
            while self.triangle_samples.len() > self.square_samples.len() {
                let Some(note) = self.melody_notes.pop_front() else {
                    // Not enough melody notes to continue
                    return None;
                };

                self.square_samples
                    .extend(plugin.sample_melody_note(note, sample_rate));
                self.voice_samples
                    .extend(plugin.sample_voice_note(note, sample_rate));
                self.sfx_samples
                    .extend(plugin.sample_sfx_note(note, sample_rate));
            }

            // Sample the next bass note if any
            if let Some(note) = self.bass_notes.pop_front() {
                self.triangle_samples
                    .extend(plugin.sample_bass_note(note, sample_rate));
            }

            // Stop here if we can't generate enough bass samples
            if self.triangle_samples.is_empty() {
                return None;
            }
        }
    }

    /// Sample the notes currently in the buffer
    pub fn samples(&mut self, plugin: &Box<dyn Plugin>, sample_rate: u32) -> Vec<StemsSample> {
        let mut samples = Vec::new();
        while let Some(sample) = self.sample(plugin, sample_rate) {
            samples.push(sample);
        }

        samples
    }
}
