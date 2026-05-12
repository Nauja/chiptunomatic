/// Upper bound on how many [`SongPlanNote`] values [`crate::SongPlanGenerator::generate_chunk`] can emit in one call.
pub const MAX_PLAN_CHUNK_NOTES: usize = 1024;

/// Sample rate (Hz), fixed — matches the Python reference implementation.
pub const SAMPLE_RATE: u32 = 44100;

pub const MELODY_BASE: f64 = 60.0;
pub const BASS_BASE: f64 = 36.0;
pub const HARMONY_BASE: f64 = 48.0;

pub const CHORD_BEATS: usize = 4;

pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub const PENTATONIC_MINOR: [u8; 5] = [0, 3, 5, 7, 10];

pub const DURATIONS: [u8; 6] = [2, 2, 4, 4, 4, 8];

pub const CHORD_PROGRESSIONS: [[usize; 4]; 4] =
    [[0, 3, 2, 3], [0, 4, 3, 4], [0, 2, 3, 2], [0, 1, 4, 2]];

pub const KICK_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
];

pub const SNARE_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0],
    [0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
];

pub const HAT_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
    [1, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
];
