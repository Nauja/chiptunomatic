import type { ChiptuneSongInfo } from './chiptunomatic-types';

/** Main thread → worker. */
export interface MainGenerateMessage {
  type: 'generate';
  id: number;
  wasmScriptHref: string;
  fileName: string;
  buffer: ArrayBuffer;
}

/** Worker → main: progressive WAV is shipped as PCM chunks, then finalized on [`wav_end`]. */
export type WorkerGenerateReply =
  | { type: 'metadata'; id: number; info: ChiptuneSongInfo }
  | {
      type: 'wav_begin';
      id: number;
      sampleRateHz: number;
      /** Hint for UX / buffer sizing (estimated from song metadata). May exceed actual PCM. */
      pcmSampleCapacityHint: number;
    }
  | { type: 'wav_pcm_chunk'; id: number; pcm: ArrayBuffer }
  | { type: 'wav_end'; id: number; totalPcmSamples: number }
  | { type: 'error'; id: number; phase: 'metadata' | 'wav'; message: string };
