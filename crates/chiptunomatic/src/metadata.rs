//! [`SongMetadata`] construction from seeds, strings, or filesystem paths.

use core::fmt::Display;

use alloc::{string::String, vec::Vec};

use crate::plugin::SectionDef;
use crate::DrumPattern;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    pub bpm: i32,
    pub beat_duration: f64,
    pub sixteenth: f64,
}

impl Timing {
    pub fn from_bpm(bpm: i32) -> Self {
        let beat_duration = 60.0 / f64::from(bpm);
        Self {
            bpm,
            beat_duration,
            sixteenth: beat_duration / 4.0,
        }
    }
}

/// Timing, key, BPM, chords, drum loop, etc. derived from a file seed.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct SongMetadata {
    pub seed: Vec<u8>,
    pub rng_seed: u64,
    pub root_semitone: u8,
    pub bpm: i32,
    pub chord_progression: Vec<usize>,
    pub chord_description: String,
    pub timing: Timing,
    pub total_beats: u64,
    pub total_duration: f64,
    pub total_duration_str: String,
    pub beat_skip_bytes: u64,
    pub data_byte_len: u64,
    pub data_byte_len_str: String,
    pub drum_pattern: DrumPattern,
    pub drum_seed: [u8; 8],
    pub sections: Vec<SectionDef>,
}

impl Display for SongMetadata {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(
            f,
            "root: {} bpm: {} beats: {}",
            self.root_semitone, self.bpm, self.total_beats
        )?;
        writeln!(
            f,
            "duration: {} data: {}",
            self.total_duration_str, self.data_byte_len_str
        )?;
        writeln!(f, "chords: {}", self.chord_description)
    }
}
