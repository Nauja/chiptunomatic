use crate::plugin::{Plugin, String, Vec};
use crate::synth::{karplus_strong, midi_to_hz, pitch_sweep_sine, sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumPattern, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

// Taiko kick: sparse downbeat patterns — beats 1+3, beats 1+&2+3, beats 1+2+3+&4
const KOTO_KICK_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1 + 3
    [1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1, &2, 3
    [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0], // beats 1, 2, 3, &4
];

// Hand percussion: sparse accents — 2+4, beat 3 only, beat 2 only
const KOTO_SNARE_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // beats 2 + 4
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beat 3 only
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // beat 2 only
];

// Kane bell: quarter notes at most — no 8th-note subdivisions
const KOTO_HAT_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0], // quarter notes
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1 + 3 only
    [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0], // off-beats (&1 &2 &3 &4)
];

pub const KOTO_BPM_BASE: i32 = 107;
pub const KOTO_BPM_VARIATION: i32 = 24; // (seed >> 8) % 24 → 0..23, centre at 12 → 95–119 BPM

/// Yo pentatonic — the bright Japanese folk/festival scale: root, M2, P4, P5, M6.
/// Intervals W W+H H W give an open, celebratory sound, distinct from the darker Hirajoshi.
pub const KOTO_SCALE: [u8; 5] = [0, 2, 5, 7, 9];

/// Circular I–IV–V progressions characteristic of matsuri/folk koto.
pub const KOTO_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 2, 3, 2], // root – P4 – P5 – P4
    [0, 3, 2, 0], // root – P5 – P4 – root
    [0, 2, 3, 4], // root – P4 – P5 – M6
    [0, 3, 4, 3], // root – P5 – M6 – P5
];

/// Karplus-Strong koto strings, Yo pentatonic scale, taiko + kane percussion, 95–119 BPM
#[derive(Debug, Clone)]
pub struct KotoPlugin {}

impl Plugin for KotoPlugin {
    fn mode(&self) -> &'static str {
        "koto"
    }

    fn mode_string(&self) -> String {
        String::from("koto")
    }

    fn scale(&self) -> &[u8] {
        &KOTO_SCALE
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % KOTO_CHORD_PROGRESSIONS.len();
        KOTO_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % KOTO_BPM_VARIATION as u32) as i32 - KOTO_BPM_VARIATION / 2;
        KOTO_BPM_BASE + variation // 95–119
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // Karplus-Strong plucked koto string at the natural register (no octave shift).
        // A faint octave partial (2× frequency, 0.08 amp) adds the upper-harmonic brightness
        // characteristic of high-tension koto strings and festival playing style.
        let mel = karplus_strong(sr, midi_to_hz(note.midi), dur, 0.20);
        let mel_oct = karplus_strong(sr, midi_to_hz(note.midi) * 2.0, dur, 0.04);
        let harm = karplus_strong(sr, midi_to_hz(note.harmony_midi), dur, 0.10);
        let fade_len = ((sr * 0.010) as usize).min(total);
        (0..total)
            .map(|i| {
                let fade = if i >= total - fade_len {
                    (total - i) as f32 / fade_len as f32
                } else {
                    1.0
                };
                StemSample {
                    value: (mel.get(i).copied().unwrap_or(0.0)
                        + mel_oct.get(i).copied().unwrap_or(0.0)
                        + harm.get(i).copied().unwrap_or(0.0))
                        * fade,
                    byte_index: note.byte_index,
                }
            })
            .collect()
    }

    fn sample_bass_note(&self, note: BassNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        let raw = karplus_strong(sr, midi_to_hz(note.midi), dur, 0.22);
        let fade_len = ((sr * 0.010) as usize).min(total);
        (0..total.min(raw.len()))
            .map(|i| {
                let fade = if i >= total - fade_len {
                    (total - i) as f32 / fade_len as f32
                } else {
                    1.0
                };
                StemSample {
                    value: raw[i] * fade,
                    byte_index: note.byte_index,
                }
            })
            .collect()
    }

    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        let kick = KOTO_KICK_PATTERNS[(seed[0] % 3) as usize];
        let snare = KOTO_SNARE_PATTERNS[(seed[1] % 3) as usize];
        let hat = KOTO_HAT_PATTERNS[(seed[2] % 3) as usize];
        let mut steps = [DrumStep::default(); 16];
        for offset in 0..16 {
            steps[offset] = DrumStep {
                kick: kick[offset] == 1,
                snare: snare[offset] == 1,
                hat: hat[offset] == 1,
                open_hat: false,
                offset,
            };
        }
        // Seed bit: let beat 3 (step 8) ring as an open kane bell when a hat falls there
        if seed[5] & 0x01 != 0 && steps[8].hat {
            steps[8].open_hat = true;
        }
        DrumPattern { steps }
    }

    fn sample_step(&self, step: &DrumStep, config: SampleStepConfig, samples: &mut Vec<f32>) {
        if step.kick {
            // Taiko: pitch sweep from ~1.4× the fundamental down to the target (bachi impact
            // starts bright, drum head settles to its resonant pitch). Longer resonant tail.
            let freq = 78.0 + f64::from(config.color % 18);
            let dur_sec = (0.35_f64).min(config.step_duration * 3.0);
            let raw = pitch_sweep_sine(config.sample_rate, freq * 1.4, freq, dur_sec, 0.50, 18.0);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.0, 0.18, 0.0, 0.06),
                samples,
            );
            // Sub-bass layer for physical weight
            let sub = sine(config.sample_rate, 50.0, dur_sec * 0.7, 0.22);
            overlay_samples(
                &envelope(config.sample_rate, &sub, 0.0, 0.15, 0.0, 0.05),
                samples,
            );
            // Short noise transient — bachi stick impact on the drum head
            let stick = noise_burst(config.sample_rate, config.random, 0.012, 0.22);
            overlay_samples(
                &envelope(config.sample_rate, &stick, 0.0, 0.010, 0.0, 0.002),
                samples,
            );
        }
        if step.snare {
            // Light hand percussion — restrained, keeps the koto strings in focus
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.12 } else { 0.06 };
            let dur_n = (0.06_f64).min(config.step_duration);
            let noise = noise_burst(config.sample_rate, config.random, dur_n, amp);
            let tone = sine(config.sample_rate, 200.0, dur_n, amp * 0.30);
            let mixed: alloc::vec::Vec<f32> = noise
                .iter()
                .zip(tone.iter())
                .map(|(&n, &t)| n + t)
                .collect();
            overlay_samples(
                &envelope(config.sample_rate, &mixed, 0.001, 0.04, 0.0, 0.02),
                samples,
            );
        }
        if step.hat {
            // Kane bell: two sines at an inharmonic ratio (1.0 : 2.4) produce the
            // struck-metal ring of a small Japanese bell rather than plain noise.
            let base = 1250.0 + f64::from(config.color % 5) * 50.0;
            let (dur_h, sustain) = if step.open_hat {
                ((0.08_f64).min(config.step_duration * 2.0), 0.15_f32)
            } else {
                ((0.025_f64).min(config.step_duration * 0.6), 0.0_f32)
            };
            let s1 = sine(config.sample_rate, base, dur_h, 0.12);
            let s2 = sine(config.sample_rate, base * 2.4, dur_h, 0.06);
            let mixed: alloc::vec::Vec<f32> =
                s1.iter().zip(s2.iter()).map(|(&a, &b)| a + b).collect();
            overlay_samples(
                &envelope(config.sample_rate, &mixed, 0.0, 0.008, sustain, 0.015),
                samples,
            );
        }
    }
}
