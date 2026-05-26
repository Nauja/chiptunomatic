use crate::plugin::{Plugin, String, Vec};
use crate::synth::{fm_sine, karplus_strong, midi_to_hz, sine, vibrato_sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumPattern, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const PERSIAN_BPM_BASE: i32 = 76;
pub const PERSIAN_BPM_VARIATION: i32 = 16; // 68–84 BPM — slow, contemplative

/// Pentatonic selection from the Persian heptatonic scale.
/// The 3-semitone augmented-second jump from b2 (index 1) to M3 (index 2)
/// is the interval that defines Middle-Eastern and Persian tonality.
pub const PERSIAN_SCALE: [u8; 5] = [0, 1, 4, 5, 7]; // root, b2, M3, P4, P5

/// Drone-centred progressions — Persian classical (dastgah) music gravitates
/// strongly back to the root after each movement away from it.
pub const PERSIAN_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 0, 2, 0], // i – i – M3 – i   (drone with a single colour note)
    [0, 1, 0, 2], // i – b2 – i – M3  (augmented-second stress)
    [0, 3, 0, 3], // i – P4 – i – P4  (suspended fourth, very common in Shur)
    [0, 2, 0, 0], // i – M3 – i – i   (minimal movement, mostly root)
];

/// Tombak "dom" patterns — sparse low open strokes on the main beats.
/// Persian classical percussion is extremely restrained; many bars have 2-3 hits.
pub const PERSIAN_KICK_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1 and 3 only
    [1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0], // 1 and &2
    [1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0], // 1, &2(late), &4
];

/// Tombak "tak" patterns — closed mid strokes, sparse counterweight to dom.
pub const PERSIAN_SNARE_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // tak on beats 2 and 4
    [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0], // &1 and &3
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0], // sparse off-beats
];

/// Finger-cymbal (zill) patterns — single accent per bar, almost subliminal.
pub const PERSIAN_HAT_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beat 3 only
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // beats 1 and 4
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // beat 2 only
];

/// Persian / ancient Middle-Eastern mode.
///
/// Instruments (informed by spectral analysis of reference material):
///   Melody  — santoor / tar (two slightly-detuned KS strings + additive H1–H4,
///             with H4 boosted to match the measured harmonic profile)
///   Bass    — oud / harp (bright Karplus-Strong pluck + warm sine reinforcement)
///   Voice   — ney flute (FM 1:1, β=0.35) + fast melismatic vibrato (6.5 Hz,
///             matching the ~7.4 Hz average measured in the reference track)
///   Kick    — tombak "dom" (warm sine ~155 Hz + brief membrane snap noise)
///   Snare   — tombak "tak" (mid-high sine ~620 Hz + dry noise, very short)
///   Hat     — finger cymbals / zills (metallic ring, single accent per bar)
#[derive(Debug, Clone)]
pub struct PersianPlugin {}

