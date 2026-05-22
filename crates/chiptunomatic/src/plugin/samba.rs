use crate::plugin::{Plugin, String, Vec};
use crate::synth::{fm_sine, midi_to_hz, pitch_sweep_sine, sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumPattern, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const SAMBA_BPM_BASE: i32 = 108;
pub const SAMBA_BPM_VARIATION: i32 = 16; // (seed >> 8) % 16 → 0..15, centre at 8 → 100–115 BPM

/// Major pentatonic — bright, upbeat samba scale: root, M2, M3, P5, M6.
pub const SAMBA_SCALE: [u8; 5] = [0, 2, 4, 7, 9];

/// Circular, driving loops built on the major pentatonic for samba.
/// Degree 3 (index → P5 semitone) is accurate in both PENTATONIC_MINOR and SAMBA_SCALE.
pub const SAMBA_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 3, 2, 3], // root – P5 – M3 – P5  (I–V loop)
    [0, 2, 0, 3], // root – M3 – root – P5
    [0, 3, 4, 3], // root – P5 – M6 – P5
    [0, 2, 3, 0], // root – M3 – P5 – root
];

/// Surdo patterns: the contratempos fall on beats 2 and 4 (steps 4 and 12).
/// This beat placement — not beats 1 and 3 — is the primary rhythmic identity of samba.
pub const SAMBA_KICK_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // marcação: beats 2 + 4
    [1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // beat 1 + 2 + 4
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0], // beats 2, &3, 4 (cortador touch)
];

/// Caixa patterns: dense driving 16th-note textures.
pub const SAMBA_SNARE_PATTERNS: [[u8; 16]; 3] = [
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1], // all 16th notes
    [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0], // all 8th notes
    [1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1], // repinique syncopation
];

/// Tamborim patterns: teleco-teco and cruzado syncopated figures.
pub const SAMBA_HAT_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 1, 0, 1, 0], // teleco-teco (dense)
    [1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0], // teleco-teco (sparse)
    [1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0], // cruzado (3+3+2)
];

#[derive(Debug, Clone)]
pub struct SambaPlugin {}

impl Plugin for SambaPlugin {
    fn mode(&self) -> &'static str {
        "samba"
    }

    fn mode_string(&self) -> String {
        String::from("samba")
    }

    fn scale(&self) -> &[u8] {
        &SAMBA_SCALE
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % SAMBA_CHORD_PROGRESSIONS.len();
        SAMBA_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % SAMBA_BPM_VARIATION as u32) as i32 - SAMBA_BPM_VARIATION / 2;
        SAMBA_BPM_BASE + variation // 100–115
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        // Bright, articulated melody: FM 2:1, β=0.6 — crisp and reedy, sits clearly
        // above the drums without going metallic. Punchy envelope: fast attack,
        // short decay to a modest sustain so notes articulate crisply.
        let midi = (note.midi - 12.0).max(21.0);
        let harmony_midi = (note.harmony_midi - 12.0).max(21.0);
        let mel = fm_sine(sr, midi_to_hz(midi), dur, 0.38, 2.0, 0.6);
        let mel = envelope(sr, &mel, 0.002, 0.15, 0.25, 0.06);
        let harm = fm_sine(sr, midi_to_hz(harmony_midi), dur, 0.16, 2.0, 0.6);
        let harm = envelope(sr, &harm, 0.002, 0.15, 0.25, 0.06);
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
        // Short, punchy sine bass — fast decay to 15 % sustain mimics a plucked
        // bass guitar string. The quick cutoff keeps it rhythmically tight so it
        // locks in with the surdo rather than blurring into a held tone.
        let raw = sine(sr, midi_to_hz(note.midi), dur, 0.42);
        let with_env = envelope(sr, &raw, 0.002, 0.07, 0.15, 0.04);
        (0..total.min(with_env.len()))
            .map(|i| StemSample {
                value: with_env[i],
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        let kick = SAMBA_KICK_PATTERNS[(seed[0] % 3) as usize];
        let snare = SAMBA_SNARE_PATTERNS[(seed[1] % 3) as usize];
        let hat = SAMBA_HAT_PATTERNS[(seed[2] % 3) as usize];

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
        DrumPattern { steps }
    }

    fn sample_step(&self, step: &DrumStep, config: SampleStepConfig, samples: &mut Vec<f32>) {
        if step.kick {
            // Surdo: pitch sweep from ~1.5× the target frequency down to the fundamental
            // gives the boom-and-settle impact of a large bass drum head.  The longer
            // 280 ms tail lets it breathe between the contratempos on beats 2 and 4.
            let freq = 60.0 + f64::from(config.color % 15);
            let dur_sec = (0.28_f64).min(config.step_duration * 4.0);
            let raw = pitch_sweep_sine(config.sample_rate, freq * 1.5, freq, dur_sec, 0.60, 20.0);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.002, 0.22, 0.0, 0.05),
                samples,
            );
        }
        if step.snare {
            // Caixa: accent on 8th-note positions (even steps) and quieter on the
            // 16th-note offbeats — matches the natural stress pattern of the caixa
            // whether the pattern is all-16ths, all-8ths, or syncopated.
            let accent = config.pattern % 2 == 0;
            let amp: f32 = if accent { 0.32 } else { 0.18 };
            let dur_n = (0.06_f64).min(config.step_duration);
            let noise = noise_burst(config.sample_rate, config.random, dur_n, amp);
            let tone = sine(config.sample_rate, 250.0, dur_n, amp * 0.25);
            let mixed: Vec<f32> = noise
                .iter()
                .zip(tone.iter())
                .map(|(&n, &t)| n + t)
                .collect();
            overlay_samples(
                &envelope(config.sample_rate, &mixed, 0.0, 0.03, 0.0, 0.015),
                samples,
            );
        }
        if step.hat {
            // Tamborim: noise + brief high "tok" sine (1 400–1 700 Hz) for the
            // wood-frame-and-metal-head timbre.  The syncopated pattern tables
            // provide the samba groove; the synthesis just needs to be short and bright.
            let amp: f32 = if config.pattern % 2 == 0 { 0.15 } else { 0.10 };
            let d = (0.015_f64).min(config.step_duration * 0.35);
            let noise = noise_burst(config.sample_rate, config.random, d, amp);
            let tok_hz = 1400.0 + f64::from(config.color % 10) * 30.0;
            let tok = sine(config.sample_rate, tok_hz, d, amp * 0.50);
            let mixed: Vec<f32> = noise.iter().zip(tok.iter()).map(|(&n, &t)| n + t).collect();
            overlay_samples(
                &envelope(config.sample_rate, &mixed, 0.001, d * 0.6, 0.0, d * 0.3),
                samples,
            );
        }
    }
}
