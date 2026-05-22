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

export interface IncrementalSynthesizerLike {
  consumeBytes(data: Uint8Array): Uint8Array;
  free(): void;
}

/**
 * Stream PCM from an [`IncrementalSynthesizerLike`] as transferable byte buffers
 * (little-endian mono i16). Input is fed in chunks of `inputChunkSize` bytes; PCM is flushed
 * to `onChunk` whenever `maxSamplesBeforeFlush` samples have accumulated.
 *
 * Returns the total number of PCM samples produced.
 */
export function streamPcmChunksFromSynthesizer(
  synth: IncrementalSynthesizerLike,
  input: Uint8Array,
  inputChunkSize: number,
  maxSamplesBeforeFlush: number,
  onChunk: (pcmBytesOwned: Uint8Array) => void,
): number {
  const maxBytes = maxSamplesBeforeFlush * 2; // i16 = 2 bytes per sample
  const parts: Uint8Array[] = [];
  let bufferedBytes = 0;
  let totalSamples = 0;

  const flush = () => {
    if (bufferedBytes === 0) return;
    const merged = new Uint8Array(bufferedBytes);
    let pos = 0;
    for (const p of parts) {
      merged.set(p, pos);
      pos += p.length;
    }
    parts.length = 0;
    bufferedBytes = 0;
    onChunk(merged);
  };

  const chunkSize = Math.max(1, inputChunkSize);
  for (let offset = 0; offset < input.length; offset += chunkSize) {
    const slice = input.subarray(offset, Math.min(offset + chunkSize, input.length));
    const pcm = synth.consumeBytes(slice);
    if (pcm.length === 0) continue;
    // Copy out of WASM memory before any further WASM calls.
    parts.push(pcm.slice());
    bufferedBytes += pcm.length;
    totalSamples += pcm.length >>> 1;
    if (bufferedBytes >= maxBytes) flush();
  }
  flush();
  return totalSamples;
}
