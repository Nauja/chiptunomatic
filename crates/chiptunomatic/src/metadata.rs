//! [`SongMetadata`] construction from seeds, strings, or filesystem paths.

use core::fmt::Display;

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::{
    constants::{CHORD_PROGRESSIONS, NOTE_NAMES, PENTATONIC_MINOR},
    DrumPattern,
};
use thiserror::Error;

/// Chord progression: indexes [`crate::CHORD_PROGRESSIONS`] from `seed % len`.
pub fn chord_progression_from_seed(seed: u32) -> Vec<usize> {
    let idx = (seed as usize) % CHORD_PROGRESSIONS.len();
    CHORD_PROGRESSIONS[idx].to_vec()
}

fn pick_root_and_tempo(seed: u32) -> (u8, i32) {
    let root = (seed % 12) as u8;
    let tempo_variation = ((seed >> 8) % 40) as i32 - 20;
    (root, 120 + tempo_variation)
}

#[derive(Error, Debug)]
pub enum GenerateError {
    #[error("empty input")]
    EmptyInput,
    #[error("invalid seed")]
    InvalidSeed,
}

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
#[derive(Error, Debug)]
pub enum SongMetadataFromPathError {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("file too large")]
    FileTooLarge,
    #[error("error generating")]
    Generate(#[from] GenerateError),
}

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

fn format_duration(secs: f64) -> String {
    let total = secs.floor() as u64;
    if total < 60 {
        format!("{total}s")
    } else {
        let m = total / 60;
        let s = total % 60;
        format!("{m}:{s:02}")
    }
}

/// Formats byte length using `K` / `M` / `G` at 1024-based thresholds (`>= 1 KiB`, etc.).
fn format_data_byte_size(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let x = n as f64;
    if n >= 1 << 30 {
        format!("{:.2} G", x / GB)
    } else if n >= 1 << 20 {
        format!("{:.2} M", x / MB)
    } else if n >= 1 << 10 {
        format!("{:.2} K", x / KB)
    } else {
        format!("{n} B")
    }
}

/// Timing, key, BPM, chords, drum loop, etc. carried with a [`crate::SongPlan`].
#[derive(Debug, Clone, PartialEq)]
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
    pub data_byte_len: u64,
    pub data_byte_len_str: String,
    pub drum_pattern: DrumPattern,
    // Seed that generated the drum pattern
    pub drum_seed: [u8; 8],
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

impl SongMetadata {
    /// Builds [`SongMetadata`] from a bytes seed (root/tempo, chords, drums via [`chord_progression_from_seed`],
    /// [`drum_pattern_from_seed`]) and the **stream byte length** `data_byte_len` used to derive [`SongMetadata::total_beats`].
    pub fn from_seed(seed: &[u8], data_byte_len: u64) -> Result<Self, GenerateError> {
        if data_byte_len == 0 {
            return Err(GenerateError::EmptyInput);
        }
        let digest = md5::compute(seed).0;
        let rng_seed = u64::from(
            u32::from_be_bytes([digest[12], digest[13], digest[14], digest[15]]) % (1u32 << 31),
        );

        let root_seed = u32::from_be_bytes(
            digest[0..4]
                .try_into()
                .map_err(|_| GenerateError::InvalidSeed)?,
        );
        let chord_seed = u32::from_be_bytes(
            digest[4..8]
                .try_into()
                .map_err(|_| GenerateError::InvalidSeed)?,
        );
        let drum_pattern_seed: [u8; 8] = digest[8..16]
            .try_into()
            .map_err(|_| GenerateError::InvalidSeed)?;

        let (root, bpm) = pick_root_and_tempo(root_seed);
        let timing = Timing::from_bpm(bpm);
        let total_beats = data_byte_len / 3;
        let total_duration = (total_beats as f64) * timing.beat_duration;
        let chord_progression = chord_progression_from_seed(chord_seed);
        let drum_pattern = DrumPattern::from_seed(&drum_pattern_seed);
        let chord_description = chord_progression
            .iter()
            .map(|&d| {
                let note_idx = (root as usize + usize::from(PENTATONIC_MINOR[d])) % 12;
                NOTE_NAMES[note_idx].to_string()
            })
            .collect::<Vec<_>>()
            .join(" – ");

        Ok(Self {
            seed: digest.into(),
            rng_seed,
            root_semitone: root,
            bpm,
            total_beats,
            total_duration,
            total_duration_str: format_duration(total_duration),
            chord_progression,
            chord_description,
            timing,
            data_byte_len,
            data_byte_len_str: format_data_byte_size(data_byte_len),
            drum_pattern,
            drum_seed: drum_pattern_seed,
        })
    }

    /// [`SongMetadata::from_seed`] using the UTF-8 bytes of any value convertible with [`Into<String>`]
    /// (same digest layout as hashing that string).
    pub fn from_string<S: Into<String>>(s: S, data_byte_len: u64) -> Result<Self, GenerateError> {
        let s = s.into();
        Self::from_seed(s.as_bytes(), data_byte_len)
    }

    /// [`SongMetadata::from_string`] backed by **`path`’s** final component (`Path::file_name`, lossy Unicode;
    /// falls back to the full path if missing), with **`data_byte_len`** from **`std::fs::metadata(path)`**.
    ///
    /// **Not defined** when targeting **`wasm32-unknown-unknown`** (browser WebAssembly); use
    /// [`SongMetadata::from_string`] with an explicit name and length instead.
    #[cfg(all(
        feature = "std",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ))]
    pub fn from_path<P: std::convert::AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, SongMetadataFromPathError> {
        let path = path.as_ref();
        let md = std::fs::metadata(path).map_err(SongMetadataFromPathError::Io)?;

        let seed_name = path
            .file_name()
            .map(|os| os.to_string_lossy())
            .unwrap_or_else(|| path.as_os_str().to_string_lossy());

        Self::from_string(seed_name, md.len()).map_err(SongMetadataFromPathError::Generate)
    }
}
