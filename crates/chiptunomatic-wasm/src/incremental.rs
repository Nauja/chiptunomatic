//! Incremental WASM API: expose [`SongNoteReader`] (backing buffer per [`consume_input_bytes`] call),
//! [`SongSampleGenerator`] (JS name [`SampleGenerator`]), and [`MixGenerator`] (JS name [`IterMix`]).

use chiptunomatic::{
    constants::{MAX_PLAN_CHUNK_NOTES, SAMPLE_RATE},
    BassNote, MelodyNote, Mix, MixGenerator, Sample, SongNote, SongNoteReader, SongSample,
    SongSampleGenerator, StemSample,
};
use wasm_bindgen::prelude::*;

use crate::SongMetadataView;

// --- DTOs -------------------------------------------------------------------------------

#[wasm_bindgen]
#[derive(Clone)]
pub struct PlanNoteWasm {
    /// 0 = melody, 1 = bass.
    pub kind: u8,
    pub midi: f64,
    #[wasm_bindgen(js_name = harmonyMidi)]
    pub harmony_midi: f64,
    pub duration: f64,
    pub degree: u32,
    pub octave: i32,
    #[wasm_bindgen(js_name = byteIndex)]
    pub byte_index: u64,
}

impl PlanNoteWasm {
    fn from_core(note: SongNote) -> Self {
        match note {
            SongNote::Melody(m) => Self {
                kind: 0,
                midi: m.midi,
                harmony_midi: m.harmony_midi,
                duration: m.duration,
                degree: m.degree as u32,
                octave: m.octave,
                byte_index: m.byte_index,
            },
            SongNote::Bass(b) => Self {
                kind: 1,
                midi: b.midi,
                harmony_midi: 0.0,
                duration: b.duration,
                degree: 0,
                octave: 0,
                byte_index: b.byte_index,
            },
        }
    }

