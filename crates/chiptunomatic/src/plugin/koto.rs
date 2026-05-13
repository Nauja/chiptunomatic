use crate::plugin::{Plugin, String, Vec};
use crate::synth::{karplus_strong, midi_to_hz, sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumSample, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const KOTO_BPM_BASE: i32 = 90;
pub const KOTO_BPM_VARIATION: i32 = 20; // (seed >> 8) % 20 → 0..19, centre at 10 → 80–99 BPM

/// Hirajoshi pentatonic — the defining koto scale: root, M2, m3, P5, m6.
/// Intervals between adjacent degrees: W H WW H, giving the characteristic Japanese sound.
pub const KOTO_SCALE: [u8; 5] = [0, 2, 3, 7, 8];

/// Progressions built from the Hirajoshi degrees to emphasise the scale's characteristic
/// semitone intervals (m3 at degree 2, m6 at degree 4).
pub const KOTO_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 2, 3, 2], // root – m3 – P5 – m3
    [0, 3, 2, 0], // root – P5 – m3 – root
    [0, 2, 0, 3], // root – m3 – root – P5
    [0, 3, 4, 3], // root – P5 – m6 – P5
];

/// Karplus-Strong koto strings, Hirajoshi scale, taiko percussion, 80–99 BPM
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
        KOTO_BPM_BASE + variation // 80–99
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // Karplus-Strong plucked koto string. Natural decay: high notes fade quickly,
        // low notes ring longer — matching real koto string physics.
        let mel = karplus_strong(sr, midi_to_hz(note.midi), dur, 0.42);
        let harm = karplus_strong(sr, midi_to_hz(note.harmony_midi), dur, 0.20);
        // 10 ms linear fade-out to avoid a click at the note boundary.
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
        // Plucked bass koto string — lower pitch means a longer delay line,
        // so this naturally rings longer than the melody strings.
        let raw = karplus_strong(sr, midi_to_hz(note.midi), dur, 0.35);
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

    fn sample_step(&self, step: DrumStep, config: SampleStepConfig, samples: &mut Vec<DrumSample>) {
        if step.kick {
            // Taiko-like deep thump: pitched sine, moderate amplitude
            let freq = 78.0 + f64::from(config.color % 18);
            let dur_sec = (0.11_f64).min(config.step_duration * 1.8);
            let raw = sine(config.sample_rate, freq, dur_sec, 0.32);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.001, 0.08, 0.0, 0.03),
                samples,
            );
        }
        if step.snare {
            // Light hand percussion — very restrained, keeps the koto in focus
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.12 } else { 0.06 };
            let dur_n = (0.06_f64).min(config.step_duration);
            let noise = noise_burst(config.sample_rate, &config.random, dur_n, amp);
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
            // Barely audible shimmer — koto music has no cymbals
            overlay_samples(
                &if step.open_hat {
                    let d = (0.05_f64).min(config.step_duration * 1.2);
                    let s = noise_burst(config.sample_rate, &config.random, d, 0.06);
                    envelope(config.sample_rate, &s, 0.001, 0.03, 0.10, 0.03)
                } else {
                    let amp = if config.pattern % 2 == 0 { 0.05 } else { 0.03 };
                    let d = (0.012_f64).min(config.step_duration * 0.30);
                    noise_burst(config.sample_rate, &config.random, d, amp)
                },
                samples,
            );
        }
    }
}
