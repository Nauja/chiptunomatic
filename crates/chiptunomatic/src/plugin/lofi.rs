use crate::plugin::{Plugin, String, Vec};
use crate::synth::{fm_sine, karplus_strong, midi_to_hz, pitch_sweep_sine, sine};
use crate::{
    plugin::{overlay_samples, SampleStepConfig},
    synth::{envelope, noise_burst},
    DrumSample, DrumStep,
};
use crate::{BassNote, MelodyNote, StemSample};

pub const LOFI_BPM_BASE: i32 = 78;
pub const LOFI_BPM_VARIATION: i32 = 8; // (seed >> 8) % 8 → 0..7, centre at 4 → 74–81 BPM

/// Jazz-flavoured 4-chord loops for lofi mode. Degree indices into [`PENTATONIC_MINOR`].
/// Pentatonic minor degrees: 0=root, 1=m3, 2=P4, 3=P5, 4=m7 — enough for jazzy feel.
pub const LOFI_CHORD_PROGRESSIONS: [[usize; 4]; 4] = [
    [0, 4, 3, 4], // i – m7 – P5 – m7
    [0, 2, 4, 3], // i – P4 – m7 – P5
    [0, 3, 2, 4], // i – P5 – P4 – m7
    [0, 2, 3, 1], // i – P4 – P5 – m3
];

/// Use lofi synthesis (FM sine waves, slower BPM, jazz chords).
/// FM sine waves, 74–82 BPM, jazz-flavoured chord progressions
#[derive(Debug, Clone)]
pub struct LofiPlugin {}

