use crate::plugin::{Plugin, String, Vec};
use crate::synth::{fm_sine, midi_to_hz, sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst, square},
    DrumPattern, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const RAP_BPM_BASE: i32 = 92;
pub const RAP_BPM_VARIATION: i32 = 16; // (seed >> 8) % 16 → 0..15, centre at 8 → 84–99 BPM

/// Boom-bap kick: the "boom" on beat 1 is fixed; the second hit lands on an
/// off-beat rather than squarely on beat 3 for that swaggering, loose feel.
pub const RAP_KICK_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0], // 1, 3, &3
    [1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0], // 1, &2, 3
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0], // 1, &3, 4
];

/// Boom-bap snare: backbeat on beats 2 and 4; ghost notes at off-beat positions
/// play at half amplitude (accent check in sample_step uses pattern == 4 || 12).
pub const RAP_SNARE_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // clean 2 + 4
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0], // 2, ghost &3, 4
    [0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0], // 2, ghost &2, 4
];

/// Boom-bap hi-hat: sparse and unhurried — quarter notes, sparse 8ths, or straight 8ths.
pub const RAP_HAT_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0], // quarter notes only
    [1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0], // sparse 8ths
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0], // straight 8ths
];

/// Soul/funk-influenced loops for rap mode — minor-feel movement without wandering far from root.
pub const RAP_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 2, 0, 3], // i – IV – i – V  (classic boom-bap loop)
    [0, 3, 2, 1], // i – V – IV – m3
    [0, 1, 2, 1], // i – m3 – IV – m3
    [0, 2, 3, 0], // i – IV – V – i
];

/// Use rap synthesis (boom-bap drums, warm FM piano, punchy melodic bass).
/// Boom-bap kick/snare, warm FM piano melody, punchy melodic bass, 84–100 BPM
#[derive(Debug, Clone)]
pub struct RapPlugin {}

impl Plugin for RapPlugin {
    fn mode(&self) -> &'static str {
        "rap"
    }

    fn mode_string(&self) -> String {
        String::from("rap")
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % RAP_CHORD_PROGRESSIONS.len();
        RAP_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % RAP_BPM_VARIATION as u32) as i32 - RAP_BPM_VARIATION / 2;
        RAP_BPM_BASE + variation // 84–99
    }

    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        let kick = RAP_KICK_PATTERNS[(seed[0] % 3) as usize];
        let snare = RAP_SNARE_PATTERNS[(seed[1] % 3) as usize];
        let hat = RAP_HAT_PATTERNS[(seed[2] % 3) as usize];

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
        // Open hat on beat 3 (step 8) adds a breath between the backbeats
        if seed[5] & 0x01 != 0 {
            steps[8].hat = true;
            steps[8].open_hat = true;
        }

        DrumPattern { steps }
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // Warm FM piano/Rhodes — mod ratio 1.5, index 1.0 for mid-range presence
        let midi = (note.midi - 12.0).max(21.0);
        let harmony_midi = (note.harmony_midi - 12.0).max(21.0);
        let mel = fm_sine(sr, midi_to_hz(midi), dur, 0.28, 1.5, 1.0);
        let mel = envelope(sr, &mel, 0.004, 0.18, 0.45, 0.08);
        let harm = fm_sine(sr, midi_to_hz(harmony_midi), dur, 0.12, 1.5, 1.0);
        let harm = envelope(sr, &harm, 0.004, 0.18, 0.45, 0.08);
        (0..total)
            .map(|i| StemSample {
                value: mel.get(i).copied().unwrap_or(0.0) + harm.get(i).copied().unwrap_or(0.0),
                byte_index: note.byte_index,
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
        // Clean punchy sine bass — melodic and present, no pitch sweep
        let raw = sine(sr, midi_to_hz(note.midi), dur, 0.45);
        let with_env = envelope(sr, &raw, 0.005, 0.07, 0.60, 0.06);
        (0..total.min(with_env.len()))
            .map(|i| StemSample {
                value: with_env[i],
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn sample_step(&self, step: &DrumStep, config: SampleStepConfig, samples: &mut Vec<f32>) {
        if step.kick {
            // Boom: square thud at ~78 Hz + sine sub, tight decay
            let freq = 72.0 + f64::from(config.color % 15);
            let dur_sec = (0.13_f64).min(config.step_duration * 2.0);
            let thud = square(config.sample_rate, freq, dur_sec, 0.50, 0.25);
            overlay_samples(
                &envelope(config.sample_rate, &thud, 0.001, 0.08, 0.0, 0.02),
                samples,
            );
            let sub = sine(config.sample_rate, 52.0, dur_sec, 0.25);
            overlay_samples(
                &envelope(config.sample_rate, &sub, 0.001, 0.10, 0.0, 0.02),
                samples,
            );
        }
        if step.snare {
            // Bap: noise burst + 220 Hz pitched body for the crack
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.30 } else { 0.12 };
            let dur_n = (0.08_f64).min(config.step_duration);
            let noise = noise_burst(config.sample_rate, config.random, dur_n, amp);
            let tone = sine(config.sample_rate, 220.0, dur_n, amp * 0.50);
            let mixed: alloc::vec::Vec<f32> = noise
                .iter()
                .zip(tone.iter())
                .map(|(&n, &t)| n + t)
                .collect();
            overlay_samples(
                &envelope(config.sample_rate, &mixed, 0.001, 0.05, 0.0, 0.025),
                samples,
            );
        }
        if step.hat {
            // Simple 8th-note hi-hat — moderate, unhurried
            overlay_samples(
                &if step.open_hat {
                    let d = (0.08_f64).min(config.step_duration * 2.0);
                    let s = noise_burst(config.sample_rate, config.random, d, 0.14);
                    envelope(config.sample_rate, &s, 0.001, 0.05, 0.18, 0.03)
                } else {
                    let amp = if config.pattern % 2 == 0 { 0.12 } else { 0.07 };
                    let d = (0.018_f64).min(config.step_duration * 0.42);
                    noise_burst(config.sample_rate, config.random, d, amp)
                },
                samples,
            );
        }
    }
}
