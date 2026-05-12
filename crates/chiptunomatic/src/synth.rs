//! Waveforms, envelope, and MIDI → Hz

use alloc::vec;
use alloc::vec::Vec;

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

pub fn noise_burst<R: rand::Rng + ?Sized>(
    sample_rate: f64,
    rng: &mut R,
    duration: f64,
    amp: f32,
) -> Vec<f32> {
    let n = (sample_rate * duration).floor() as usize;
    (0..n)
        .map(|_| amp * (rng.gen::<f32>() * 2.0 - 1.0))
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
