use core::fmt::Debug;

use alloc::boxed::Box;
pub use alloc::string::String;
pub use alloc::vec::Vec;
use dyn_clone::DynClone;

use crate::random::Random;
use crate::synth::{envelope, midi_to_hz, vibrato_sine};
use crate::{constants::PENTATONIC_MINOR, BassNote, DrumPattern, DrumStep, MelodyNote, StemSample};

pub mod chiptune;
pub mod koto;
pub mod lofi;
pub mod medieval;
pub mod metal;
pub mod persian;
pub mod rap;
pub mod rock;
pub mod samba;
pub mod toy;
pub mod trap;

#[derive(Debug)]
pub struct SampleStepConfig<'a> {
    pub sample_rate: f64,
    pub step_duration: f64,
    pub pattern: usize,
    pub color: u8,
    pub random: &'a mut Box<dyn Random>,
}

/// Controls which stems are audible in a song section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StemMask {
    pub voice: bool,
    pub square: bool,
    pub triangle: bool,
    pub noise: bool,
    pub sfx: bool,
}

impl StemMask {
    pub const ALL: Self = Self {
        voice: true,
        square: true,
        triangle: true,
        noise: true,
        sfx: true,
    };
}

impl Default for StemMask {
    fn default() -> Self {
        Self::ALL
    }
}

/// One entry in a plugin's repeating song-structure table.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct SectionDef {
    /// Length of this section in beats.
    pub beats: u64,
    /// Which stems are audible during this section.
    pub stems: StemMask,
    /// Seconds of silence inserted after this section ends.
    pub silence_after: f64,
}

pub trait Plugin: Debug + DynClone + Send {
    /// Return the mode name
    fn mode(&self) -> &'static str;
    fn mode_string(&self) -> String;

    /// Return the melody scale
    fn scale(&self) -> &[u8] {
        &PENTATONIC_MINOR
    }

    /// Generate a chord progression from a seed
    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize>;
    /// Generate a tempo from a seed
    fn tempo_from_seed(&self, seed: u32) -> i32;
    /// Sample the square wave of a melody note
    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample>;
    /// Sample the triangle wave of a bass note
    fn sample_bass_note(&self, note: BassNote, sample_rate: u32) -> Vec<StemSample>;

    /// Whether this plugin produces SFX on the sfx stem.
    /// Defaults to false; override to true in plugins that implement `sample_sfx_note`.
    fn has_sfx(&self) -> bool {
        false
    }

    /// Sample an SFX note for a dedicated effects stem.
    /// The default returns silence; plugins override for style-specific SFX.
    fn sample_sfx_note(&self, _note: MelodyNote, _sample_rate: u32) -> Vec<StemSample> {
        Vec::new()
    }

    /// Sample a voice note (humming/singing) following the melody.
    /// The default produces a gentle vibrato sine that blends across all modes;
    /// plugins may override for a style-specific vocal timbre.
    fn sample_voice_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // 5.5 Hz vibrato, ±1.5 % depth — standard human vocal vibrato parameters.
        let raw = vibrato_sine(sr, midi_to_hz(note.midi), dur, 0.12, 5.5, 0.015);
        let shaped = envelope(sr, &raw, 0.030, 0.080, 0.75, 0.10);
        (0..total)
            .map(|i| StemSample {
                value: shaped.get(i).copied().unwrap_or(0.0),
                byte_index: note.byte_index,
            })
            .collect()
    }

    /// Sample a drum step
    fn sample_step<'a>(
        &self,
        step: &DrumStep,
        config: SampleStepConfig<'a>,
        samples: &mut Vec<f32>,
    );

    /// Build a 16-step drum pattern from an 8-byte seed.
    /// Plugins override this to return sparser or style-specific patterns.
    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        DrumPattern::from_seed(seed)
    }

    /// Return the repeating section table for this plugin.
    /// An empty slice (the default) disables structured sections.
    fn section_defs(&self) -> &'static [SectionDef] {
        &[]
    }

    /// Generate the section table from the song seed.
    /// Override for seed-driven variation; default converts the static `section_defs()` slice.
    fn section_defs_from_seed(&self, _seed: u32) -> Vec<SectionDef> {
        self.section_defs().to_vec()
    }
}

dyn_clone::clone_trait_object!(Plugin);

pub(crate) fn overlay_samples(src: &Vec<f32>, dst: &mut Vec<f32>) {
    for i in 0..dst.len().min(src.len()) {
        dst[i] += src[i];
    }
}
