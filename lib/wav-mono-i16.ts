/**
 * Mono 16‑bit PCM little‑endian WAV (RIFF).
 */
export function pcm16MonoToWav(pcm: Int16Array, sampleRate: number): Uint8Array {
  const dataBytes = pcm.length * 2;
  const out = new Uint8Array(44 + dataBytes);
  const dv = new DataView(out.buffer);

  out.set([0x52, 0x49, 0x46, 0x46], 0); // RIFF
  dv.setUint32(4, 36 + dataBytes, true);
  out.set([0x57, 0x41, 0x56, 0x45], 8); // WAVE

  out.set([0x66, 0x6d, 0x74, 0x20], 12); // fmt
  dv.setUint32(16, 16, true);
  dv.setUint16(20, 1, true); // PCM
  dv.setUint16(22, 1, true); // mono
  dv.setUint32(24, sampleRate, true);
  dv.setUint32(28, sampleRate * 2, true);
  dv.setUint32(32, 2, true); // block align
  dv.setUint16(34, 16, true);

  out.set([0x64, 0x61, 0x74, 0x61], 36); // data
  dv.setUint32(40, dataBytes, true);

  out.set(new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength), 44);
  return out;
}

/** Concatenate LE mono PCM chunk payloads (`wav_pcm_chunk` order) before [`pcm16MonoToWav`]. */
export function concatenatePcmChunksToInt16(chunks: Uint8Array[]): Int16Array {
  let bytes = 0;
  for (const c of chunks) {
    bytes += c.length;
  }
  const evenLen = bytes & ~1;
  const merged = new Uint8Array(evenLen);
  let o = 0;
  for (const c of chunks) {
    const n = Math.min(c.length, evenLen - o);
    merged.set(c.subarray(0, n), o);
    o += n;
    if (o >= evenLen) break;
  }
  return new Int16Array(
    merged.buffer,
    merged.byteOffset,
    merged.byteLength >>> 1,
  );
}

function mixFrequencyToPcmSample(frequency: number): number {
  return Math.max(-32768, Math.min(32767, Math.round(frequency * 32767.0)));
}

export interface WasmNoteLike {
  free(): void;
}

export interface WasmSampleLike {
  drum: number;
  free(): void;
}

export interface DrumSampleGeneratorLike {
  nextSample(): number;
}

export interface WasmMixLike {
  readonly frequency: number;
  free(): void;
}

export interface WasmConsumeNotesOutcomeLike {
  readonly consumedByteCount: number;
  readonly notes: WasmNoteLike[];
  free(): void;
}

export interface WasmSampleGeneratorLike {
  samplesForPlanNote(note: WasmNoteLike): WasmSampleLike[];
  free(): void;
}

export interface WasmSampleGeneratorStatics {
  withMetadata(metadata: { free(): void }, sampleRateHz: number): WasmSampleGeneratorLike;
}

export interface WasmIterMixLike {
  generateMix(sample: WasmSampleLike): WasmMixLike;
  free(): void;
}

export interface WasmSongNoteReaderLike {
  consumeInputBytes(data: Uint8Array): WasmConsumeNotesOutcomeLike;
  free(): void;
}

export interface WasmSongNoteReaderStatics {
  withMetadata(metadata: { free(): void }): WasmSongNoteReaderLike;
}

/** Metadata view with wall‑clock length (for PCM preallocation). */
export interface WasmMetadataBorrow {
  readonly totalDuration: number;
  free(): void;
}

