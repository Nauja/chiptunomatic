//! Incremental [`SongNote`] construction: feed bytes as they arrive, poll for generated note events.

use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;

use crate::{
    constants::{BASS_BASE, CHORD_BEATS, DURATIONS, HARMONY_BASE, MELODY_BASE},
    consumer::Consume,
    plugin::Plugin,
};
use crate::{DescribeNote, SongMetadata};

/// Note for the melody
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct MelodyNote {
    pub midi: f64,
    pub harmony_midi: f64,
    pub duration: f64,
    pub degree: usize,
    pub octave: i32,
    pub byte_index: u64,
}

impl DescribeNote for MelodyNote {
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

/// Note for the bass
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct BassNote {
    pub midi: f64,
    pub duration: f64,
    pub byte_index: u64,
}

impl DescribeNote for BassNote {
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

/// Either a melody of bass note
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Note {
    Melody(MelodyNote),
    Bass(BassNote),
}

impl Default for Note {
    fn default() -> Self {
        Self::Melody(Default::default())
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
enum SongNoteReaderState {
    #[default]
    MelodyUpperByte,
    MelodyLowerByte,
    BassByte,
    Skip(u64),
}

/// Mutable melodic walk state across [`melody_note_from_bytes`] calls (matches sequential generator).
#[derive(Debug, Clone, PartialEq, Eq)]
struct MelodyWalkState {
    pub degree: isize,
    pub octave: i32,
    pub direction: isize,
}

impl Default for MelodyWalkState {
    fn default() -> Self {
        Self {
            degree: 0,
            octave: 0,
            direction: 1,
        }
    }
}

/// Drives interleaved melody/bass notes generation from a growable byte stream
#[derive(Default, Debug, Clone)]
pub struct NoteGenerator {
    // Input byte stream
    stream: VecDeque<u8>,
    // Position in the input byte stream
    pub(crate) position: u64,
    state: SongNoteReaderState,
    // Last read upper byte for melody note
    melody_upper_byte: u8,
    // Current time of the melody
    pub(crate) melody_time: f64,
    // Current time of the bass
    pub(crate) bass_time: f64,
    bass_step: usize,
    melody_state: MelodyWalkState,
}

impl Consume for NoteGenerator {
    type Item = u8;

    // Add a single byte to the buffer
    fn push(&mut self, byte: u8) {
        self.stream.push_back(byte);
    }

    // Add bytes to the buffer
    fn extend(&mut self, bytes: &[u8]) {
        self.stream.extend(bytes);
    }

    // Capacity of the byte buffer
    fn capacity(&self) -> usize {
        self.stream.capacity()
    }

    // Length of the byte buffer
    fn len(&self) -> usize {
        self.stream.len()
    }

    // If the byte buffer is empty
    fn is_empty(&self) -> bool {
        self.stream.is_empty()
    }
}

impl NoteGenerator {
    // Generate the next note from the byte buffer, returning None if the
    // buffer is empty
    pub fn note(&mut self, metadata: &SongMetadata, plugin: &Box<dyn Plugin>) -> Option<Note> {
        loop {
            let Some(byte) = self.stream.pop_front() else {
                return None;
            };

            match self.state {
                SongNoteReaderState::MelodyUpperByte => {
                    self.melody_upper_byte = byte;
                    self.position += 1;
                    self.state = SongNoteReaderState::MelodyLowerByte;
                }
                SongNoteReaderState::MelodyLowerByte => {
                    // Generate the melody note with 2 bytes
                    let melody = self.melody_note(metadata, plugin, self.melody_upper_byte, byte);

                    // Advance the melody time
                    self.melody_time += melody.duration;

                    self.position += 1;
                    self.state = SongNoteReaderState::BassByte;
                    return Some(Note::Melody(melody));
                }
                SongNoteReaderState::BassByte => {
                    // Generate the bass note
                    let bass = self.bass_note(metadata, plugin, byte);

                    // Advance the bass time
                    self.bass_time += bass.duration;
                    self.bass_step += 1;

                    self.position += 1;
                    self.state = if metadata.beat_skip_bytes == 0 {
                        SongNoteReaderState::MelodyUpperByte
                    } else {
                        SongNoteReaderState::Skip(metadata.beat_skip_bytes)
                    };
                    return Some(Note::Bass(bass));
                }
                // Skip n consecutive bytes to produce less beats for bigger files
                SongNoteReaderState::Skip(n) => {
                    self.position += 1;
                    self.state = if n > 1 {
                        SongNoteReaderState::Skip(n - 1)
                    } else {
                        SongNoteReaderState::MelodyUpperByte
                    }
                }
            }
        }
    }

    // Generate all notes from the byte buffer
    pub fn notes(&mut self, metadata: &SongMetadata, plugin: &Box<dyn Plugin>) -> Vec<Note> {
        let mut notes = Vec::new();

        loop {
            notes.push(match self.note(metadata, plugin) {
                None => return notes,
                Some(n) => n,
            });
        }
    }

    // Read n notes from the byte buffer
    pub fn notes_buf(
        &mut self,
        metadata: &SongMetadata,
        plugin: &Box<dyn Plugin>,
        notes: &mut [Note],
    ) -> usize {
        let mut notes_written = 0;
        let limit = notes.len();
        while notes_written < limit {
            notes[notes_written] = match self.note(metadata, plugin) {
                None => return notes_written,
                Some(n) => n,
            };
            notes_written += 1;
        }

        notes_written
    }

    /// Generate the duration of a note from a byte
    fn bytes_to_duration(&self, metadata: &SongMetadata, byte_val: u8) -> f64 {
        let idx = byte_val as usize % DURATIONS.len();
        DURATIONS[idx] as f64 * metadata.timing.sixteenth
    }

    /// Generate a melody note from a pair of bytes
    fn melody_note(
        &mut self,
        metadata: &SongMetadata,
        plugin: &Box<dyn Plugin>,
        upper_byte: u8,
        lower_byte: u8,
    ) -> MelodyNote {
        let scale: &[u8] = plugin.scale();
        let chord_duration = CHORD_BEATS as f64 * metadata.timing.beat_duration;

        let duration = self.bytes_to_duration(metadata, lower_byte);
        let chord_idx = (self.melody_time / metadata.timing.beat_duration / CHORD_BEATS as f64)
            as usize
            % metadata.chord_progression.len();
        let chord_deg = metadata.chord_progression[chord_idx];

        let phrase_duration = chord_duration * metadata.chord_progression.len() as f64;
        let time_left_in_chord = chord_duration - (self.melody_time % chord_duration);
        let time_left_in_phrase = phrase_duration - (self.melody_time % phrase_duration);
        if time_left_in_chord < metadata.timing.beat_duration * 1.5 {
            // Chord boundary: snap to chord root.
            self.melody_state.degree = chord_deg as isize;
            self.melody_state.octave = 0;
        } else if time_left_in_phrase < metadata.timing.beat_duration * 2.0 {
            // Phrase end: steer degree back toward the chord root so each
            // phrase resolves before the next one begins.
            let target = chord_deg as isize;
            let step = (target - self.melody_state.degree).signum();
            self.melody_state.direction = if step != 0 {
                step
            } else {
                self.melody_state.direction
            };
            self.melody_state.degree =
                (self.melody_state.degree + step).clamp(0, scale.len() as isize - 1);
        } else {
            let step_choice = (upper_byte % 8) as u32;
            let delta = if step_choice < 4 {
                self.melody_state.direction
            } else if step_choice < 6 {
                let dlt = -self.melody_state.direction;
                self.melody_state.direction = -self.melody_state.direction;
                dlt
            } else if step_choice == 6 {
                0
            } else {
                self.melody_state.direction * 2
            };

            let mut new_degree = self.melody_state.degree + delta;

            while new_degree >= scale.len() as isize {
                new_degree -= scale.len() as isize;
                if self.melody_state.octave < 1 {
                    self.melody_state.octave += 1;
                } else {
                    new_degree = scale.len() as isize - 2;
                    self.melody_state.direction = -1;
                }
            }
            while new_degree < 0 {
                new_degree += scale.len() as isize;
                if self.melody_state.octave > -1 {
                    self.melody_state.octave -= 1;
                } else {
                    new_degree = 1;
                    self.melody_state.direction = 1;
                }
            }

            self.melody_state.degree = new_degree;
        }

        let mut midi = MELODY_BASE
            + metadata.root_semitone as f64
            + scale[self.melody_state.degree as usize] as f64
            + (self.melody_state.octave * 12) as f64;
        midi = midi.max(21.0).min(108.0);

        // Generate the harmony note
        let harm_degree = (self.melody_state.degree as usize).saturating_sub(1);
        let mut harmony_midi = HARMONY_BASE
            + metadata.root_semitone as f64
            + scale[harm_degree] as f64
            + (self.melody_state.octave * 12) as f64;
        harmony_midi = harmony_midi.max(21.0).min(108.0);

        MelodyNote {
            midi,
            harmony_midi,
            duration,
            degree: self.melody_state.degree as usize,
            octave: self.melody_state.octave,
            byte_index: self.position,
        }
    }

    /// Generate a bass note half-step from a byte
    fn bass_note(&self, metadata: &SongMetadata, plugin: &Box<dyn Plugin>, b: u8) -> BassNote {
        let scale = plugin.scale();
        let chord_idx = (self.bass_time / metadata.timing.beat_duration / CHORD_BEATS as f64)
            as usize
            % metadata.chord_progression.len();
        let chord_deg = metadata.chord_progression[chord_idx];

        let chord_root = BASS_BASE + metadata.root_semitone as f64 + scale[chord_deg] as f64;
        let chord_fifth = chord_root + 7.0;
        let chord_color =
            BASS_BASE + metadata.root_semitone as f64 + scale[(chord_deg + 1) % scale.len()] as f64;

        let midi = if self.bass_step % 2 == 0 {
            if (b & 0x07) == 0 {
                chord_color
            } else {
                chord_root
            }
        } else if (b & 0x07) == 0 {
            chord_color
        } else {
            chord_fifth
        };

        let midi = midi.max(21.0).min(108.0);
        let half = metadata.timing.beat_duration * 2.0;
        BassNote {
            midi,
            duration: half,
            byte_index: self.position,
        }
    }
}
