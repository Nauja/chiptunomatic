//! Incremental [`SongNote`] construction: feed bytes as they arrive, poll for generated note events.

use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;

use crate::constants::{
    BASS_BASE, CHORD_BEATS, DURATIONS, HARMONY_BASE, MELODY_BASE, PENTATONIC_MINOR,
};
use crate::{
    song::{Note, SampleStem, SquareNote, TriangleNote},
    SongMetadata, StemSample,
};

/// Note for the melody
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct MelodyNote {
    pub midi: f64,
    pub harmony_midi: f64,
    pub duration: f64,
    pub degree: usize,
    pub octave: i32,
    /// [`ByteStream`] position before reading the first melody byte (b1) for this note;
    /// the second byte is always at offset `stream_offset_b1 + 1` in stream order.
    /// Map to file indices with `% data.len()` for wraparound.
    pub byte_index: u64,
}

impl Note for MelodyNote {
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

impl SampleStem for MelodyNote {
    fn sample_square(&self, sample_rate: u32) -> Vec<StemSample> {
        if self.duration <= 0.0 {
            return Vec::new();
        }

        // Sample the melody note first
        let mut melody_samples = SquareNote {
            midi: self.midi,
            duration: self.duration,
            amp: 0.28,
            duty: 0.5,
            byte_index: self.byte_index,
        }
        .sample_square(sample_rate);

        // Sample the harmony note
        let harmony_samples = SquareNote {
            midi: self.harmony_midi,
            duration: self.duration,
            amp: 0.18,
            duty: 0.25,
            byte_index: self.byte_index,
        }
        .sample_square(sample_rate);

        for i in 0..melody_samples.len() {
            melody_samples[i].value += harmony_samples[i].value;
        }

        melody_samples
    }

    fn sample_triangle(&self, _sample_rate: u32) -> Vec<StemSample> {
        Default::default()
    }
}

/// Note for the bass
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct BassNote {
    pub midi: f64,
    pub duration: f64,
    /// [`ByteStream`] position before reading the bass control byte for this half-note step.
    /// Map to file indices with `% data.len()` for wraparound.
    pub byte_index: u64,
}

impl Note for BassNote {
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

impl SampleStem for BassNote {
    fn sample_square(&self, _sample_rate: u32) -> Vec<StemSample> {
        Default::default()
    }

