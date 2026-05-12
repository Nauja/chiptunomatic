//! Shared note → sample → mix pipeline for headless CLI and TUI playback.

use std::fs::File;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Result;
use chiptunomatic::{Mix, ReadSongNotes, Sample, SongMetadata, StdDrumSampleGenerator};

/// Signals that TUI playback was interrupted to start another file or stop streaming.
#[derive(Debug)]
pub(crate) struct PlaybackCancelled;

impl std::fmt::Display for PlaybackCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("playback cancelled")
    }
}

impl std::error::Error for PlaybackCancelled {}

pub(crate) trait MixChunkSink {
    fn consume(&mut self, mixes: &[Mix]) -> Result<()>;

    /// Per-stem samples before peak-normalized mix; used for TUI waveform display.
    fn tap_samples(&mut self, _samples: &[Sample]) {}
}

/// Outcome of streaming a song through a [`MixChunkSink`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SynthCompletion {
    /// Reached end of the note stream.
    Finished,
    /// Stopped early (e.g. playback cancelled for a new request).
    Interrupted,
}

pub(crate) fn drive_synthesis(
    path: &std::path::Path,
    consumer: &mut dyn MixChunkSink,
) -> Result<SynthCompletion> {
    let metadata = SongMetadata::from_path(path)?;
    let reader = File::open(path)?;
    drive_synthesis_reader(metadata, reader, consumer)
}

pub(crate) fn drive_synthesis_reader(
    metadata: SongMetadata,
    reader: File,
    consumer: &mut dyn MixChunkSink,
) -> Result<SynthCompletion> {
    let song_samples = ReadSongNotes::new(metadata.clone(), reader).samples();
    let drum_samples = StdDrumSampleGenerator::new(metadata).samples();
    let mut mixes = song_samples.mix(drum_samples);
    let mut mixes_buffer = [Mix::default(); 1024];
    let mut interrupted = false;

    loop {
        match mixes.fill_buf(&mut mixes_buffer)? {
            0 => break,
            n => {
                consumer.tap_samples(
                    mixes_buffer
                        .iter()
                        .take(n)
                        .map(|m| m.sample)
                        .collect::<Vec<Sample>>()
                        .as_slice(),
                );
                match consumer.consume(&mixes_buffer[0..n]) {
                    Ok(()) => {}
                    Err(e) if e.is::<PlaybackCancelled>() => {
                        interrupted = true;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    Ok(if interrupted {
        SynthCompletion::Interrupted
    } else {
        SynthCompletion::Finished
    })
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

impl MixChunkSink for PlaybackSink<'_> {
    fn tap_samples(&mut self, samples: &[Sample]) {
        if let Ok(mut g) = self.stem_plot.lock() {
            g.push_samples(samples);
        }
    }

    fn consume(&mut self, mixes: &[Mix]) -> Result<()> {
        if self.live_generation.load(Ordering::SeqCst) != self.my_generation {
            return Err(PlaybackCancelled.into());
        }

        if mixes.is_empty() {
            return Ok(());
        }

        let chunk: Vec<f32> = mixes.iter().map(|m| m.frequency).collect();
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
