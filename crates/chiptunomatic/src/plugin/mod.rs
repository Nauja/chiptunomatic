use core::fmt::Debug;

use alloc::rc::Rc;
pub use alloc::string::String;
pub use alloc::vec::Vec;
use dyn_clone::DynClone;

use crate::synth::{envelope, midi_to_hz, vibrato_sine};
#[cfg(feature = "arpeggio")]
use crate::ArpeggioConfig;
use crate::{
    constants::PENTATONIC_MINOR, BassNote, DrumPattern, DrumSample, DrumStep, MelodyNote,
    StemSample,
};

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

/// Hide the generic Rng object
pub trait Random: Debug {
    fn next_float(&self) -> f32;
}

#[derive(Debug)]
pub struct SampleStepConfig<'a> {
    pub sample_rate: f64,
    pub step_duration: f64,
    pub pattern: usize,
    pub color: u8,
    pub random: &'a Rc<dyn Random>,
}

pub trait Plugin: Debug + DynClone {
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
        step: DrumStep,
        config: SampleStepConfig<'a>,
        samples: &mut Vec<DrumSample>,
    );

    /// Build a 16-step drum pattern from an 8-byte seed.
    /// Plugins override this to return sparser or style-specific patterns.
    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        DrumPattern::from_seed(seed)
    }

    #[cfg(feature = "arpeggio")]
    /// Return an arpeggio configuration for this plugin, or `None` to play
    /// notes as-is.  Wrap a [`SongNoteReader`] with [`ArpeggioNoteReader`]
    /// to apply it.
    fn arpeggio(&self) -> Option<ArpeggioConfig> {
        None
    }
}

dyn_clone::clone_trait_object!(Plugin);

pub(crate) fn overlay_samples(src: &Vec<DrumSample>, dst: &mut Vec<DrumSample>) {
    for i in 0..dst.len().min(src.len()) {
        dst[i] += src[i];
    }
}
