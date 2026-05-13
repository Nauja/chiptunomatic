use crate::plugin::{Plugin, String, Vec};
use crate::synth::{midi_to_hz, sine, square, vibrato_sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumPattern, DrumSample, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const METAL_BPM_BASE: i32 = 158;
pub const METAL_BPM_VARIATION: i32 = 36; // 140–175 BPM

/// Driving dark minor loops — minimal movement, maximum menace.
pub const METAL_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 1, 3, 1], // i – m3 – P5 – m3
    [0, 3, 4, 3], // i – P5 – m7 – P5
    [0, 4, 1, 0], // i – m7 – m3 – i
    [0, 1, 4, 3], // i – m3 – m7 – P5
];

/// Kick: 8th-note double-kick, gallop (1+&+2), or syncopated 16ths.
pub const METAL_KICK_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0], // 8th-note double-kick
    [1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1], // gallop
    [1, 1, 0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 1, 0], // syncopated 16ths
];

/// Snare: backbeat 2+4, with accent variant that adds a hit on &2 or &4.
pub const METAL_SNARE_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // classic 2+4
    [0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0], // 2 + &2 + 4
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0], // 2 + 4 + &4
];

/// Hat: tight 8ths, relentless 16ths, or syncopated 16ths.
pub const METAL_HAT_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0], // 8th notes
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1], // 16th notes
    [1, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0], // syncopated 16ths
];

/// Heavy metal mode: distorted power chords, double-kick patterns, explosive snare.
#[derive(Debug, Clone)]
pub struct MetalPlugin {}

impl Plugin for MetalPlugin {
    fn mode(&self) -> &'static str {
        "metal"
    }

    fn mode_string(&self) -> String {
        String::from("metal")
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % METAL_CHORD_PROGRESSIONS.len();
        METAL_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % METAL_BPM_VARIATION as u32) as i32 - METAL_BPM_VARIATION / 2;
        METAL_BPM_BASE + variation // 140–175
    }

    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        let kick = METAL_KICK_PATTERNS[(seed[0] % 3) as usize];
        let snare = METAL_SNARE_PATTERNS[(seed[1] % 3) as usize];
        let hat = METAL_HAT_PATTERNS[(seed[2] % 3) as usize];

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
        // Breathing open hat on beat 3 (step 8) or &4 (step 14).
        let oh = if seed[3] & 0x01 != 0 { 8 } else { 14 };
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
        // Power chord: root + perfect fifth, hard-clipped for maximum distortion.
        let root = square(sr, midi_to_hz(note.midi), dur, 0.38, 0.5);
        let fifth = square(sr, midi_to_hz(note.midi + 7.0), dur, 0.28, 0.5);
        let root = envelope(sr, &root, 0.001, 0.012, 0.92, 0.015);
        let fifth = envelope(sr, &fifth, 0.001, 0.012, 0.92, 0.015);
        (0..total)
            .map(|i| {
                let s = root.get(i).copied().unwrap_or(0.0) + fifth.get(i).copied().unwrap_or(0.0);
                // Hard clip at ±0.65 for a squared-off, saturated guitar tone.
                StemSample {
                    value: s.clamp(-0.65, 0.65) / 0.65,
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
        // Thick distorted bass — square wave driven hard through a soft saturator.
        let raw = square(sr, midi_to_hz(note.midi), dur, 0.55, 0.5);
        let saturated: Vec<f32> = raw
            .iter()
            .map(|&s| {
                let d = s * 2.8;
                (d / (1.0 + d.abs())) * 0.90
            })
            .collect();
        let with_env = envelope(sr, &saturated, 0.001, 0.018, 0.88, 0.015);
        (0..total.min(with_env.len()))
            .map(|i| StemSample {
                value: with_env[i],
                byte_index: note.byte_index,
            })
            .collect()
    }

    /// Harsh metal vocal: fast wide vibrato (7 Hz, ±4 %) pushed through soft saturation.
    fn sample_voice_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        let raw = vibrato_sine(sr, midi_to_hz(note.midi), dur, 0.14, 7.0, 0.04);
        let shaped = envelope(sr, &raw, 0.004, 0.035, 0.88, 0.040);
        (0..total)
            .map(|i| {
                let s = shaped.get(i).copied().unwrap_or(0.0);
                let d = s * 3.2;
                StemSample {
                    value: d / (1.0 + d.abs()),
                    byte_index: note.byte_index,
                }
            })
            .collect()
    }

    fn sample_step(&self, step: DrumStep, config: SampleStepConfig, samples: &mut Vec<DrumSample>) {
        if step.kick {
            // Heavy double-kick thud: square body + sine sub for low-end weight.
            let freq = 55.0 + f64::from(config.color % 8);
            let dur_sec = (0.065_f64).min(config.step_duration);
            let body = square(config.sample_rate, freq, dur_sec, 0.70, 0.5);
            overlay_samples(
                &envelope(config.sample_rate, &body, 0.001, 0.040, 0.0, 0.010),
                samples,
            );
            let sub = sine(config.sample_rate, 42.0, dur_sec, 0.38);
            overlay_samples(
                &envelope(config.sample_rate, &sub, 0.001, 0.055, 0.0, 0.010),
                samples,
            );
        }
        if step.snare {
            // Explosive snare: maximum-amplitude noise burst + pitched crack on backbeats.
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.52 } else { 0.22 };
            let dur_n = (0.055_f64).min(config.step_duration);
            let raw = noise_burst(config.sample_rate, &config.random, dur_n, amp);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.0, 0.022, 0.0, 0.012),
                samples,
            );
            if accent {
                let tone = sine(config.sample_rate, 220.0, dur_n, 0.22);
                overlay_samples(
                    &envelope(config.sample_rate, &tone, 0.0, 0.018, 0.0, 0.010),
                    samples,
                );
            }
        }
        if step.hat {
            // Crisp, aggressive hi-hat — loud and tight.
            overlay_samples(
                &if step.open_hat {
                    let d = (0.090_f64).min(config.step_duration * 2.0);
                    let s = noise_burst(config.sample_rate, &config.random, d, 0.24);
                    envelope(config.sample_rate, &s, 0.0, 0.035, 0.08, 0.025)
                } else {
                    let d = (0.014_f64).min(config.step_duration * 0.32);
                    noise_burst(config.sample_rate, &config.random, d, 0.22)
                },
                samples,
            );
        }
    }
}
