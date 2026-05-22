use crate::{
    constants::CHORD_PROGRESSIONS,
    plugin::{overlay_samples, Plugin, SampleStepConfig, String, Vec},
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
        let variation = ((seed >> 8) % 40) as i32 - 20;
        120 + variation // 100–140
    }

    fn sample_melody_note(&self, note: MelodyNote, sample_rate: u32) -> Vec<StemSample> {
        let mut melody_samples = SquareNote {
            midi: note.midi,
            duration: note.duration,
            amp: 0.28,
            duty: 0.5,
            byte_index: note.byte_index,
        }
        .sample_square(sample_rate);

        let harmony_samples = SquareNote {
            midi: note.harmony_midi,
            duration: note.duration,
            amp: 0.18,
            duty: 0.25,
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
}
