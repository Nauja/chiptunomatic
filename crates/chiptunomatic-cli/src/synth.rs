//! Shared note → sample → mix pipeline for headless CLI and TUI playback.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Result;
use chiptunomatic::Sample;

/// Signals that TUI playback was interrupted to start another file or stop streaming.
#[derive(Debug)]
pub(crate) struct PlaybackCancelled;

impl std::fmt::Display for PlaybackCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("playback cancelled")
    }
}

impl std::error::Error for PlaybackCancelled {}

/// Outcome of a synthesis run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SynthCompletion {
    /// Reached end of the note stream.
    Finished,
    /// Stopped early (e.g. playback cancelled for a new request).
    Interrupted,
}

#[derive(Debug)]
pub(crate) struct PlaybackProgress {
    elapsed: AtomicU64,
    total: AtomicU64,
}

impl PlaybackProgress {
    pub(crate) fn new() -> Self {
        Self {
            elapsed: AtomicU64::new(0),
            total: AtomicU64::new(0),
        }
    }

    pub(crate) fn begin_track(&self, total_samples: u64) {
        self.elapsed.store(0, Ordering::Relaxed);
        self.total.store(total_samples.max(1), Ordering::Relaxed);
    }

    pub(crate) fn reset_idle(&self) {
        self.elapsed.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
    }

    /// Sets progress to the end of the current track estimate (natural completion only).
    pub(crate) fn mark_complete(&self) {
        let t = self.total.load(Ordering::Relaxed);
        if t != 0 {
            self.elapsed.store(t, Ordering::Relaxed);
        }
    }

    pub(crate) fn add_samples(&self, n: u64) {
        if n == 0 || self.total.load(Ordering::Relaxed) == 0 {
            return;
        }
        self.elapsed.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn ratio(&self) -> f64 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let elapsed = self.elapsed.load(Ordering::Relaxed);
        ((elapsed.min(total)) as f64 / total as f64).clamp(0.0, 1.0)
    }

    pub(crate) fn elapsed_samples(&self) -> u64 {
        let total = self.total.load(Ordering::Relaxed);
        self.elapsed.load(Ordering::Relaxed).min(total)
    }

    pub(crate) fn total_samples(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }
}

pub(crate) struct PlaybackSink<'a> {
    pub(crate) audio: &'a mut crate::audio::AudioPlayer,
    /// Bumped on every new play request; when it differs from `my_generation`, stop this track.
    pub(crate) live_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub(crate) my_generation: u64,
    pub(crate) stem_plot: std::sync::Arc<std::sync::Mutex<crate::stem_plot::StemPlotBuffer>>,
    pub(crate) playback_progress: std::sync::Arc<PlaybackProgress>,
    pub(crate) is_paused: std::sync::Arc<AtomicBool>,
}

impl PlaybackSink<'_> {
    pub(crate) fn tap_samples(&mut self, samples: &[Sample]) {
        if let Ok(mut g) = self.stem_plot.lock() {
            g.push_samples(samples);
        }
    }

    pub(crate) fn consume(&mut self, samples: &[Sample]) -> Result<()> {
        if self.live_generation.load(Ordering::SeqCst) != self.my_generation {
            return Err(PlaybackCancelled.into());
        }

        if samples.is_empty() {
            return Ok(());
        }

        // Re-apply per-stem volumes/muting from the live snapshot.
        // `Mix.sample` holds raw per-stem synthesis output (bare default generator was used).
        let chunk: Vec<f32> = samples.iter().map(|s| s.value).collect();
        self.audio.append(chunk.as_slice());
        self.playback_progress.add_samples(chunk.len() as u64);

        loop {
            if self.live_generation.load(Ordering::SeqCst) != self.my_generation {
                return Err(PlaybackCancelled.into());
            }

            if self.is_paused.load(Ordering::SeqCst) {
                if !self.audio.sink.is_paused() {
                    self.audio.sink.pause();
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }

            if self.audio.sink.is_paused() {
                self.audio.sink.play();
            }

            if self.audio.sink.len() <= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        Ok(())
    }
}
