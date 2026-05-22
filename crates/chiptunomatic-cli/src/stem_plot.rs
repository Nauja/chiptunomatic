//! Rolling buffers of per-stem waveform samples for the playback panel charts.

use chiptunomatic::Sample;

/// Rolling waveform buffers for realtime stem visualization (voice, square, triangle, noise, sfx).
pub(crate) struct StemPlotBuffer {
    voice: Vec<f32>,
    square: Vec<f32>,
    triangle: Vec<f32>,
    noise: Vec<f32>,
    sfx: Vec<f32>,
    cap_samples: usize,
}

impl StemPlotBuffer {
    pub(crate) fn new(cap_samples: usize) -> Self {
        Self {
            voice: Vec::new(),
            square: Vec::new(),
            triangle: Vec::new(),
            noise: Vec::new(),
            sfx: Vec::new(),
            cap_samples: cap_samples.max(256),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.voice.clear();
        self.square.clear();
        self.triangle.clear();
        self.noise.clear();
        self.sfx.clear();
    }

    pub(crate) fn push_samples(&mut self, samples: &[Sample]) {
        if samples.is_empty() {
            return;
        }
        for s in samples {
            self.voice.push(s.stems.voice.value);
            self.square.push(s.stems.square.value);
            self.triangle.push(s.stems.triangle.value);
            self.noise.push(s.stems.noise);
            self.sfx.push(s.stems.sfx.value);
        }
        self.trim();
    }

    fn trim(&mut self) {
        let excess = self.square.len().saturating_sub(self.cap_samples);
        if excess > 0 {
            self.voice.drain(..excess);
            self.square.drain(..excess);
            self.triangle.drain(..excess);
            self.noise.drain(..excess);
            self.sfx.drain(..excess);
        }
    }

    pub(crate) fn voice(&self) -> &[f32] {
        &self.voice
    }

    pub(crate) fn square(&self) -> &[f32] {
        &self.square
    }

    pub(crate) fn triangle(&self) -> &[f32] {
        &self.triangle
    }

    pub(crate) fn noise(&self) -> &[f32] {
        &self.noise
    }

    pub(crate) fn sfx(&self) -> &[f32] {
        &self.sfx
    }

    pub(crate) fn voice_peak(&self) -> f32 {
        recent_peak(&self.voice, self.peak_window())
    }
    pub(crate) fn square_peak(&self) -> f32 {
        recent_peak(&self.square, self.peak_window())
    }
    pub(crate) fn triangle_peak(&self) -> f32 {
        recent_peak(&self.triangle, self.peak_window())
    }
    pub(crate) fn noise_peak(&self) -> f32 {
        recent_peak(&self.noise, self.peak_window())
    }
    pub(crate) fn sfx_peak(&self) -> f32 {
        recent_peak(&self.sfx, self.peak_window())
    }

    /// Window size for level metering: ~25 ms worth of samples.
    fn peak_window(&self) -> usize {
        (self.cap_samples / 6).max(1)
    }
}

fn recent_peak(buf: &[f32], window: usize) -> f32 {
    let start = buf.len().saturating_sub(window);
    buf[start..].iter().map(|x| x.abs()).fold(0.0_f32, f32::max)
}

/// Down-sample `buf` to at most `max_points` points for [`ratatui::widgets::Chart`].
pub(crate) fn waveform_points(buf: &[f32], max_points: usize) -> Vec<(f64, f64)> {
    let max_points = max_points.clamp(8, 512);
    let n = buf.len();
    if n == 0 {
        return vec![(0.0, 0.0), (1.0, 0.0)];
    }

    let stride = (n / max_points).max(1);

    let mut pts: Vec<(f64, f64)> = Vec::new();
    for i in (0..n).step_by(stride) {
        let y = (buf[i] as f64).clamp(-1.2, 1.2);
        let xf = i as f64 / (n - 1).max(1) as f64;
        pts.push((xf, y));
    }

    if pts.len() < 2 {
        let y = (buf[n - 1] as f64).clamp(-1.2, 1.2);
        pts.push((1.0, y));
    }

    pts
}
