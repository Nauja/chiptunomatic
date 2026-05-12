//! wasm-bindgen entry: [`SongMetadata`] helpers and incremental [`SongNoteReader`] /
//! [`SampleGenerator`] / [`MixGenerator`] (JS name [`IterMix`]) bindings.

mod incremental;

use chiptunomatic::{GenerateError, SongMetadata};
use wasm_bindgen::prelude::*;

fn gen_err(e: GenerateError) -> JsValue {
    JsValue::from_str(&e.to_string())
}

// --- SongMetadata (matches `chiptunomatic` core; no filesystem) -------------------------------

/// JavaScript handle for [`SongMetadata`]. Obtain via [`create_song_metadata_from_string`] or
/// [`create_song_metadata_from_seed`].
#[wasm_bindgen(js_name = SongMetadataView)]
pub struct SongMetadataView {
    #[wasm_bindgen(skip)]
    inner: SongMetadata,
}

#[wasm_bindgen]
impl SongMetadataView {
    /// 16-byte MD5 digest used as the numeric seed bundle (same as core [`SongMetadata::seed`]).
    #[wasm_bindgen(getter, js_name = seed)]
    pub fn seed_bytes(&self) -> Vec<u8> {
        self.inner.seed.clone()
    }

    #[wasm_bindgen(getter, js_name = rngSeed)]
    pub fn rng_seed(&self) -> u64 {
        self.inner.rng_seed
    }

    #[wasm_bindgen(getter, js_name = rootSemitone)]
    pub fn root_semitone(&self) -> u8 {
        self.inner.root_semitone
    }

    #[wasm_bindgen(getter)]
    pub fn bpm(&self) -> i32 {
        self.inner.bpm
    }

    /// Indexes into [`chiptunomatic::PENTATONIC_MINOR`] / chord table; one number per chord step.
    #[wasm_bindgen(getter, js_name = chordProgression)]
    pub fn chord_progression(&self) -> Vec<u32> {
        self.inner
            .chord_progression
            .iter()
            .map(|&i| i as u32)
            .collect()
    }

    #[wasm_bindgen(getter, js_name = chordDescription)]
    pub fn chord_description(&self) -> String {
        self.inner.chord_description.clone()
    }

    #[wasm_bindgen(getter, js_name = timingBpm)]
    pub fn timing_bpm(&self) -> i32 {
        self.inner.timing.bpm
    }

    #[wasm_bindgen(getter, js_name = beatDuration)]
    pub fn beat_duration(&self) -> f64 {
        self.inner.timing.beat_duration
    }

    #[wasm_bindgen(getter)]
    pub fn sixteenth(&self) -> f64 {
        self.inner.timing.sixteenth
    }

    #[wasm_bindgen(getter, js_name = totalBeats)]
    pub fn total_beats(&self) -> u64 {
        self.inner.total_beats
    }

    #[wasm_bindgen(getter, js_name = totalDuration)]
    pub fn total_duration(&self) -> f64 {
        self.inner.total_duration
    }

    #[wasm_bindgen(getter, js_name = dataByteLen)]
    pub fn data_byte_len(&self) -> u64 {
        self.inner.data_byte_len
    }

    #[wasm_bindgen(getter, js_name = drumKick)]
    pub fn drum_kick(&self) -> Vec<u8> {
        self.inner.drum_pattern.steps.iter().map(|s| s.kick as u8).collect()
    }

    #[wasm_bindgen(getter, js_name = drumSnare)]
    pub fn drum_snare(&self) -> Vec<u8> {
        self.inner.drum_pattern.steps.iter().map(|s| s.snare as u8).collect()
    }

    #[wasm_bindgen(getter, js_name = drumHat)]
    pub fn drum_hat(&self) -> Vec<u8> {
        self.inner.drum_pattern.steps.iter().map(|s| s.hat as u8).collect()
    }

    #[wasm_bindgen(getter, js_name = drumPatternSeed)]
    pub fn drum_pattern_seed(&self) -> Vec<u8> {
        self.inner.drum_seed.to_vec()
    }

    /// `undefined` if no open-hat accent; otherwise the 16-step index (see core [`DrumPattern`]).
    #[wasm_bindgen(getter, js_name = openHatStep)]
    pub fn open_hat_step(&self) -> Option<u32> {
        self.inner.drum_pattern.steps.iter()
            .find(|s| s.open_hat)
            .map(|s| s.offset as u32)
    }

    #[inline]
    pub(crate) fn clone_inner_metadata(&self) -> SongMetadata {
        self.inner.clone()
    }
}

/// Same as [`SongMetadata::from_string`]: UTF-8 `name` bytes plus stream length used for timing / beats.
#[wasm_bindgen(js_name = createSongMetadataFromString)]
pub fn create_song_metadata_from_string(
    name: &str,
    data_byte_len: u64,
) -> Result<SongMetadataView, JsValue> {
    SongMetadata::from_string(name, data_byte_len)
        .map(|inner| SongMetadataView { inner })
        .map_err(gen_err)
}

/// Same as [`SongMetadata::from_seed`]: arbitrary seed bytes (hashed) plus stream length.
#[wasm_bindgen(js_name = createSongMetadataFromSeed)]
pub fn create_song_metadata_from_seed(
    seed: &[u8],
    data_byte_len: u64,
) -> Result<SongMetadataView, JsValue> {
    SongMetadata::from_seed(seed, data_byte_len)
        .map(|inner| SongMetadataView { inner })
        .map_err(gen_err)
}
