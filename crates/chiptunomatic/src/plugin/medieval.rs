use crate::plugin::{Plugin, String, Vec};
use crate::synth::{karplus_strong, midi_to_hz, noise_burst, sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::envelope,
    DrumPattern, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const MEDIEVAL_BPM_BASE: i32 = 90;
pub const MEDIEVAL_BPM_VARIATION: i32 = 20; // centre 90, range 80–100 BPM

/// Dorian pentatonic: root – M2 – P4 – P5 – m7. Open perfect consonances, no minor third.
pub const MEDIEVAL_SCALE: [u8; 5] = [0, 2, 5, 7, 10];

/// Sparse drum patterns — processional feel, 2–4 hits per bar maximum.
/// Quarter-note beat positions in a 16-step grid: 0, 4, 8, 12.
pub const MEDIEVAL_KICK_PATTERNS: [[u8; 16]; 3] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1 + 3
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // beat 1 only (solemn)
    [1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1, &2, 3
];
pub const MEDIEVAL_SNARE_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beat 3 (half-time)
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // beat 2 only
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0], // beat 4 only
];
pub const MEDIEVAL_HAT_PATTERNS: [[u8; 16]; 3] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // no cymbala
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // downbeat only
    [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // beats 1 + 3
];

/// Progressions built entirely on root, P4 (degree 2), and P5 (degree 3).
pub const MEDIEVAL_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 3, 2, 3], // i – P5 – P4 – P5
    [0, 2, 3, 2], // i – P4 – P5 – P4
    [0, 3, 0, 2], // i – P5 – i  – P4
    [0, 2, 0, 3], // i – P4 – i  – P5
];

/// Lute melody + recorder counter-voice, open-fifth drone bass, tabor + cymbala percussion.
#[derive(Debug, Clone)]
pub struct MedievalPlugin {}

