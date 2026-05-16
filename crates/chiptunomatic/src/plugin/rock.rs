use crate::plugin::{Plugin, String, Vec};
use crate::synth::{midi_to_hz, sine, triangle};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst, square},
    DrumSample, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const ROCK_BPM_BASE: i32 = 130;
pub const ROCK_BPM_VARIATION: i32 = 30; // (seed >> 8) % 30 → 0..29, centre at 15 → 115–144 BPM

/// Driving 4-chord loops for rock mode. Classic I–V–IV and variations.
pub const ROCK_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 3, 2, 3], // I – V – IV – V  (classic rock)
    [0, 2, 3, 2], // I – IV – V – IV (blues-rock turnaround)
    [0, 3, 4, 3], // I – V – m7 – V
    [0, 2, 4, 2], // I – IV – m7 – IV
];

/// Use rock synthesis (power chords, distortion, faster BPM).
/// Power-chord square waves with soft-clip distortion, 115–145 BPM
#[derive(Debug, Clone)]
pub struct RockPlugin {}

impl Plugin for RockPlugin {
    fn mode(&self) -> &'static str {
        "rock"
    }

    fn mode_string(&self) -> String {
        String::from("rock")
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % ROCK_CHORD_PROGRESSIONS.len();
        ROCK_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % ROCK_BPM_VARIATION as u32) as i32 - ROCK_BPM_VARIATION / 2;
        ROCK_BPM_BASE + variation // 115–145
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // Power chord: root + perfect fifth (7 semitones), soft-clipped for grit
        let root = square(sr, midi_to_hz(note.midi), dur, 0.30, 0.5);
        let fifth = square(sr, midi_to_hz(note.midi + 7.0), dur, 0.20, 0.5);
        let root = envelope(sr, &root, 0.002, 0.05, 0.80, 0.04);
        let fifth = envelope(sr, &fifth, 0.002, 0.05, 0.80, 0.04);
        (0..total)
            .map(|i| {
                let s = root.get(i).copied().unwrap_or(0.0) + fifth.get(i).copied().unwrap_or(0.0);
                // Soft saturation: x / (1 + |x|)
                StemSample {
                    value: s / (1.0 + s.abs()),
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
        // Overdriven triangle bass — punchy with harmonic grit
        let raw = triangle(sr, midi_to_hz(note.midi), dur, 0.30);
        let driven: Vec<f32> = raw
            .iter()
            .map(|&s| {
                let d = s * 1.5;
                d / (1.0 + d.abs())
            })
            .collect();
        let with_env = envelope(sr, &driven, 0.002, 0.04, 0.60, 0.04);
        (0..total.min(with_env.len()))
            .map(|i| StemSample {
                value: with_env[i],
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn sample_step(&self, step: DrumStep, config: SampleStepConfig, samples: &mut Vec<DrumSample>) {
        if step.kick {
            // Heavy kick: square thud + sine sub-layer for body
            let freq = 65.0 + f64::from(config.color % 10);
            let dur_sec = (0.14_f64).min(config.step_duration * 2.0);
            let raw = square(config.sample_rate, freq, dur_sec, 0.55, 0.3);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.001, 0.06, 0.0, 0.015),
                samples,
            );
            let sub = sine(config.sample_rate, 50.0, dur_sec, 0.30);
            overlay_samples(
                &envelope(config.sample_rate, &sub, 0.001, 0.09, 0.0, 0.02),
                samples,
            );
        }
        if step.snare {
            // Cracking snare: loud noise burst, body tone on backbeats
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.35 } else { 0.15 };
            let dur_n = (0.09_f64).min(config.step_duration);
            let raw = noise_burst(config.sample_rate, &config.random, dur_n, amp);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.001, 0.04, 0.0, 0.02),
                samples,
            );
            if accent {
                let tone = sine(config.sample_rate, 200.0, dur_n, 0.15);
                overlay_samples(
                    &envelope(config.sample_rate, &tone, 0.001, 0.03, 0.0, 0.015),
                    samples,
                );
            }
        }
        if step.hat {
            // Crisp, tight hi-hat — present but not dominating
            overlay_samples(
                &if step.open_hat {
                    let d = (0.12_f64).min(config.step_duration * 3.0);
                    let s = noise_burst(config.sample_rate, &config.random, d, 0.18);
                    envelope(config.sample_rate, &s, 0.001, 0.06, 0.15, 0.04)
                } else {
                    let amp = if config.pattern % 2 == 0 { 0.14 } else { 0.08 };
                    let d = (0.022_f64).min(config.step_duration * 0.5);
                    noise_burst(config.sample_rate, &config.random, d, amp)
                },
                samples,
            );
        }
    }
}
