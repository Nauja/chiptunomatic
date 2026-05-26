//! Waveforms, envelope, and MIDI → Hz

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use crate::random::Random;

pub fn midi_to_hz(midi_note: f64) -> f64 {
    440.0 * 2_f64.powf((midi_note - 69.0) / 12.0)
}

/// Like `numpy.linspace` with `endpoint=True`: `num == 1` yields `[start]`.
fn linspace(start: f32, end: f32, num: usize) -> Vec<f32> {
    match num {
        0 => vec![],
        1 => vec![start],
        _ => (0..num)
            .map(|i| start + (end - start) * (i as f32 / (num - 1) as f32))
            .collect(),
    }
}

/// Square wave; `duty` 0.5 = symmetric.
pub fn square(sample_rate: f64, freq: f64, duration: f64, amp: f32, duty: f64) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    let threshold = (2.0 * duty - 1.0) as f32;
    let two_pi_f = 2.0 * core::f64::consts::PI * freq;
    (0..n)
        .map(|i| {
            let t = i as f64 / sample_rate;
            let s = (two_pi_f * t).sin() as f32;
            let x = s - threshold;
            let sign = if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            };
            amp * sign
        })
        .collect()
}

pub fn triangle(sample_rate: f64, freq: f64, duration: f64, amp: f32) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    let two_pi = 2.0 * core::f64::consts::PI;
    let coef = amp as f64 * (2.0 / core::f64::consts::PI);
    (0..n)
        .map(|i| {
            let t = i as f64 / sample_rate;
            let s = (two_pi * freq * t).sin();
            (coef * s.asin()) as f32
        })
        .collect()
}

pub fn noise_burst(
    sample_rate: f64,
    random: &mut Box<dyn Random>,
    duration: f64,
    amp: f32,
) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    (0..n)
        .map(|_| amp * (random.next_float() * 2.0 - 1.0))
        .collect()
}

/// Pure sine wave.
pub fn sine(sample_rate: f64, freq: f64, duration: f64, amp: f32) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    let two_pi_f = 2.0 * core::f64::consts::PI * freq;
    (0..n)
        .map(|i| amp * (two_pi_f * (i as f64 / sample_rate)).sin() as f32)
        .collect()
}

/// FM synthesis — simulates electric-piano / rhodes character.
///
/// `mod_ratio`: modulator frequency = `freq * mod_ratio`
/// `mod_index` (β): higher values add harmonic richness (1–3 is a typical electric-piano range)
pub fn fm_sine(
    sample_rate: f64,
    freq: f64,
    duration: f64,
    amp: f32,
    mod_ratio: f64,
    mod_index: f64,
) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    let two_pi = 2.0 * core::f64::consts::PI;
    (0..n)
        .map(|i| {
            let t = i as f64 / sample_rate;
            let modulator = mod_index * (two_pi * freq * mod_ratio * t).sin();
            amp * (two_pi * freq * t + modulator).sin() as f32
        })
        .collect()
}

/// Karplus-Strong plucked-string synthesis.
///
/// A delay line of length `period = sample_rate / freq` is seeded with deterministic
/// pseudo-random noise, then each output sample is the average of the current and previous
/// delay-line values. The averaging acts as a one-pole low-pass filter: energy at the
/// fundamental decays slowly while higher harmonics damp out quickly, mimicking a vibrating
/// string losing energy to friction. Because higher frequencies have shorter delay lines, they
/// complete more filter passes per unit time and decay faster — matching real plucked strings.
pub fn karplus_strong(sample_rate: f64, freq: f64, duration: f64, amp: f32) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    if n == 0 {
        return Vec::new();
    }
    let period = ((sample_rate / freq).round() as usize).max(2);

    // Deterministic LCG seeded from the period so identical pitches sound identical,
    // but different pitches produce independent noise bursts.
    let mut lcg: u32 = (period as u32).wrapping_mul(2654435761);
    let mut buf: Vec<f32> = (0..period)
        .map(|_| {
            lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
            (lcg >> 15) as f32 / 32767.0 - 1.0
        })
        .collect();

    let mut out = Vec::with_capacity(n);
    let mut pos = 0usize;
    for _ in 0..n {
        let prev = (pos + period - 1) % period;
        let avg = (buf[pos] + buf[prev]) * 0.5;
        buf[pos] = avg;
        out.push(avg * amp);
        pos = (pos + 1) % period;
    }
    out
}

/// Sine with gentle vibrato — LFO modulates the instantaneous frequency.
///
/// `vib_rate`: LFO oscillation speed in Hz (typical human vibrato: 5–6 Hz).
/// `vib_depth`: fractional pitch deviation (0.015 = ±1.5 %).
/// Uses a phase accumulator so frequency transitions are click-free.
pub fn vibrato_sine(
    sample_rate: f64,
    freq: f64,
    duration: f64,
    amp: f32,
    vib_rate: f64,
    vib_depth: f64,
) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    let two_pi = 2.0 * core::f64::consts::PI;
    let sr_recip = 1.0 / sample_rate;
    (0..n)
        .scan(0.0f64, move |phase, i| {
            let t = i as f64 * sr_recip;
            let inst_freq = freq * (1.0 + vib_depth * (two_pi * vib_rate * t).sin());
            *phase += two_pi * inst_freq * sr_recip;
            Some(amp * (*phase).sin() as f32)
        })
        .collect()
}

/// Sine with an exponential pitch sweep — for a punchy lofi kick.
///
/// Frequency decays from `freq_start` toward `freq_end`; `decay_rate` controls how fast
/// (higher = faster sweep; 30.0 reaches ~95 % of the way in 0.1 s).
pub fn pitch_sweep_sine(
    sample_rate: f64,
    freq_start: f64,
    freq_end: f64,
    duration: f64,
    amp: f32,
    decay_rate: f64,
) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    let two_pi = 2.0 * core::f64::consts::PI;
    let sr_recip = 1.0 / sample_rate;
    (0..n)
        .scan(0.0f64, move |phase, i| {
            let t = i as f64 * sr_recip;
            let freq = freq_end + (freq_start - freq_end) * (-decay_rate * t).exp();
            *phase += two_pi * freq * sr_recip;
            Some(amp * (*phase).sin() as f32)
        })
        .collect()
}

pub fn envelope(
    sample_rate: f64,
    samples: &[f32],
    attack: f64,
    decay: f64,
    sustain: f32,
    release: f64,
) -> Vec<f32> {
    let n = samples.len();
    let a = (attack * sample_rate).floor() as usize;
    let a = a.min(n);
    let d = (decay * sample_rate).floor() as usize;
    let d = d.min(n.saturating_sub(a));
    let r = (release * sample_rate).floor() as usize;
    let r = r.min(n);
    let s_len = n.saturating_sub(a + d + r);

    let mut env: Vec<f32> = Vec::new();
    env.extend(linspace(0.0, 1.0, a));
    env.extend(linspace(1.0, sustain, d));
    env.extend(core::iter::repeat(sustain).take(s_len));
    env.extend(linspace(sustain, 0.0, r));

    while env.len() < n {
        env.push(0.0);
    }
    env.truncate(n);

    samples
        .iter()
        .zip(env.iter())
        .map(|(&s, &e)| s * e)
        .collect()
}
