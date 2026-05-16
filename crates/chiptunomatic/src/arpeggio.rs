use alloc::collections::VecDeque;
use alloc::rc::Rc;

use crate::constants::{CHORD_BEATS, HARMONY_BASE, MELODY_BASE};
use crate::plugin::Plugin;
use crate::{MelodyNote, ReadSongNote, SongNote};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpeggioPattern {
    Up,
    Down,
    /// Ping-pong: 0 → N-1 → 1 → N-1 → … (endpoints not doubled).
    UpDown,
}

#[derive(Debug, Clone, Copy)]
pub struct ArpeggioConfig {
    /// How many sub-notes each melody note is split into (clamped to ≥ 1).
    pub subdivisions: u8,
    pub pattern: ArpeggioPattern,
}

/// Wraps [`SongNoteReader`] and expands each [`SongNote::Melody`] into
/// `subdivisions` shorter sub-notes cycling through the active chord tones.
/// [`SongNote::Bass`] passes through unchanged.
pub struct ArpeggioNoteReader<RSN: ReadSongNote> {
    inner: RSN,
    config: ArpeggioConfig,
    pending: VecDeque<SongNote>,
    melody_elapsed: f64,
}

impl<RSN: ReadSongNote> ArpeggioNoteReader<RSN> {
    pub fn new(inner: RSN, config: ArpeggioConfig) -> Self {
        Self {
            inner,
            config,
            pending: VecDeque::new(),
            melody_elapsed: 0.0,
        }
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

    fn expand(&mut self, note: MelodyNote) {
        let n = self.config.subdivisions.max(1) as usize;
        let sub_dur = note.duration / n as f64;

        let metadata = self.inner.metadata();
        let scale = self.inner.plugin().scale();
        let chord_idx = (self.melody_elapsed / metadata.timing.beat_duration / CHORD_BEATS as f64)
            as usize
            % metadata.chord_progression.len();
        let chord_deg = metadata.chord_progression[chord_idx];

        self.melody_elapsed += note.duration;

        // Cycle through 3 consecutive scale degrees starting at the chord root,
        // clamped to the scale length.
        let num_tones = 3usize.min(scale.len());

        for i in 0..n {
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

            self.pending.push_back(SongNote::Melody(MelodyNote {
                midi,
                harmony_midi,
                duration: sub_dur,
                degree: deg,
                octave: note.octave,
                byte_index: note.byte_index,
            }));
        }
    }
}

impl<RSN: ReadSongNote> ReadSongNote for ArpeggioNoteReader<RSN> {
    fn with_capacity(self, capacity: usize) -> Self {
        Self {
            inner: self.inner.with_capacity(capacity),
            ..self
        }
    }

    fn with_buffer(self, buffer: &[u8]) -> Self {
        Self {
            inner: self.inner.with_buffer(buffer),
            ..self
        }
    }

    fn metadata(&self) -> &crate::SongMetadata {
        self.inner.metadata()
    }

    fn plugin(&self) -> &Rc<dyn Plugin> {
        self.inner.plugin()
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn position(&self) -> u64 {
        self.inner.position()
    }

    fn melody_time(&self) -> f64 {
        self.inner.melody_time()
    }

    fn bass_time(&self) -> f64 {
        self.inner.bass_time()
    }

    fn push(&mut self, byte: u8) {
        self.inner.push(byte);
    }

    fn extend(&mut self, bytes: &[u8]) {
        self.inner.extend(bytes);
    }

    fn read_note(&mut self) -> Option<SongNote> {
        if let Some(note) = self.pending.pop_front() {
            return Some(note);
        }
        let note = self.inner.read_note()?;
        match note {
            SongNote::Bass(_) => Some(note),
            SongNote::Melody(m) => {
                self.expand(m);
                self.pending.pop_front()
            }
        }
    }

    fn read_notes(&mut self) -> alloc::vec::Vec<SongNote> {
        let mut out = alloc::vec::Vec::new();
        while let Some(n) = self.read_note() {
            out.push(n);
        }
        out
    }

    fn read_notes_buf(&mut self, notes: &mut [SongNote]) -> usize {
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
}
