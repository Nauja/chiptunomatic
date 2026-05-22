use crate::plugin::samba::SAMBA_SCALE;
use crate::plugin::{Plugin, String, Vec};
use crate::synth::{fm_sine, midi_to_hz, sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const TOY_BPM_BASE: i32 = 105;
pub const TOY_BPM_VARIATION: i32 = 20; // (seed >> 8) % 20 → 0..19, centre at 10 → 95–114 BPM

/// Major pentatonic — bright, happy toy scale: root, M2, M3, P5, M6.
pub const TOY_SCALE: [u8; 5] = [0, 2, 4, 7, 9];

/// Simple bright loops on the major pentatonic degrees.
pub const TOY_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 2, 3, 2], // root – M3 – P5 – M3
    [0, 3, 4, 3], // root – P5 – M6 – P5
    [0, 2, 4, 2], // root – M3 – M6 – M3
    [0, 4, 3, 0], // root – M6 – P5 – root
];

/// Use toy synthesis (kalimba Karplus-Strong with octave shimmer, major pentatonic, light toy percussion).
/// Kalimba: FM tine (3:1, β=1.2) with octave shimmer, major pentatonic, light toy percussion, 95–114 BPM
#[derive(Debug, Clone)]
pub struct ToyPlugin {}

impl Plugin for ToyPlugin {
    fn mode(&self) -> &'static str {
        "toy"
    }

    fn mode_string(&self) -> String {
        String::from("toy")
    }

    fn scale(&self) -> &[u8] {
        // Same as samba
        &SAMBA_SCALE
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % TOY_CHORD_PROGRESSIONS.len();
        TOY_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % TOY_BPM_VARIATION as u32) as i32 - TOY_BPM_VARIATION / 2;
        TOY_BPM_BASE + variation // 95–114
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // Kalimba tine: FM with a 3:1 modulator ratio and β=1.2 produces the slight
        // inharmonicity of struck metal — bright without being a bell. Fast 1ms attack,
        // 220ms decay to a 8% sustain so the note has a clear "tink" then fades.
        let mel = fm_sine(sr, midi_to_hz(note.midi), dur, 0.55, 3.0, 1.2);
        let mel = envelope(sr, &mel, 0.001, 0.22, 0.08, 0.10);
        // Octave shimmer: same FM formula one octave up, much shorter decay so it
        // disappears in the first ~100ms and leaves the fundamental to ring alone —
        // exactly how a real kalimba tine behaves.
        let shimmer = fm_sine(sr, midi_to_hz(note.midi + 12.0), dur, 0.28, 3.0, 1.2);
        let shimmer = envelope(sr, &shimmer, 0.001, 0.10, 0.02, 0.06);
        (0..total)
            .map(|i| StemSample {
                value: mel.get(i).copied().unwrap_or(0.0) + shimmer.get(i).copied().unwrap_or(0.0),
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
        // Gentle sine foundation — warm harmonic support without competing with the
        // kalimba tines. Moderate sustain so it holds quietly under the melody.
        let raw = sine(sr, midi_to_hz(note.midi), dur, 0.28);
        let with_env = envelope(sr, &raw, 0.005, 0.12, 0.45, 0.08);
        (0..total.min(with_env.len()))
            .map(|i| StemSample {
                value: with_env[i],
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn sample_step(&self, step: &DrumStep, config: SampleStepConfig, samples: &mut Vec<f32>) {
        if step.kick {
            // Light toy thump: soft pitched sine, gentle and airy
            let freq = 100.0 + f64::from(config.color % 20);
            let dur_sec = (0.08_f64).min(config.step_duration * 1.5);
            let raw = sine(config.sample_rate, freq, dur_sec, 0.22);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.002, 0.06, 0.0, 0.02),
                samples,
            );
        }
        if step.snare {
            // Very soft tap — barely a percussion hit
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.10 } else { 0.05 };
            let dur_n = (0.05_f64).min(config.step_duration);
            let noise = noise_burst(config.sample_rate, config.random, dur_n, amp);
            let tone = sine(config.sample_rate, 150.0, dur_n, amp * 0.35);
            let mixed: alloc::vec::Vec<f32> = noise
                .iter()
                .zip(tone.iter())
                .map(|(&n, &t)| n + t)
                .collect();
            overlay_samples(
                &envelope(config.sample_rate, &mixed, 0.001, 0.04, 0.0, 0.015),
                samples,
            );
        }
        if step.hat {
            // Barely-there shimmer — tiny toy tick
            overlay_samples(
                &if step.open_hat {
                    let d = (0.04_f64).min(config.step_duration);
                    let s = noise_burst(config.sample_rate, config.random, d, 0.05);
                    envelope(config.sample_rate, &s, 0.001, 0.02, 0.10, 0.02)
                } else {
                    let amp = if config.pattern % 2 == 0 { 0.04 } else { 0.02 };
                    let d = (0.010_f64).min(config.step_duration * 0.25);
                    noise_burst(config.sample_rate, config.random, d, amp)
                },
                samples,
            );
        }
    }
}
