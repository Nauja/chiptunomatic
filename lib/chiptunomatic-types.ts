/** Plain song fields for React (from WASM metadata). */
export interface ChiptuneSongInfo {
  rootNoteName: string;
  bpm: number;
  chordDescription: string;
  /** Wall-clock length implied by tempo and total beats. */
  totalDurationSec: number;
  totalBeats: string;
}

/** Getters produced by wasm-bindgen for `SongMetadataView`. */
export interface SongMetadataJs {
  readonly rootSemitone: number;
  readonly bpm: number;
  readonly chordDescription: string;
  readonly totalDuration: number;
  readonly totalBeats: bigint;
  // Drum pattern
  readonly drumKick: Uint8Array;
  readonly drumSnare: Uint8Array;
  readonly drumHat: Uint8Array;
  readonly openHatStep: number | undefined;
  /** Full 16-byte MD5 seed; used by drum synthesis for per-step color. */
  readonly seed: Uint8Array;
  readonly rngSeed: bigint;
  readonly sixteenth: number;
  free(): void;
}

/** Static `SongNoteReader.withMetadata` (wasm-pack `--target web`). */
export interface ChiptuneSongNoteReaderHandle {
  consumeInputBytes(data: Uint8Array): ChiptuneConsumeNotesOutcomeHandle;
  free(): void;
}

/** Return value of [`ChiptuneSongNoteReaderHandle.consumeInputBytes`]. */
export interface ChiptuneConsumeNotesOutcomeHandle {
  readonly consumedByteCount: number;
  readonly notes: ChiptunePlanNoteHandle[];
  free(): void;
}

/** `PlanNoteWasm` backing object (owned by WASM). */
export interface ChiptunePlanNoteHandle {
  free(): void;
}

/** Wasm-pack glue shape for `SampleGenerator`. */
export interface ChiptunomaticSampleGenerator {
  samplesForPlanNote(note: ChiptunePlanNoteHandle): ChiptuneSampleHandle[];
  free(): void;
}

/** `SampleWasm`; release after use. */
export interface ChiptuneSampleHandle {
  drum: number;
  free(): void;
}

/** `MixWasm`; release after mixing. */
export interface ChiptuneMixHandle {
  readonly frequency: number;
  free(): void;
}

/** Js name `IterMix` — wraps [`MixGenerator`]. */
export interface ChiptunomaticIterMix {
  generateMix(sample: ChiptuneSampleHandle): ChiptuneMixHandle;
  free(): void;
}

/** Shape of wasm-pack `--target web` glue (`public/chiptunomatic-wasm/chiptunomatic_wasm.js`). */
export interface ChiptunomaticWasmModule {
  default: (moduleOrPath?: unknown) => Promise<void>;
  chiptuneSampleRate(): number;
  SongNoteReader: {
    withMetadata(metadata: SongMetadataJs): ChiptuneSongNoteReaderHandle;
  };
  SampleGenerator: new (sampleRateHz: number) => ChiptunomaticSampleGenerator;
  IterMix: new () => ChiptunomaticIterMix;
  createSongMetadataFromString(
    name: string,
    dataByteLen: bigint,
  ): SongMetadataJs;
}
