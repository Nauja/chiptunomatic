use crate::plugin::{Plugin, String, Vec};
use crate::synth::{fm_sine, midi_to_hz, pitch_sweep_sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumPattern, DrumSample, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const TRAP_BPM_BASE: i32 = 140;
pub const TRAP_BPM_VARIATION: i32 = 30; // (seed >> 8) % 30 → 0..29, centre at 15 → 125–154 BPM

/// Trap kick: sparse placement to leave space for the 808 bass.
/// Beats 1-only, 1+3, or beat-1 with a syncopated &2 hit.
pub const TRAP_KICK_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // beat 1 only
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1 + 3
    [1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0], // beat 1 + &2 (syncopated)
];

/// Trap hi-hat: rapid 16th-note stutter clusters — the genre's defining rhythmic texture.
pub const TRAP_HAT_PATTERNS: [[u8; 16]; 3] = [
    [1, 1, 1, 0, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 0, 1], // stutter (3+2+3 grouping)
    [1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1], // triplet clusters
    [1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1], // rolling 16ths
];

/// Dark minor loops for trap mode — root, minor 3rd, 5th, minor 7th stay low and ominous.
pub const TRAP_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 1, 0, 4], // i – m3 – i – m7
    [0, 4, 1, 3], // i – m7 – m3 – P5
    [0, 1, 3, 4], // i – m3 – P5 – m7
    [0, 3, 1, 0], // i – P5 – m3 – i
];

/// Use trap synthesis (808 bass, bell melody, trap clap, crisp hi-hats).
/// 808 bass, bell/pluck FM melody, trap clap, crisp hi-hats, 125–155 BPM
#[derive(Debug, Clone)]
pub struct TrapPlugin {}

impl Plugin for TrapPlugin {
    fn mode(&self) -> &'static str {
        "trap"
    }

    fn mode_string(&self) -> String {
        String::from("trap")
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % TRAP_CHORD_PROGRESSIONS.len();
        TRAP_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % TRAP_BPM_VARIATION as u32) as i32 - TRAP_BPM_VARIATION / 2;
        TRAP_BPM_BASE + variation // 125–154
    }

    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        let kick = TRAP_KICK_PATTERNS[(seed[0] % 3) as usize];
        let hat = TRAP_HAT_PATTERNS[(seed[2] % 3) as usize];

        let mut steps = [DrumStep::default(); 16];
        for offset in 0..16 {
            steps[offset] = DrumStep {
                kick: kick[offset] == 1,
                snare: offset == 4 || offset == 12, // clap locked to beats 2 and 4
                hat: hat[offset] == 1,
                open_hat: false,
                offset,
            };
        }
        // Open hat on &2 (step 6) or &4 (step 14)
        let oh = if seed[5] & 0x01 != 0 { 6 } else { 14 };
        steps[oh].hat = true;
        steps[oh].open_hat = true;

        DrumPattern { steps }
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // Bell/pluck: FM with high mod ratio and index for metallic shimmer
        let mel = fm_sine(sr, midi_to_hz(note.midi), dur, 0.25, 4.0, 2.5);
        let mel = envelope(sr, &mel, 0.002, 0.15, 0.10, 0.05);
        let harm = fm_sine(sr, midi_to_hz(note.harmony_midi), dur, 0.10, 4.0, 2.5);
        let harm = envelope(sr, &harm, 0.002, 0.15, 0.10, 0.05);
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
        // 808-style bass: pitch-sweep sine from 2.2× the target note down to the note.
        // The sweep mimics the iconic 808 "wub" — starts bright, settles into the sub.
        let target_hz = midi_to_hz(note.midi);
        let raw = pitch_sweep_sine(sr, target_hz * 2.2, target_hz, dur, 0.60, 12.0);
        let with_env = envelope(sr, &raw, 0.003, 0.20, 0.70, 0.12);
        (0..total.min(with_env.len()))
            .map(|i| StemSample {
                value: with_env[i],
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn sample_step(&self, step: DrumStep, config: SampleStepConfig, samples: &mut Vec<DrumSample>) {
        if step.kick {
            // 808 kick: dramatic pitch sweep across the full step window
            let raw = pitch_sweep_sine(
                config.sample_rate,
                180.0,
                42.0,
                config.step_duration,
                0.80,
                22.0,
            );
            overlay_samples(
                &envelope(
                    config.sample_rate,
                    &raw,
                    0.001,
                    config.step_duration * 0.55,
                    0.0,
                    config.step_duration * 0.35,
                ),
                samples,
            );
        }
        if step.snare {
            // Trap clap: two layered noise bursts, second delayed 8 ms for clap width
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.40 } else { 0.18 };
            let dur_n = (0.07_f64).min(config.step_duration);
            let raw = noise_burst(config.sample_rate, &config.random, dur_n, amp);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.0, 0.030, 0.0, 0.020),
                samples,
            );
            if accent {
                let raw2 = noise_burst(config.sample_rate, &config.random, dur_n * 0.6, amp * 0.7);
                let env2 = envelope(config.sample_rate, &raw2, 0.0, 0.025, 0.0, 0.015);
                let offset = (config.sample_rate * 0.008) as usize;
                for i in 0..env2.len() {
                    let dst = i + offset;
                    if dst < samples.len() {
                        samples[dst] += env2[i];
                    }
                }
            }
        }
        if step.hat {
            // Crisp metallic trap hi-hat
            overlay_samples(
                &if step.open_hat {
                    let d = (0.10_f64).min(config.step_duration * 2.5);
                    let s = noise_burst(config.sample_rate, &config.random, d, 0.20);
                    envelope(config.sample_rate, &s, 0.0, 0.04, 0.25, 0.05)
                } else {
                    let amp = if config.pattern % 2 == 0 { 0.16 } else { 0.10 };
                    let d = (0.020_f64).min(config.step_duration * 0.45);
                    noise_burst(config.sample_rate, &config.random, d, amp)
                },
                samples,
            );
        }
    }
}