export function forEachMixedPcmSample(
  readerStatic: WasmSongNoteReaderStatics,
  metadataView: WasmMetadataBorrow,
  rawInput: Uint8Array,
  SampleGeneratorStatics: WasmSampleGeneratorStatics,
  IterMixCtor: new () => WasmIterMixLike,
  sampleHz: number,
  drumGen: DrumSampleGeneratorLike | undefined,
  onSample: (pcmI16: number) => void,
): void {
  let reader: WasmSongNoteReaderLike | undefined;
  let sampleGen: WasmSampleGeneratorLike | undefined;
  let mixer: WasmIterMixLike | undefined;

  try {
    reader = readerStatic.withMetadata(metadataView);
    sampleGen = SampleGeneratorStatics.withMetadata(metadataView, sampleHz);
    mixer = new IterMixCtor();

    let sliceStart = 0;

    for (;;) {
      const remainder = rawInput.subarray(sliceStart);
      const outcome = reader.consumeInputBytes(remainder);
      try {
        const consumed = outcome.consumedByteCount;
        const notes = outcome.notes;
        sliceStart += consumed;

        if (notes.length === 0 && consumed === 0) {
          break;
        }

        for (let ni = 0; ni < notes.length; ni++) {
          const note = notes[ni];
          try {
            const stems = sampleGen.samplesForPlanNote(note);
            for (let si = 0; si < stems.length; si++) {
              const s = stems[si];
              try {
                if (drumGen !== undefined) {
                  s.drum = drumGen.nextSample();
                }
                const m = mixer.generateMix(s);
                try {
                  onSample(mixFrequencyToPcmSample(m.frequency));
                } finally {
                  m.free();
                }
              } finally {
                s.free();
              }
            }
          } finally {
            note.free();
          }
        }
      } finally {
        outcome.free();
      }
    }
  } finally {
    reader?.free();
    mixer?.free();
    sampleGen?.free();
  }
}

/** Collect all PCM into one buffer (growable heap). */
export function pcm16SamplesFromIncrementalPlan(
  readerStatic: WasmSongNoteReaderStatics,
  metadataView: WasmMetadataBorrow,
  rawInput: Uint8Array,
  sampleHz: number,
  SampleGeneratorStatics: WasmSampleGeneratorStatics,
  IterMixCtor: new () => WasmIterMixLike,
  drumGen: DrumSampleGeneratorLike | undefined,
): Int16Array {
  const totalDurSec = metadataView.totalDuration;
  let pcmCap =
    totalDurSec > 0 ? Math.ceil(totalDurSec * sampleHz + sampleHz / 10) : sampleHz * 8;

  pcmCap = Math.max(sampleHz, pcmCap);

  let pcm = new Int16Array(pcmCap);
  let writeHead = 0;

  const ensure = (extra: number) => {
    while (writeHead + extra > pcm.length) {
      let n = pcm.length * 2;
      if (n < writeHead + extra) {
        n = writeHead + extra + 8192;
      }
      const next = new Int16Array(n);
      next.set(pcm.subarray(0, writeHead));
      pcm = next;
    }
  };

  forEachMixedPcmSample(
    readerStatic,
    metadataView,
    rawInput,
    SampleGeneratorStatics,
    IterMixCtor,
    sampleHz,
    drumGen,
    (sample) => {
      ensure(1);
      pcm[writeHead++] = sample;
    },
  );

  return pcm.subarray(0, writeHead);
}

/** Stream PCM batches as transferable byte buffers (little‑endian mono i16). */
export function streamPcmChunksFromIncrementalPlan(
  readerStatic: WasmSongNoteReaderStatics,
  metadataView: WasmMetadataBorrow,
  rawInput: Uint8Array,
  sampleHz: number,
  SampleGeneratorStatics: WasmSampleGeneratorStatics,
  IterMixCtor: new () => WasmIterMixLike,
  drumGen: DrumSampleGeneratorLike | undefined,
  maxSamplesBeforeFlush: number,
  onChunk: (pcmBytesOwned: Uint8Array) => void,
): number {
  const scratchCap = Math.max(256, maxSamplesBeforeFlush);
  const scratch = new Int16Array(scratchCap);
  let scratchUsed = 0;
  let totalSamples = 0;

  const flush = () => {
    if (scratchUsed === 0) return;
    const copy = scratch.slice(0, scratchUsed);
    onChunk(
      new Uint8Array(copy.buffer, copy.byteOffset, copy.byteLength),
    );
    scratchUsed = 0;
  };

  forEachMixedPcmSample(
    readerStatic,
    metadataView,
    rawInput,
    SampleGeneratorStatics,
    IterMixCtor,
    sampleHz,
    drumGen,
    (sample) => {
      scratch[scratchUsed++] = sample;
      totalSamples++;
      if (scratchUsed >= scratchCap) {
        flush();
      }
    },
  );

  flush();
  return totalSamples;
}
