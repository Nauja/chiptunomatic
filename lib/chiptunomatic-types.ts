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

/** WASM `IncrementalSynthesizer` instance — feed raw bytes, receive back LE i16 PCM. */
export interface IncrementalSynthesizerHandle {
  /** Feed `data` bytes; returns LE i16 PCM bytes generated from them. */
  consumeBytes(data: Uint8Array): Uint8Array;
  setMasterVolume(volume: number): void;
  setMasterMuted(muted: boolean): void;
  setVoiceOutput(volume: number, muted: boolean, solo: boolean): void;
  setSquareOutput(volume: number, muted: boolean, solo: boolean): void;
  setTriangleOutput(volume: number, muted: boolean, solo: boolean): void;
  setNoiseOutput(volume: number, muted: boolean, solo: boolean): void;
  free(): void;
}

/** Static side of `IncrementalSynthesizer` — created via `withMetadata`. */
export interface IncrementalSynthesizerStatics {
  withMetadata(metadata: SongMetadataJs): IncrementalSynthesizerHandle;
}

/** Shape of wasm-pack `--target web` glue (`public/chiptunomatic-wasm/chiptunomatic_wasm.js`). */
export interface ChiptunomaticWasmModule {
  default: (moduleOrPath?: unknown) => Promise<void>;
  chiptuneSampleRate(): number;
  IncrementalSynthesizer: IncrementalSynthesizerStatics;
  createSongMetadataFromString(
    name: string,
    dataByteLen: bigint,
  ): SongMetadataJs;
  createSongMetadataFromStringWithMode(
    name: string,
    dataByteLen: bigint,
    mode: string,
  ): SongMetadataJs;
  /** Returns comma-separated mode names. */
  musicModeNames(): string;
}