    fn to_core(&self) -> Result<SongNote, JsValue> {
        match self.kind {
            0 => Ok(SongNote::Melody(MelodyNote {
                midi: self.midi,
                harmony_midi: self.harmony_midi,
                duration: self.duration,
                degree: self.degree as usize,
                octave: self.octave,
                byte_index: self.byte_index,
            })),
            1 => Ok(SongNote::Bass(BassNote {
                midi: self.midi,
                duration: self.duration,
                byte_index: self.byte_index,
            })),
            _ => Err(JsValue::from_str("invalid PlanNote.kind (expected 0 or 1)")),
        }
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct SampleWasm {
    #[wasm_bindgen(js_name = squareValue)]
    pub square_value: f32,
    #[wasm_bindgen(js_name = squareByteIndex)]
    pub square_byte_index: u64,
    #[wasm_bindgen(js_name = triangleValue)]
    pub triangle_value: f32,
    #[wasm_bindgen(js_name = triangleByteIndex)]
    pub triangle_byte_index: u64,
    pub drum: f32,
}

impl From<SongSample> for SampleWasm {
    fn from(s: SongSample) -> Self {
        Self {
            square_value: s.square.value,
            square_byte_index: s.square.byte_index,
            triangle_value: s.triangle.value,
            triangle_byte_index: s.triangle.byte_index,
            drum: 0.0,
        }
    }
}

impl SampleWasm {
    fn to_core(&self) -> Sample {
        Sample {
            song: SongSample {
                square: StemSample {
                    value: self.square_value,
                    byte_index: self.square_byte_index,
                },
                triangle: StemSample {
                    value: self.triangle_value,
                    byte_index: self.triangle_byte_index,
                },
            },
            drum: self.drum,
        }
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct MixWasm {
    pub frequency: f32,
    #[wasm_bindgen(js_name = squareByteIndex)]
    pub square_byte_index: u64,
    #[wasm_bindgen(js_name = triangleByteIndex)]
    pub triangle_byte_index: u64,
}

impl MixWasm {
    fn from_mix(m: Mix) -> Self {
        Self {
            frequency: m.frequency,
            square_byte_index: m.sample.song.square.byte_index,
            triangle_byte_index: m.sample.song.triangle.byte_index,
        }
    }
}

#[wasm_bindgen]
pub struct ConsumeNotesOutcome {
    consumed_byte_count: u32,
    #[wasm_bindgen(skip)]
    notes: Vec<PlanNoteWasm>,
}

#[wasm_bindgen]
impl ConsumeNotesOutcome {
    #[wasm_bindgen(getter, js_name = consumedByteCount)]
    pub fn consumed_byte_count(&self) -> u32 {
        self.consumed_byte_count
    }

    #[wasm_bindgen(getter)]
    pub fn notes(&self) -> Vec<PlanNoteWasm> {
        self.notes.clone()
    }
}

impl ConsumeNotesOutcome {
    pub(crate) fn new(consumed_byte_count: u32, notes: Vec<PlanNoteWasm>) -> Self {
        Self {
            consumed_byte_count,
            notes,
        }
    }
}

// --- SongNoteReader ---------------------------------------------------------------------

#[wasm_bindgen(js_name = SongNoteReader)]
pub struct WasmSongNoteReader {
    inner: SongNoteReader,
}

#[wasm_bindgen]
impl WasmSongNoteReader {
    /// Build reader from [`SongMetadataView`] (same metadata framing as the core stream APIs).
    #[wasm_bindgen(js_name = withMetadata)]
    pub fn with_metadata(metadata: &SongMetadataView) -> WasmSongNoteReader {
        WasmSongNoteReader {
            inner: SongNoteReader::new(metadata.clone_inner_metadata()),
        }
    }

    /// Append `data` to the internal byte buffer and read up to [`MAX_PLAN_CHUNK_NOTES`] notes.
    ///
    /// Returns the absolute position advance (bytes consumed from the buffer) and emitted notes.
    /// JS should advance its file-read pointer by the number of bytes it provided per call, as
    /// unconsumed bytes remain buffered for the next call.
    #[wasm_bindgen(js_name = consumeInputBytes)]
    pub fn consume_input_bytes(&mut self, data: &[u8]) -> ConsumeNotesOutcome {
        let pos_before = self.inner.position();
        self.inner.extend(data);

        let mut notes_js = Vec::with_capacity(MAX_PLAN_CHUNK_NOTES);

        while notes_js.len() < MAX_PLAN_CHUNK_NOTES {
            match self.inner.read_note() {
                None => break,
                Some(note) => notes_js.push(PlanNoteWasm::from_core(note)),
            }
        }

        let consumed = self.inner.position() - pos_before;
        ConsumeNotesOutcome::new(u32::try_from(consumed).unwrap_or(u32::MAX), notes_js)
    }
}

// --- SampleGenerator ---------------------------------------------------------------------

#[wasm_bindgen(js_name = SampleGenerator)]
pub struct WasmSampleGenerator(SongSampleGenerator);

#[wasm_bindgen]
impl WasmSampleGenerator {
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate_hz: u32) -> WasmSampleGenerator {
        WasmSampleGenerator(SongSampleGenerator::new().with_sample_rate(sample_rate_hz))
    }

    /// Same as [`SongSampleGenerator::sample_note`] for one [`SongNote`] (`PlanNoteWasm` envelope).
    #[wasm_bindgen(js_name = samplesForPlanNote)]
    pub fn samples_for_plan_note(
        &mut self,
        note: &PlanNoteWasm,
    ) -> Result<Vec<SampleWasm>, JsValue> {
        self.0.push(note.to_core()?);
        let chunk = self.0.sample();
        Ok(chunk.into_iter().map(SampleWasm::from).collect())
    }
}

// --- IterMix / MixGenerator -------------------------------------------------------------

#[wasm_bindgen(js_name = IterMix)]
pub struct WasmIterMix {
    inner: MixGenerator,
}

#[wasm_bindgen]
impl WasmIterMix {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmIterMix {
        WasmIterMix {
            inner: MixGenerator::new(),
        }
    }

    #[wasm_bindgen(js_name = resetPeak)]
    pub fn reset_peak(&mut self) {
        self.inner.reset();
    }

    #[wasm_bindgen(js_name = generateMix)]
    pub fn generate_mix(&mut self, sample: &SampleWasm) -> MixWasm {
        MixWasm::from_mix(self.inner.mix_sample(sample.to_core()))
    }
}

#[wasm_bindgen(js_name = maxPlanChunkNotes)]
pub fn wasm_max_plan_chunk_notes() -> u32 {
    MAX_PLAN_CHUNK_NOTES as u32
}

#[wasm_bindgen(js_name = chiptuneSampleRate)]
pub fn wasm_chiptune_sample_rate() -> u32 {
    SAMPLE_RATE
}
