use crate::{
    constants::CHORD_PROGRESSIONS,
    plugin::{overlay_samples, Plugin, SampleStepConfig, SectionDef, StemMask, String, Vec},
    synth::{envelope, noise_burst, square},
    BassNote, DrumStep, MelodyNote, SampleStem, SquareNote, StemSample, TriangleNote,
};


/// Square + triangle waves, 100–140 BPM, pentatonic minor.
#[derive(Debug, Clone)]
pub struct ChiptunePlugin {}

impl ChiptunePlugin {
    pub fn new() -> Self {
        ChiptunePlugin {}
    }
}

impl Plugin for ChiptunePlugin {
    fn mode(&self) -> &'static str {
        "chiptune"
    }

    fn mode_string(&self) -> String {
        String::from("chiptune")
    }

    fn chord_progression_from_seed(&self, seed: u32) -> Vec<usize> {
        let idx = (seed as usize) % CHORD_PROGRESSIONS.len();
        CHORD_PROGRESSIONS[idx].to_vec()
    }

    fn tempo_from_seed(&self, seed: u32) -> i32 {
        let variation = ((seed >> 8) % 30) as i32 - 15;
        165 + variation // 150–179
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        // NES pulse channels cycle through 4 duty cycles; tie to scale degree so
        // each pitch class has a consistent timbre across the song.
        const MELODY_DUTIES: [f64; 4] = [0.5, 0.25, 0.5, 0.125];
        const HARMONY_DUTIES: [f64; 4] = [0.25, 0.5, 0.125, 0.5];
        let duty_idx = note.degree % 4;
        let mut melody_samples = SquareNote {
            midi: (note.midi - 12.0).max(21.0),
            duration: note.duration,
            amp: 0.28,
            duty: MELODY_DUTIES[duty_idx],
            byte_index: note.byte_index,
        }
        .sample_square(sample_rate);

        let harmony_samples = SquareNote {
            midi: (note.harmony_midi - 12.0).max(21.0),
            duration: note.duration,
            amp: 0.18,
            duty: HARMONY_DUTIES[duty_idx],
            byte_index: note.byte_index,
        }
        .sample_square(sample_rate);

        for i in 0..melody_samples.len() {
            melody_samples[i].value += harmony_samples[i].value;
        }
        melody_samples
    }

    fn sample_bass_note(&self, note: BassNote, sample_rate: u32) -> Vec<StemSample> {
        TriangleNote {
            midi: note.midi,
            duration: note.duration,
            byte_index: note.byte_index,
        }
        .sample_triangle(sample_rate)
    }

    fn has_sfx(&self) -> bool {
        true
    }

    fn sample_sfx_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        // Fire a short video-game SFX roughly once every 24 melody notes.
        let note_num = note.byte_index / 3;
        if note_num % 24 != 0 {
            return Vec::new();
        }
        let sr = sample_rate as f64;

        // Helper: phase-accumulator square with linearly changing frequency.
        // f_start → f_end over `dur` seconds, duty cycle `duty` (0..1).
        let freq_sweep = |f_start: f64, f_end: f64, dur: f64, amp: f32, duty: f64| -> Vec<f32> {
            let n = (sr * dur).floor() as usize;
            let mut phase = 0.0_f64;
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let t = i as f64 / n.max(1) as f64;
                let freq = f_start + (f_end - f_start) * t;
                phase = (phase + freq / sr).fract();
                out.push(if phase < duty { amp } else { -amp });
            }
            out
        };

        // Helper: exponential frequency sweep (better for descending "zap" character).
        let exp_sweep = |f_start: f64, f_end: f64, dur: f64, amp: f32, duty: f64| -> Vec<f32> {
            let n = (sr * dur).floor() as usize;
            let log_ratio = (f_end / f_start).ln();
            let mut phase = 0.0_f64;
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let t = i as f64 / n.max(1) as f64;
                let freq = f_start * (log_ratio * t).exp();
                phase = (phase + freq / sr).fract();
                out.push(if phase < duty { amp } else { -amp });
            }
            out
        };

        let raw: Vec<f32> = match (note_num / 24) % 5 {
            0 => {
                // Coin: two sharp metallic pings — no sustain, instant decay, high register.
                // The zero-sustain percussive envelope is what distinguishes it from held notes.
                let mut out = envelope(sr, &square(sr, 1047.0, 0.035, 0.45, 0.5), 0.0, 0.012, 0.0, 0.006);
                out.extend(envelope(sr, &square(sr, 1568.0, 0.055, 0.45, 0.5), 0.0, 0.018, 0.0, 0.008));
                out
            }
            1 => {
                // Jump: smooth upward pitch glide 180 Hz → 650 Hz (narrow duty for buzz).
                // Continuous sweep makes it unmistakably different from any discrete-note SFX.
                let sweep = freq_sweep(180.0, 650.0, 0.18, 0.38, 0.25);
                envelope(sr, &sweep, 0.001, 0.01, 0.85, 0.045)
            }
            2 => {
                // Power-up: 10-note rapid ascending arpeggio across 3 octaves (C3 → C6).
                // Each step only 18 ms so the whole thing sounds like a continuous rising sweep.
                let freqs = [131.0f64, 165.0, 196.0, 262.0, 330.0, 392.0, 523.0, 659.0, 784.0, 1047.0];
                let mut out = Vec::new();
                for &f in &freqs {
                    out.extend(envelope(sr, &square(sr, f, 0.018, 0.40, 0.5), 0.001, 0.004, 0.75, 0.004));
                }
                out
            }
            3 => {
                // Laser: exponential downward sweep 1800 Hz → 80 Hz, very narrow duty (10%).
                // Exponential glide + ultra-narrow pulse = harsh "zap" texture unlike anything else.
                let sweep = exp_sweep(1800.0, 80.0, 0.22, 0.42, 0.10);
                envelope(sr, &sweep, 0.0, 0.005, 0.65, 0.055)
            }
            _ => {
                // 1-UP: E5–G5–E6–C6–D6–G6 with short-short-long rhythmic pattern.
                // The recognisable melodic phrase and rhythmic variation set it apart from sweeps.
                let notes: &[(f64, f64)] = &[
                    (659.0, 0.055), // E5
                    (784.0, 0.055), // G5
                    (1319.0, 0.11), // E6  ← held longer (short-short-LONG pattern)
                    (1047.0, 0.055),// C6
                    (1175.0, 0.055),// D6
                    (1568.0, 0.18), // G6  ← held longer
                ];
                let mut out = Vec::new();
                for &(f, d) in notes {
                    out.extend(envelope(sr, &square(sr, f, d, 0.35, 0.5), 0.002, 0.012, 0.80, 0.018));
                }
                out
            }
        };
        raw.iter()
            .map(|&v| StemSample { value: v, byte_index: note.byte_index })
            .collect()
    }

    fn sample_step(&self, step: &DrumStep, config: SampleStepConfig, samples: &mut Vec<f32>) {
        if step.kick {
            let freq = 60.0 + f64::from(config.color % 12);
            let dur_sec = (0.12_f64).min(config.step_duration * 2.0);
            let raw = square(config.sample_rate, freq, dur_sec, 0.4, 0.2);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.002, 0.09, 0.0, 0.01),
                samples,
            );
        }
        if step.snare {
            let accent = config.pattern == 4 || config.pattern == 12;
            let amp = if accent { 0.22 } else { 0.09 };
            let dur_n = (0.07_f64).min(config.step_duration);
            let raw = noise_burst(config.sample_rate, config.random, dur_n, amp);
            overlay_samples(
                &envelope(config.sample_rate, &raw, 0.001, 0.055, 0.0, 0.015),
                samples,
            );
        }
        if step.hat {
            overlay_samples(
                &if step.open_hat {
                    let d = (0.09_f64).min(config.step_duration * 3.0);
                    let s = noise_burst(config.sample_rate, config.random, d, 0.12);
                    envelope(config.sample_rate, &s, 0.001, 0.07, 0.25, 0.03)
                } else {
                    let amp = if config.pattern % 2 == 0 { 0.10 } else { 0.05 };
                    let d = (0.018_f64).min(config.step_duration * 0.45);
                    noise_burst(config.sample_rate, config.random, d, amp)
                },
                samples,
            );
        }
    }

    fn section_defs_from_seed(&self, seed: u32) -> Vec<SectionDef> {
        // Use bits 16+ of root_seed (bits 0-3 = root note, 8-12 = BPM variation).

        // Bits 16-17: verse beats — 24 (25%), 28 (25%), 32 (50%)
        let verse_beats: u64 = match (seed >> 16) & 3 {
            0 => 24,
            1 => 28,
            _ => 32,
        };
        // Bit 18: intro/outro length — 8 or 16 beats
        let bookend_beats: u64 = if (seed >> 18) & 1 == 0 { 16 } else { 8 };
        // Bit 19: pre-chorus length — 12 or 16 beats
        let prechorus_beats: u64 = if (seed >> 19) & 1 == 0 { 16 } else { 12 };
        // Bit 20: chorus length — 28 or 32 beats
        let chorus_beats: u64 = if (seed >> 20) & 1 == 0 { 32 } else { 28 };
        // Bits 21-22: silence after each chorus — 0.3 / 0.5 / 0.7 / 1.0 s
        let chorus_silence: f64 = [0.3, 0.5, 0.7, 1.0][((seed >> 21) & 3) as usize];
        // Bit 23: voice active in verse (25% chance by requiring 2 bits both set)
        let voice_in_verse = (seed >> 23) & 3 == 3;
        // Bit 25: bridge present or absent
        let has_bridge = (seed >> 25) & 1 == 0;
        // Bit 26: bridge length — 12 or 16 beats
        let bridge_beats: u64 = if (seed >> 26) & 1 == 0 { 16 } else { 12 };

        let intro = SectionDef {
            beats: bookend_beats,
            stems: StemMask { voice: false, square: true, triangle: true, noise: false, sfx: false },
            silence_after: 0.8,
        };
        let verse = SectionDef {
            beats: verse_beats,
            stems: StemMask { voice: voice_in_verse, square: true, triangle: true, noise: true, sfx: true },
            silence_after: 0.0,
        };
        let pre_chorus = SectionDef {
            beats: prechorus_beats,
            stems: StemMask::ALL,
            silence_after: 0.0,
        };
        let chorus = SectionDef {
            beats: chorus_beats,
            stems: StemMask::ALL,
            silence_after: chorus_silence,
        };
        let bridge = SectionDef {
            beats: bridge_beats,
            stems: StemMask { voice: false, square: false, triangle: true, noise: true, sfx: false },
            silence_after: 0.8,
        };
        let outro = SectionDef {
            beats: bookend_beats,
            stems: StemMask { voice: false, square: true, triangle: true, noise: false, sfx: false },
            silence_after: 1.5,
        };

        // intro → verse → pre-chorus → chorus → verse → pre-chorus → chorus → [bridge →] chorus → outro
        let mut sections = Vec::new();
        sections.push(intro);
        sections.push(verse);
        sections.push(pre_chorus);
        sections.push(chorus);
        sections.push(verse);
        sections.push(pre_chorus);
        sections.push(chorus);
        if has_bridge {
            sections.push(bridge);
        }
        sections.push(SectionDef { silence_after: 0.0, ..chorus });
        sections.push(outro);
        sections
    }
}