impl Plugin for LofiPlugin {
    fn mode(&self) -> &'static str {
        "lofi"
    }

    fn mode_string(&self) -> String {
        String::from("lofi")
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % LOFI_CHORD_PROGRESSIONS.len();
        LOFI_CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % LOFI_BPM_VARIATION as u32) as i32 - LOFI_BPM_VARIATION / 2;
        LOFI_BPM_BASE + variation // 74–82
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let sr = sample_rate as f64;
        let total = (sr * note.duration).floor() as usize;
        if total == 0 {
            return Vec::new();
        }
        let dur = total as f64 / sr;
        let hz = midi_to_hz(note.midi);

        // Rhodes tine: two FM layers reproduce the characteristic bright-then-warm
        // transition of a struck metal tine.
        // Attack layer — high β creates the initial bright "ding"; decays to silence
        // in ~55 ms so it doesn't sustain beyond the transient.
        let tine_attack = fm_sine(sr, hz, dur, 0.25, 2.0, 2.8);
        let tine_attack = envelope(sr, &tine_attack, 0.001, 0.055, 0.0, 0.005);
        // Body layer — low β gives the warm, slightly bell-like sustain.
        let tine_body = fm_sine(sr, hz, dur, 0.20, 2.0, 0.45);
        let tine_body = envelope(sr, &tine_body, 0.003, 0.10, 0.55, 0.12);
        // Harmony voice (body only, quieter)
        let harm = fm_sine(sr, midi_to_hz(note.harmony_midi), dur, 0.09, 2.0, 0.45);
        let harm = envelope(sr, &harm, 0.003, 0.10, 0.55, 0.12);

        // Warm brass pad on the harmony pitch: FM 1:1 mod ratio, β=1.8 gives the
        // characteristic closed, buzzy spectrum of a muted brass section.  The slow
        // 40 ms attack lets it swell in under the Rhodes rather than competing with
        // the tine's attack transient.
        let brass = fm_sine(sr, midi_to_hz(note.harmony_midi), dur, 0.11, 1.0, 1.8);
        let brass = envelope(sr, &brass, 0.040, 0.08, 0.65, 0.15);

        // Lead flute: near-pure additive tone on the melody pitch.
        // Flutes are close to a pure sine; very faint 2nd and 3rd harmonics
        // add the slight cylindrical-bore colour without FM metallic artefacts.
        // The 20 ms attack mimics breath building up before the tone speaks fully.
        let flute_f = sine(sr, hz, dur, 0.16);
        let flute_h2 = sine(sr, hz * 2.0, dur, 0.028);
        let flute_h3 = sine(sr, hz * 3.0, dur, 0.009);
        let flute_sum: alloc::vec::Vec<f32> = (0..total)
            .map(|i| {
                flute_f.get(i).copied().unwrap_or(0.0)
                    + flute_h2.get(i).copied().unwrap_or(0.0)
                    + flute_h3.get(i).copied().unwrap_or(0.0)
            })
            .collect();
        let flute = envelope(sr, &flute_sum, 0.020, 0.060, 0.85, 0.12);

        (0..total)
            .map(|i| StemSample {
                value: tine_attack.get(i).copied().unwrap_or(0.0)
                    + tine_body.get(i).copied().unwrap_or(0.0)
                    + harm.get(i).copied().unwrap_or(0.0)
                    + brass.get(i).copied().unwrap_or(0.0)
                    + flute.get(i).copied().unwrap_or(0.0),
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
        let hz = midi_to_hz(note.midi);

        // Three strings voiced as an open guitar chord: root, perfect fifth, octave.
        // Each KS voice is seeded from its own period (different frequency → different
        // LCG state), so they sound like distinct strings rather than copies.
        // The 8 ms attack ramp (sustain=1.0, decay=0) replaces the hard KS click
        // with a mellow pluck onset while leaving the natural KS decay untouched.
        let s1 = karplus_strong(sr, hz, dur, 0.20);
        let s1 = envelope(sr, &s1, 0.008, 0.0, 1.0, 0.0);
        let s2 = karplus_strong(sr, hz * 1.4983, dur, 0.11); // perfect fifth
        let s2 = envelope(sr, &s2, 0.008, 0.0, 1.0, 0.0);
        let s3 = karplus_strong(sr, hz * 2.0, dur, 0.05); // octave
        let s3 = envelope(sr, &s3, 0.008, 0.0, 1.0, 0.0);

        // Warm sine at the root reinforces the fundamental and rounds off the
        // high-frequency KS noise, keeping the overall timbre mellow.
        let body = sine(sr, hz, dur, 0.10);
        let body = envelope(sr, &body, 0.010, 0.12, 0.45, 0.08);

        (0..total)
            .map(|i| StemSample {
                value: s1.get(i).copied().unwrap_or(0.0)
                    + s2.get(i).copied().unwrap_or(0.0)
                    + s3.get(i).copied().unwrap_or(0.0)
                    + body.get(i).copied().unwrap_or(0.0),
                byte_index: note.byte_index,
            })
            .collect()
    }

    fn sample_step(&self, step: DrumStep, config: SampleStepConfig, samples: &mut Vec<DrumSample>) {
        if step.kick {
            // Punchy boom-bap kick: pitch sweep from 100 Hz → 45 Hz
            let dur_sec = (0.15_f64).min(config.step_duration * 2.5);
            let raw = pitch_sweep_sine(config.sample_rate, 100.0, 45.0, dur_sec, 0.35, 30.0);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.001, 0.07, 0.0, 0.04),
                samples,
            );
        }
        if step.snare {
            // Soft snare: noise + subtle body tone
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.14 } else { 0.07 };
            let dur_n = (0.10_f64).min(config.step_duration);
            let noise = noise_burst(config.sample_rate, &config.random, dur_n, amp);
            let tone = sine(config.sample_rate, 180.0, dur_n, amp * 0.4);
            let mixed: alloc::vec::Vec<f32> = noise
                .iter()
                .zip(tone.iter())
                .map(|(&n, &t)| n + t)
                .collect();
            overlay_samples(
                &envelope(config.sample_rate, &mixed, 0.001, 0.07, 0.0, 0.025),
                samples,
            );
        }
        if step.hat {
            // Very soft, short hat — lofi hats sit quietly in the mix
            overlay_samples(
                &if step.open_hat {
                    let d = (0.07_f64).min(config.step_duration * 2.5);
                    let s = noise_burst(config.sample_rate, &config.random, d, 0.07);
                    envelope(config.sample_rate, &s, 0.001, 0.04, 0.20, 0.03)
                } else {
                    let amp = if config.pattern % 2 == 0 { 0.05 } else { 0.03 };
                    let d = (0.012_f64).min(config.step_duration * 0.35);
                    noise_burst(config.sample_rate, &config.random, d, amp)
                },
                samples,
            );
        }

        // Continuous tape hiss present on every step, including silent ones.
        // samples is pre-sized with zeros by DrumSampleGenerator, so overlay
        // works even when no kick/snare/hat fired.
        let hiss = noise_burst(
            config.sample_rate,
            &config.random,
            config.step_duration,
            0.014,
        );
        overlay_samples(&hiss, samples);

        // Random vinyl crackle: ~8 % of steps get a short sharp pop at the
        // start of the step window.  Amplitude and duration vary so no two
        // crackles sound identical.
        if config.random.next_float() < 0.08 {
            let amp = 0.07 + config.random.next_float() * 0.09; // 0.07–0.16
            let dur = 0.002 + config.random.next_float() as f64 * 0.003; // 2–5 ms
            let crackle = noise_burst(config.sample_rate, &config.random, dur, amp);
            let crackle = envelope(config.sample_rate, &crackle, 0.0, dur * 0.6, 0.0, dur * 0.4);
            overlay_samples(&crackle, samples);
        }
    }
}