impl Plugin for PersianPlugin {
    fn mode(&self) -> &'static str {
        "persian"
    }

    fn mode_string(&self) -> String {
        String::from("persian")
    }

    fn scale(&self) -> &[u8] {
        &PERSIAN_SCALE
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % PERSIAN_CHORD_PROGRESSIONS.len();
        PERSIAN_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation =
            ((seed >> 8) % PERSIAN_BPM_VARIATION as u32) as i32 - PERSIAN_BPM_VARIATION / 2;
        PERSIAN_BPM_BASE + variation // 68–84
    }

    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        let kick = PERSIAN_KICK_PATTERNS[(seed[0] % 3) as usize];
        let snare = PERSIAN_SNARE_PATTERNS[(seed[1] % 3) as usize];
        let hat = PERSIAN_HAT_PATTERNS[(seed[2] % 3) as usize];

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
        // Occasional open-cymbal accent on beat 3 (step 8).
        if seed[3] & 0x01 != 0 {
            steps[8].hat = true;
            steps[8].open_hat = true;
        }

        DrumPattern { steps }
    }

    /// Santoor / tar lead.
    ///
    /// Two slightly-detuned Karplus-Strong strings (chorus shimmer) carry the
    /// attack transient and natural decay. An additive layer reinforces H1–H4
    /// with H4 boosted relative to H2/H3, matching the measured harmonic profile
    /// of the reference recording (H4 ≈ 0.25–0.38 × H1, H2/H3 each ≈ 0.12–0.18 × H1).
    /// Attack is ~10 ms (hammer or plectrum) with a slow full decay — no sustain plateau.
    /// A quiet lyre KS pluck on the harmony note adds ensemble shimmer.
    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        let hz = midi_to_hz(note.midi);

        // Two detuned strings — subtle chorus replicates the multi-course tuning of santoor.
        let str1 = karplus_strong(sr, hz * 0.9992, dur, 0.24);
        let str2 = karplus_strong(sr, hz * 1.0008, dur, 0.24);
        let str1 = envelope(sr, &str1, 0.010, 0.0, 1.0, 0.050);
        let str2 = envelope(sr, &str2, 0.010, 0.0, 1.0, 0.050);

        // Additive harmonic colouring: H4 is intentionally louder than H2/H3.
        let h1 = sine(sr, hz, dur, 0.14);
        let h2 = sine(sr, hz * 2.0, dur, 0.05);
        let h3 = sine(sr, hz * 3.0, dur, 0.04);
        let h4 = sine(sr, hz * 4.0, dur, 0.09);
        let h1 = envelope(sr, &h1, 0.012, 0.065, 0.48, 0.095);
        let h2 = envelope(sr, &h2, 0.012, 0.080, 0.28, 0.075);
        let h3 = envelope(sr, &h3, 0.012, 0.095, 0.16, 0.060);
        let h4 = envelope(sr, &h4, 0.012, 0.115, 0.08, 0.055);

        // Lyre pluck on harmony note — quiet, fast-decaying shimmer.
        let lyre = karplus_strong(sr, midi_to_hz(note.harmony_midi), dur, 0.07);
        let lyre = envelope(sr, &lyre, 0.003, 0.0, 1.0, 0.030);

        (0..total)
            .map(|i| StemSample {
                value: str1.get(i).copied().unwrap_or(0.0)
                    + str2.get(i).copied().unwrap_or(0.0)
                    + h1.get(i).copied().unwrap_or(0.0)
                    + h2.get(i).copied().unwrap_or(0.0)
                    + h3.get(i).copied().unwrap_or(0.0)
                    + h4.get(i).copied().unwrap_or(0.0)
                    + lyre.get(i).copied().unwrap_or(0.0),
                byte_index: note.byte_index,
            })
            .collect()
    }

    /// Oud / harp bass — bright Karplus-Strong pluck with warm sine reinforcement.
    fn sample_bass_note(&self, note: BassNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        let hz = midi_to_hz(note.midi);

        // Oud/harp: bright KS pluck — 2 ms ramp only, no sustain plateau.
        let harp = karplus_strong(sr, hz, dur, 0.42);
        let harp = envelope(sr, &harp, 0.002, 0.0, 1.0, 0.030);

        // Warm oud body — sine that sustains briefly then fades under the pluck.
        let oud = sine(sr, hz, dur, 0.14);
        let oud = envelope(sr, &oud, 0.008, 0.080, 0.24, 0.100);

        (0..total.min(harp.len()))
            .map(|i| StemSample {
                value: harp[i] + oud.get(i).copied().unwrap_or(0.0),
                byte_index: note.byte_index,
            })
            .collect()
    }

    /// Ney flute + melismatic ornament.
    ///
    /// FM 1:1 with β=0.35 (hollow, reedy body) layered with fast melismatic vibrato.
    /// Vibrato rate is 6.5 Hz — the reference track measured ~7.4 Hz average, typical
    /// of a live ney player's expressive ornamental shaking.
    fn sample_voice_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        let hz = midi_to_hz(note.midi);

        // FM ney body: β=0.35 — slightly breathy, hollow fundamental.
        let ney = fm_sine(sr, hz, dur, 0.090, 1.0, 0.35);
        let ney = envelope(sr, &ney, 0.050, 0.060, 0.78, 0.120);

        // Fast melismatic vibrato: 6.5 Hz, ±3.0 % depth, long breath-in.
        let vib = vibrato_sine(sr, hz, dur, 0.068, 6.5, 0.030);
        let vib = envelope(sr, &vib, 0.065, 0.080, 0.70, 0.110);

        (0..total)
            .map(|i| StemSample {
                value: ney.get(i).copied().unwrap_or(0.0) + vib.get(i).copied().unwrap_or(0.0),
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn sample_step(&self, step: &DrumStep, config: SampleStepConfig, samples: &mut Vec<f32>) {
        if step.kick {
            // Tombak "dom" — open low stroke: warm membrane resonance + membrane snap.
            // Fundamental ~155 Hz (tombak goblet body), brief noise for the stick attack.
            let dom_hz = 155.0 + f64::from(config.color % 35);
            let dur_k = (0.085_f64).min(config.step_duration * 0.90);
            let body = sine(config.sample_rate, dom_hz, dur_k, 0.48);
            let snap = noise_burst(
                config.sample_rate,
                config.random,
                (0.018_f64).min(dur_k),
                0.18,
            );
            overlay_samples(
                &envelope(config.sample_rate, &body, 0.001, 0.038, 0.0, 0.040),
                samples,
            );
            overlay_samples(
                &envelope(config.sample_rate, &snap, 0.0, 0.008, 0.0, 0.008),
                samples,
            );
        }
        if step.snare {
            // Tombak "tak" — closed mid stroke: dry, short, higher-pitched membrane snap.
            // No heavy backbeat accent: tombak dynamics are fluid, not quantised like a snare.
            let tak_hz = 615.0 + f64::from(config.color % 90);
            let dur_t = (0.040_f64).min(config.step_duration * 0.55);
            let body = sine(config.sample_rate, tak_hz, dur_t, 0.20);
            let noise = noise_burst(config.sample_rate, config.random, dur_t, 0.14);
            overlay_samples(
                &envelope(config.sample_rate, &body, 0.0, 0.016, 0.0, 0.020),
                samples,
            );
            overlay_samples(
                &envelope(config.sample_rate, &noise, 0.0, 0.010, 0.0, 0.014),
                samples,
            );
        }
        if step.hat {
            // Finger cymbals / zills — rare metallic accent, ring sustains longer when open.
            if step.open_hat {
                let d = (0.180_f64).min(config.step_duration * 3.5);
                let noise = noise_burst(config.sample_rate, config.random, d, 0.12);
                let ring = sine(config.sample_rate, 3000.0, d, 0.08);
                overlay_samples(
                    &envelope(config.sample_rate, &noise, 0.0, 0.020, 0.20, 0.090),
                    samples,
                );
                overlay_samples(
                    &envelope(config.sample_rate, &ring, 0.0, 0.045, 0.12, 0.100),
                    samples,
                );
            } else {
                let d = (0.025_f64).min(config.step_duration * 0.50);
                let noise = noise_burst(config.sample_rate, config.random, d, 0.10);
                let ting = sine(config.sample_rate, 3400.0, d, 0.06);
                overlay_samples(
                    &envelope(config.sample_rate, &noise, 0.0, 0.010, 0.0, 0.008),
                    samples,
                );
                overlay_samples(
                    &envelope(config.sample_rate, &ting, 0.0, 0.012, 0.0, 0.008),
                    samples,
                );
            }
        }
    }
}