    fn sample_triangle(&self, sample_rate: u32) -> Vec<StemSample> {
        TriangleNote {
            midi: self.midi,
            duration: self.duration,
            byte_index: self.byte_index,
        }
        .sample_triangle(sample_rate)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SongNote {
    Melody(MelodyNote),
    Bass(BassNote),
}

impl Default for SongNote {
    fn default() -> Self {
        Self::Melody(Default::default())
    }
}

impl SampleStem for SongNote {
    fn sample_square(&self, sample_rate: u32) -> Vec<StemSample> {
        match self {
            SongNote::Melody(note) => note.sample_square(sample_rate),
            SongNote::Bass(note) => note.sample_square(sample_rate),
        }
    }

    fn sample_triangle(&self, sample_rate: u32) -> Vec<StemSample> {
        match self {
            SongNote::Melody(note) => note.sample_triangle(sample_rate),
            SongNote::Bass(note) => note.sample_triangle(sample_rate),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum SongNoteReaderState {
    MelodyUpperByte,
    MelodyLowerByte,
    BassByte,
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

/// Drives interleaved melody/bass generation from a growable byte stream (see [`ByteBufferStream`]).
#[derive(Debug)]
pub struct SongNoteReader {
    metadata: SongMetadata,
    // Input byte buffer
    buffer: VecDeque<u8>,
    // Buffer position
    position: u64,
    state: SongNoteReaderState,
    // Last read upper byte for melody note
    melody_upper_byte: u8,
    // Current time of the melody
    melody_time: f64,
    // Current time of the bass
    bass_time: f64,
    bass_step: usize,
    melody_state: MelodyWalkState,
    half: f64,
}

impl SongNoteReader {
    pub fn new(metadata: SongMetadata) -> Self {
        let half = metadata.timing.beat_duration * 2.0;

        Self {
            metadata,
            buffer: Default::default(),
            position: 0,
            state: SongNoteReaderState::MelodyUpperByte,
            melody_upper_byte: 0,
            melody_time: 0.0,
            bass_time: 0.0,
            melody_state: MelodyWalkState::default(),
            bass_step: 0,
            half,
        }
    }

    pub fn with_capacity(self, capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            ..self
        }
    }

    pub fn with_buffer(mut self, buffer: &[u8]) -> Self {
        self.extend(buffer);
        self
    }

    // Add a single byte to the buffer
    pub fn push(&mut self, byte: u8) {
        self.buffer.push_back(byte);
    }

    // Add bytes to the buffer
    pub fn extend(&mut self, bytes: &[u8]) {
        self.buffer.extend(bytes);
    }

    // Capacity of the byte buffer
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    // Length of the byte buffer
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    // If the byte buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    // Position in the byte buffer
    pub const fn position(&self) -> u64 {
        self.position
    }

    pub const fn melody_time(&self) -> f64 {
        self.melody_time
    }

    pub const fn bass_time(&self) -> f64 {
        self.bass_time
    }

    // Read the next note from the byte buffer, returning None if the
    // buffer is empty
    pub fn read_note(&mut self) -> Option<SongNote> {
        loop {
            let Some(byte) = self.buffer.pop_front() else {
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
                    let melody = self.melody_note(self.melody_upper_byte, byte);

                    // Advance the melody time
                    self.melody_time += melody.duration;

                    self.position += 1;
                    self.state = SongNoteReaderState::BassByte;
                    return Some(SongNote::Melody(melody));
                }
                SongNoteReaderState::BassByte => {
                    // Generate the bass note
                    let bass = self.bass_note(byte);

                    // Advance the bass time
                    self.bass_time += self.half;
                    self.bass_step += 1;

                    self.position += 1;
                    self.state = SongNoteReaderState::MelodyUpperByte;
                    return Some(SongNote::Bass(bass));
                }
            }
        }
    }

    // Read all notes from the byte buffer
    pub fn read_notes(&mut self) -> Vec<SongNote> {
        let mut notes = Vec::new();

        loop {
            notes.push(match self.read_note() {
                None => return notes,
                Some(n) => n,
            });
        }
    }

    // Read n notes from the byte buffer
    pub fn read_notes_buf(&mut self, notes: &mut [SongNote]) -> usize {
        let mut notes_written = 0;
        let limit = notes.len();
        while notes_written < limit {
            notes[notes_written] = match self.read_note() {
                None => return notes_written,
                Some(n) => n,
            };
            notes_written += 1;
        }

        notes_written
    }

    /// Generate the duration of a note from a byte
    fn bytes_to_duration(&self, byte_val: u8) -> f64 {
        let idx = byte_val as usize % DURATIONS.len();
        DURATIONS[idx] as f64 * self.metadata.timing.sixteenth
    }

    /// One melodic note from a pair of bytes
    fn melody_note(&mut self, upper_byte: u8, lower_byte: u8) -> MelodyNote {
        // Generate the melody note
        let scale = PENTATONIC_MINOR;
        let chord_duration = CHORD_BEATS as f64 * self.metadata.timing.beat_duration;

        let duration = self.bytes_to_duration(lower_byte);
        let chord_idx = (self.melody_time / self.metadata.timing.beat_duration / CHORD_BEATS as f64)
            as usize
            % self.metadata.chord_progression.len();
        let chord_deg = self.metadata.chord_progression[chord_idx];

        let time_left_in_chord = chord_duration - (self.melody_time % chord_duration);
        if time_left_in_chord < self.metadata.timing.beat_duration * 1.5 {
            self.melody_state.degree = chord_deg as isize;
            self.melody_state.octave = 0;
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
            + self.metadata.root_semitone as f64
            + scale[self.melody_state.degree as usize] as f64
            + (self.melody_state.octave * 12) as f64;
        midi = midi.max(21.0).min(108.0);

        // Generate the harmony note
        let scale = PENTATONIC_MINOR;
        let harm_degree = (self.melody_state.degree as usize).saturating_sub(1);
        let mut harmony_midi = HARMONY_BASE
            + self.metadata.root_semitone as f64
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

    /// One bass half-step from a byte
    fn bass_note(&self, b: u8) -> BassNote {
        let scale = PENTATONIC_MINOR;

        let chord_idx = (self.bass_time / self.metadata.timing.beat_duration / CHORD_BEATS as f64)
            as usize
            % self.metadata.chord_progression.len();
        let chord_deg = self.metadata.chord_progression[chord_idx];

        let chord_root = BASS_BASE + self.metadata.root_semitone as f64 + scale[chord_deg] as f64;
        let chord_fifth = chord_root + 7.0;
        let chord_color = BASS_BASE
            + self.metadata.root_semitone as f64
            + scale[(chord_deg + 1) % scale.len()] as f64;

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
        BassNote {
            midi,
            duration: self.half,
            byte_index: self.position,
        }
    }
}

#[cfg(feature = "std")]
mod iter {
    use std::io::{Bytes, Read};

    use crate::{SampleSongNotes, SongMetadata, SongNote, SongNoteReader};

    /// Yield one note at a time from a byte stream
    pub struct ReadSongNotes<R: Read> {
        inner: Bytes<R>,
        song_plan_generator: SongNoteReader,
        eof: bool,
    }

    impl<R: Read> ReadSongNotes<R> {
        // Read in chunks of 1024 bytes
        const DEFAULT_CAPACITY: usize = 1024;

        pub fn new(metadata: SongMetadata, inner: R) -> Self {
            Self {
                inner: inner.bytes(),
                song_plan_generator: SongNoteReader::new(metadata)
                    .with_capacity(Self::DEFAULT_CAPACITY),
                eof: false,
            }
        }

        pub fn with_capacity(self, capacity: usize) -> Self {
            Self {
                song_plan_generator: self.song_plan_generator.with_capacity(capacity),
                ..self
            }
        }

        pub fn samples(self) -> SampleSongNotes<Self> {
            SampleSongNotes::new(self)
        }
    }

    impl<R: Read> Iterator for ReadSongNotes<R> {
        type Item = std::io::Result<SongNote>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                if !self.eof && self.song_plan_generator.is_empty() {
                    // Fill the byte buffer if empty
                    for _ in 0..self.song_plan_generator.capacity() {
                        match self.inner.next() {
                            None => {
                                self.eof = true;
                                break;
                            }
                            Some(Err(e)) => return Some(Err(e)),
                            Some(Ok(b)) => self.song_plan_generator.push(b),
                        }
                    }
                }

                match self.song_plan_generator.read_note() {
                    // If EOF and None, then there is no remaining notes
                    None if self.eof => return None,
                    // If not EOF and None, loop to fill the byte buffer
                    None => {}
                    // Note generated
                    Some(n) => return Some(Ok(n)),
                }
            }
        }
    }
}

#[cfg(feature = "std")]
pub use iter::*;
