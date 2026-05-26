use alloc::boxed::Box;
use alloc::vec::Vec;
use getset::{Getters, Setters};

use crate::constants::{CHORD_BEATS, HARMONY_BASE, MELODY_BASE};
use crate::plugin::Plugin;
use crate::{MelodyNote, Note, SongMetadata};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpeggioPattern {
    #[default]
    Up,
    Down,
    /// Ping-pong: 0 → N-1 → 1 → N-1 → … (endpoints not doubled).
    UpDown,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArpeggiatorConfig {
    /// How many sub-notes each melody note is split into (clamped to ≥ 1).
    pub subdivisions: u8,
    pub pattern: ArpeggioPattern,
}

/// Wraps [`SongNoteReader`] and expands each [`SongNote::Melody`] into
/// `subdivisions` shorter sub-notes cycling through the active chord tones.
/// [`SongNote::Bass`] passes through unchanged.
#[derive(Default, Debug, Clone, PartialEq, Getters, Setters)]
pub struct Arpeggiator {
    #[getset(get = "pub", get_mut = "pub", set = "pub")]
    config: ArpeggiatorConfig,
    melody_time: f64,
}

impl Arpeggiator {
    pub fn with_config(self, config: ArpeggiatorConfig) -> Self {
        Self { config, ..self }
    }

    /// Reset the melody time
    pub fn reset(&mut self) {
        self.melody_time = 0.0
    }

    /// Expand a note.
    /// Bass notes are kept unchanged
    pub fn expand_note(
        &mut self,
        metadata: &SongMetadata,
        plugin: &Box<dyn Plugin>,
        note: &Note,
    ) -> Vec<Note> {
        if let Note::Melody(note) = note {
            let n = self.config.subdivisions.max(1) as usize;
            let sub_dur = note.duration / n as f64;

            let scale = plugin.scale();
            let chord_idx = (self.melody_time / metadata.timing.beat_duration / CHORD_BEATS as f64)
                as usize
                % metadata.chord_progression.len();
            let chord_deg = metadata.chord_progression[chord_idx];

            self.melody_time += note.duration;

            // Cycle through 3 consecutive scale degrees starting at the chord root,
            // clamped to the scale length.
            let num_tones = 3usize.min(scale.len());

            (0..n)
                .map(|i| {
                    let offset = self.tone_index(i, num_tones);
                    let deg = (chord_deg + offset) % scale.len();
                    let harm_deg = if deg > 0 { deg - 1 } else { scale.len() - 1 };

                    let midi = (MELODY_BASE
                        + metadata.root_semitone as f64
                        + scale[deg] as f64
                        + (note.octave * 12) as f64)
                        .max(21.0)
                        .min(108.0);

                    let harmony_midi = (HARMONY_BASE
                        + metadata.root_semitone as f64
                        + scale[harm_deg] as f64
                        + (note.octave * 12) as f64)
                        .max(21.0)
                        .min(108.0);

                    Note::Melody(MelodyNote {
                        midi,
                        harmony_midi,
                        duration: sub_dur,
                        degree: deg,
                        octave: note.octave,
                        byte_index: note.byte_index,
                    })
                })
                .collect()
        } else {
            let mut notes: Vec<Note> = Default::default();
            notes.push(*note);
            notes
        }
    }

    /// Expand multiple notes.
    /// Bass notes are kept unchanged
    pub fn expand_notes(
        &mut self,
        metadata: &SongMetadata,
        plugin: &Box<dyn Plugin>,
        notes: &[Note],
    ) -> Vec<Note> {
        notes
            .iter()
            .map(|n| self.expand_note(metadata, plugin, n))
            .flatten()
            .collect()
    }

    fn tone_index(&self, i: usize, num_tones: usize) -> usize {
        match self.config.pattern {
            ArpeggioPattern::Up => i % num_tones,
            ArpeggioPattern::Down => (num_tones - 1) - i % num_tones,
            ArpeggioPattern::UpDown => {
                // period = 2*(N-1): e.g. N=3 → 0,1,2,1, 0,1,2,1, …
                let cycle = (num_tones - 1) * 2;
                let pos = i % cycle;
                if pos < num_tones {
                    pos
                } else {
                    cycle - pos
                }
            }
        }
    }
}