impl Plugin for MedievalPlugin {
    fn mode(&self) -> &'static str {
        "medieval"
    }

    fn mode_string(&self) -> String {
        String::from("medieval")
    }

    fn scale(&self) -> &[u8] {
        &MEDIEVAL_SCALE
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % MEDIEVAL_CHORD_PROGRESSIONS.len();
        MEDIEVAL_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation =
            ((seed >> 8) % MEDIEVAL_BPM_VARIATION as u32) as i32 - MEDIEVAL_BPM_VARIATION / 2;
        MEDIEVAL_BPM_BASE + variation // 80–100
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;

        // Recorder (lead voice): sum of near-harmonic partials modelling a wooden recorder.
        // Real recorders have a very strong fundamental, a faint 2nd harmonic (~8 %), and
        // an even fainter 3rd (~4 %). The result is clear and slightly hollow — not electronic.
        let hz = midi_to_hz(note.midi);
        let fund = sine(sr, hz, dur, 0.32);
        let h2 = sine(sr, hz * 2.0, dur, 0.026); // ~8 % of fundamental
        let h3 = sine(sr, hz * 3.0, dur, 0.013); // ~4 % of fundamental
        let body: alloc::vec::Vec<f32> = fund
            .iter()
            .zip(h2.iter())
            .zip(h3.iter())
            .map(|((&f, &h2v), &h3v)| f + h2v + h3v)
            .collect();
        // Slow onset (recorder needs ~20 ms before the tone speaks cleanly), high sustain.
        let recorder = envelope(sr, &body, 0.022, 0.03, 0.88, 0.10);

        // Lute (accompaniment): plucked string on the harmony note, clearly below the recorder.
        let lute = karplus_strong(sr, midi_to_hz(note.harmony_midi), dur, 0.20);

        (0..total)
            .map(|i| StemSample {
                value: recorder.get(i).copied().unwrap_or(0.0)
                    + lute.get(i).copied().unwrap_or(0.0),
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

        // Open-fifth organum drone: root + perfect fifth (7 semitones up).
        let root = sine(sr, midi_to_hz(note.midi), dur, 0.28);
        let root = envelope(sr, &root, 0.020, 0.05, 0.70, 0.20);
        let fifth = sine(sr, midi_to_hz(note.midi + 7.0), dur, 0.14);
        let fifth = envelope(sr, &fifth, 0.020, 0.05, 0.70, 0.20);

        (0..total.min(root.len()))
            .map(|i| StemSample {
                value: root[i] + fifth.get(i).copied().unwrap_or(0.0),
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn drum_pattern_from_seed(&self, seed: &[u8; 8]) -> DrumPattern {
        let kick = MEDIEVAL_KICK_PATTERNS[(seed[0] % 3) as usize];
        let snare = MEDIEVAL_SNARE_PATTERNS[(seed[1] % 3) as usize];
        let hat = MEDIEVAL_HAT_PATTERNS[(seed[2] % 3) as usize];

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
        // Let a seed bit control whether beat 3's cymbala rings long (open bell).
        if seed[5] & 0x01 != 0 && steps[8].hat {
            steps[8].open_hat = true;
        }
        DrumPattern { steps }
    }

    fn sample_step(&self, step: &DrumStep, config: SampleStepConfig, samples: &mut Vec<f32>) {
        if step.kick {
            // Tabor: pitched membrane tone + short beater-on-skin noise transient.
            let freq = 150.0 + f64::from(config.color % 20);
            let dur_sec = (0.14_f64).min(config.step_duration * 2.5);
            let body = sine(config.sample_rate, freq, dur_sec, 0.28);
            let body = envelope(config.sample_rate, &body, 0.002, 0.12, 0.0, 0.02);
            let transient_dur = (0.012_f64).min(config.step_duration * 0.25);
            let hit = noise_burst(config.sample_rate, config.random, transient_dur, 0.10);
            let hit = envelope(config.sample_rate, &hit, 0.001, 0.010, 0.0, 0.001);
            let mut mixed: alloc::vec::Vec<f32> = body;
            for (i, &v) in hit.iter().enumerate() {
                if i < mixed.len() {
                    mixed[i] += v;
                }
            }
            overlay_samples(&mixed, samples);
        }
        if step.snare {
            // Frame drum body: noise + low membrane tone.
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.16 } else { 0.08 };
            let dur_n = (0.09_f64).min(config.step_duration);
            let noise = noise_burst(config.sample_rate, config.random, dur_n, amp);
            let tone = sine(config.sample_rate, 100.0, dur_n, amp * 0.50);
            let body: alloc::vec::Vec<f32> = noise
                .iter()
                .zip(tone.iter())
                .map(|(&n, &t)| n + t)
                .collect();
            overlay_samples(
                &envelope(config.sample_rate, &body, 0.001, 0.060, 0.0, 0.030),
                samples,
            );

            // Tambourin jingles: three detuned sines simulating zill pairs + metallic friction noise.
            // Each zill pair rings at a slightly different pitch; combined they give the characteristic
            // shimmering jangle. Accented beats get louder jingles.
            let jamp: f32 = if accent { 0.11 } else { 0.07 };
            let f_j = 3100.0 + f64::from(config.color % 10) * 60.0; // 3100–3700 Hz
            let j_dur = (0.07_f64).min(config.step_duration * 1.4);
            let j1 = sine(config.sample_rate, f_j, j_dur, jamp);
            let j1 = envelope(config.sample_rate, &j1, 0.001, 0.052, 0.0, 0.0);
            let j2 = sine(config.sample_rate, f_j * 1.09, j_dur, jamp * 0.85);
            let j2 = envelope(config.sample_rate, &j2, 0.001, 0.043, 0.0, 0.0);
            let j3 = sine(config.sample_rate, f_j * 1.21, j_dur, jamp * 0.70);
            let j3 = envelope(config.sample_rate, &j3, 0.001, 0.036, 0.0, 0.0);
            let j_noise = noise_burst(config.sample_rate, config.random, j_dur, jamp * 0.45);
            let j_noise = envelope(config.sample_rate, &j_noise, 0.001, 0.018, 0.0, 0.0);
            let jingle: alloc::vec::Vec<f32> = j1
                .iter()
                .zip(j2.iter())
                .zip(j3.iter())
                .zip(j_noise.iter())
                .map(|(((&a, &b), &c), &n)| a + b + c + n)
                .collect();
            overlay_samples(&jingle, samples);
        }
        if step.hat {
            // Cymbala (small finger cymbals): four inharmonic bell-mode partials
            // (hum 1.0, tierce 2.76, quint 5.4, nominal 8.93) + a short noise burst
            // for the metal-on-metal strike.  Two pure sines alone produce a
            // "bip/tine" timbre; the extra partials and transient give the metallic
            // shimmering quality of cast bronze.
            let f = 1000.0 + f64::from(config.color % 20) * 30.0; // 1000–1580 Hz
            let amp: f32 = if config.pattern % 2 == 0 { 0.14 } else { 0.09 };
            let (dur, decay) = if step.open_hat {
                ((0.30_f64).min(config.step_duration * 5.0), 0.22_f64)
            } else {
                ((0.15_f64).min(config.step_duration * 2.5), 0.11_f64)
            };

            let p1 = sine(config.sample_rate, f, dur, amp);
            let p2 = sine(config.sample_rate, f * 2.76, dur, amp * 0.55);
            let p3 = sine(config.sample_rate, f * 5.40, dur, amp * 0.22);
            let p4 = sine(config.sample_rate, f * 8.93, dur, amp * 0.08);
            let tonal: alloc::vec::Vec<f32> = p1
                .iter()
                .zip(p2.iter())
                .zip(p3.iter())
                .zip(p4.iter())
                .map(|(((&a, &b), &c), &d)| a + b + c + d)
                .collect();
            let tonal = envelope(config.sample_rate, &tonal, 0.001, decay, 0.0, 0.0);

            let t_dur = (0.012_f64).min(config.step_duration * 0.25);
            let transient = noise_burst(config.sample_rate, config.random, t_dur, amp * 1.6);
            let transient = envelope(config.sample_rate, &transient, 0.001, 0.010, 0.0, 0.001);

            let mut mixed = tonal;
            for (i, &v) in transient.iter().enumerate() {
                if i < mixed.len() {
                    mixed[i] += v;
                }
            }
            overlay_samples(&mixed, samples);
        }
    }
}
