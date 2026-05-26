//! Incremental synthesis: [`WasmIncrementalSynthesizer`] (JS name `IncrementalSynthesizer`).
//!
//! Replaces the old three-stage `SongNoteReader` / `SampleGenerator` / `IterMix` /
//! `DrumSampleGenerator` pipeline with a single object that accepts raw file bytes and returns
//! little-endian mono i16 PCM on each call.

use chiptunomatic::{
    consumer::Consume,
    constants::SAMPLE_RATE,
    random::StdRandom,
    Chiptunomatic, StemOutput,
};
use wasm_bindgen::prelude::*;

use crate::{MetadataSource, SongMetadataView};

/// Incremental synthesizer: feed raw file bytes with [`consumeBytes`] to receive back
/// little-endian mono i16 PCM for each call.
///
/// Replaces the old `SongNoteReader` + `SampleGenerator` + `IterMix` + `DrumSampleGenerator`
/// pipeline. Drums, mixing, and all stems are handled internally.
///
/// # Typical usage
///
/// ```js
/// const synth = IncrementalSynthesizer.withMetadata(metadataView);
/// const CHUNK = 4096;
/// for (let off = 0; off < input.length; off += CHUNK) {
///   const pcm = synth.consumeBytes(input.subarray(off, off + CHUNK));
///   // pcm is a Uint8Array of LE i16 samples — two bytes per sample
/// }
/// synth.free();
/// ```
#[wasm_bindgen(js_name = IncrementalSynthesizer)]
pub struct WasmIncrementalSynthesizer {
    inner: Chiptunomatic,
}

#[wasm_bindgen]
impl WasmIncrementalSynthesizer {
    /// Create a synthesizer configured with the same mode and metadata as `metadata`.
    #[wasm_bindgen(js_name = withMetadata)]
    pub fn with_metadata(metadata: &SongMetadataView) -> WasmIncrementalSynthesizer {
        let mut chip = Chiptunomatic::default()
            .with_default_plugins()
            .with_random(Box::new(StdRandom::new()));
        let _ = chip.set_mode(&metadata.mode_name);
        match &metadata.source {
            MetadataSource::String(name) => {
                let _ = chip.load_song_metadata_from_string(
                    name.as_str(),
                    metadata.inner.data_byte_len,
                );
            }
            MetadataSource::Seed(seed) => {
                let _ = chip.load_song_metadata_from_seed(seed, metadata.inner.data_byte_len);
            }
        }
        WasmIncrementalSynthesizer { inner: chip }
    }

    /// Feed `data` bytes and return all PCM samples generated from them as little-endian i16
    /// bytes (two bytes per sample).
    ///
    /// Call repeatedly with successive file chunks. Returns an empty buffer when the
    /// synthesizer cannot produce more samples from the bytes fed so far.
    #[wasm_bindgen(js_name = consumeBytes)]
    pub fn consume_bytes(&mut self, data: &[u8]) -> Vec<u8> {
        self.inner.extend(data);
        let mut pcm: Vec<u8> = Vec::new();
        while let Some(sample) = self.inner.next() {
            let v = (sample.value * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let [lo, hi] = v.to_le_bytes();
            pcm.push(lo);
            pcm.push(hi);
        }
        pcm
    }

    // --- mixer controls ---

    #[wasm_bindgen(js_name = setMasterVolume)]
    pub fn set_master_volume(&mut self, volume: f32) {
        let mut cfg = *self.inner.mixer().config();
        cfg.master_output.volume = volume;
        self.inner.mixer_mut().set_config(cfg);
    }

    #[wasm_bindgen(js_name = setMasterMuted)]
    pub fn set_master_muted(&mut self, muted: bool) {
        let mut cfg = *self.inner.mixer().config();
        cfg.master_output.muted = muted;
        self.inner.mixer_mut().set_config(cfg);
    }

    #[wasm_bindgen(js_name = setVoiceOutput)]
    pub fn set_voice_output(&mut self, volume: f32, muted: bool, solo: bool) {
        let mut cfg = *self.inner.mixer().config();
        cfg.voice_output = StemOutput { volume, muted, solo };
        self.inner.mixer_mut().set_config(cfg);
    }

    #[wasm_bindgen(js_name = setSquareOutput)]
    pub fn set_square_output(&mut self, volume: f32, muted: bool, solo: bool) {
        let mut cfg = *self.inner.mixer().config();
        cfg.square_output = StemOutput { volume, muted, solo };
        self.inner.mixer_mut().set_config(cfg);
    }

    #[wasm_bindgen(js_name = setTriangleOutput)]
    pub fn set_triangle_output(&mut self, volume: f32, muted: bool, solo: bool) {
        let mut cfg = *self.inner.mixer().config();
        cfg.triangle_output = StemOutput { volume, muted, solo };
        self.inner.mixer_mut().set_config(cfg);
    }

    #[wasm_bindgen(js_name = setNoiseOutput)]
    pub fn set_noise_output(&mut self, volume: f32, muted: bool, solo: bool) {
        let mut cfg = *self.inner.mixer().config();
        cfg.noise_output = StemOutput { volume, muted, solo };
        self.inner.mixer_mut().set_config(cfg);
    }
}

#[wasm_bindgen(js_name = chiptuneSampleRate)]
pub fn wasm_chiptune_sample_rate() -> u32 {
    SAMPLE_RATE
}
