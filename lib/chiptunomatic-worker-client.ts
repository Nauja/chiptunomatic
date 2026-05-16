"use client";

import type { ChiptuneSongInfo } from "./chiptunomatic-types";
import { concatenatePcmChunksToInt16, pcm16MonoToWav } from "./wav-mono-i16";
import type {
  MainGenerateMessage,
  MainGetModesMessage,
  WorkerGenerateReply,
} from "./chiptunomatic-worker-messages";

let worker: Worker | null = null;

let serial = 1;

function wasmGlueHref(): string {
  const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? "";
  const path = `${basePath}/chiptunomatic-wasm/chiptunomatic_wasm.js`;
  if (typeof window === "undefined") {
    throw new Error("Chiptunomatic generation runs only in the browser.");
  }
  return new URL(path, window.location.origin).href;
}

/** Optional observers while PCM chunks stream from the worker (before [`wav_end`]). */
export interface ChiptuneStreamingHooks {
  /** Little-endian mono i16 payloads in synthesis order — concatenate into the WAV data segment. */
  onPcmChunk?: (pcmBytes: Uint8Array) => void;
  /**
   * Fully valid mono‑16 WAV after each coarse slice of PCM (built on the UI thread).
   * Defaults to firing about once per second (`intermediateEveryPcmSamples`).
   */
  onIntermediateMono16Wav?: (wavBytes: Uint8Array) => void;
  /** Threshold for **`onIntermediateMono16Wav`**; defaults to the sample rate (≈ one second). */
  intermediateEveryPcmSamples?: number;
}

type WavStreamAccumulator = {
  sampleRateHz: number;
  pcmParts: Uint8Array[];
  samplesSinceIntermediate: number;
  intermediateThreshold: number;
};

type PendingHandlers = {
  fireSongInfoOnce: (info: ChiptuneSongInfo) => void;
  finalizeWav: (u: Uint8Array) => void;
  bail: (e: Error) => void;
  streaming?: ChiptuneStreamingHooks | undefined;
  wavStream: WavStreamAccumulator | null;
};

const pendingById = new Map<number, PendingHandlers>();
const pendingModesById = new Map<number, (names: string[]) => void>();

export class ChiptunomaticGenerationError extends Error {
  readonly phase: "metadata" | "wav";

  constructor(message: string, phase: "metadata" | "wav") {
    super(message);
    this.name = "ChiptunomaticGenerationError";
    this.phase = phase;
  }
}

function maybeIntermediateWavFromStream(p: PendingHandlers): void {
  const s = p.wavStream;
  const hook = p.streaming?.onIntermediateMono16Wav;
  if (!s || !hook) return;

  if (s.samplesSinceIntermediate >= s.intermediateThreshold) {
    const pcmCombined = concatenatePcmChunksToInt16(s.pcmParts);
    hook(pcm16MonoToWav(pcmCombined, s.sampleRateHz));
    s.samplesSinceIntermediate = 0;
  }
}

function appendPcmChunk(p: PendingHandlers, pcm: ArrayBuffer): void {
  const s = p.wavStream;
  if (!s) {
    return;
  }

  const u8 = new Uint8Array(pcm);
  const part = new Uint8Array(u8.length);
  part.set(u8);
  s.pcmParts.push(part);

  const samplesAdded = u8.byteLength >>> 1;
  s.samplesSinceIntermediate += samplesAdded;

  try {
    p.streaming?.onPcmChunk?.(u8.slice());
  } catch {
    // User hook errors should not wedge the synthesizer listener.
  }

  try {
    maybeIntermediateWavFromStream(p);
  } catch {
    //
  }
}

function installWorkerListener(w: Worker) {
  w.addEventListener("message", (ev: MessageEvent<WorkerGenerateReply>) => {
    const d = ev.data;

    if (d.type === "modes") {
      const resolve = pendingModesById.get(d.id);
      if (resolve) {
        pendingModesById.delete(d.id);
        resolve(d.names);
      }
      return;
    }

    const p = pendingById.get(d.id);
    if (!p) return;

    if (d.type === "metadata") {
      p.fireSongInfoOnce(d.info);
      return;
    }

    if (d.type === "wav_begin") {
      const hooks = p.streaming;
      const interval = Math.max(
        4096,
        hooks?.intermediateEveryPcmSamples ?? d.sampleRateHz,
      );
      p.wavStream = {
        sampleRateHz: d.sampleRateHz,
        pcmParts: [],
        samplesSinceIntermediate: 0,
        intermediateThreshold: interval,
      };
      return;
    }

    if (d.type === "wav_pcm_chunk") {
      appendPcmChunk(p, d.pcm);
      return;
    }

    if (d.type === "wav_end") {
      const stream = p.wavStream;
      if (!stream) {
        pendingById.delete(d.id);
        p.bail(
          new ChiptunomaticGenerationError(
            "Worker finished WAV stream without wav_begin",
            "wav",
          ),
        );
        return;
      }

      pendingById.delete(d.id);

      const pcmMerged = concatenatePcmChunksToInt16(stream.pcmParts);
      void d.totalPcmSamples;
      const wav = pcm16MonoToWav(pcmMerged, stream.sampleRateHz);
      try {
        p.streaming?.onIntermediateMono16Wav?.(wav);
      } catch {
        //
      }

      try {
        p.finalizeWav(wav);
      } catch {
        //
      }
      return;
    }

    if (d.type === "error") {
      pendingById.delete(d.id);
      p.wavStream = null;
      p.bail(
        new ChiptunomaticGenerationError(
          d.message || "generation failed",
          d.phase,
        ),
      );
    }
  });
}

function acquireWorker(): Worker {
  if (worker) return worker;
  worker = new Worker(new URL("./chiptunomatic.worker.ts", import.meta.url), {
    type: "module",
    name: "chiptunomatic",
  });
  installWorkerListener(worker);
  worker.addEventListener("error", () => {
    pendingById.forEach(({ bail }) =>
      bail(new Error("Dedicated worker crashed — refresh the page")),
    );
    pendingById.clear();
    worker = null;
  });
  return worker;
}

/** Returns all mode names in WASM integer order, obtained from the chiptunomatic crate via the worker. */
export function getMusicModes(): Promise<string[]> {
  if (typeof window === "undefined") return Promise.resolve([]);
  const id = serial++;
  const w = acquireWorker();
  return new Promise<string[]>((resolve) => {
    pendingModesById.set(id, resolve);
    const msg: MainGetModesMessage = {
      type: "get_modes",
      id,
      wasmScriptHref: wasmGlueHref(),
    };
    try {
      w.postMessage(msg);
    } catch {
      pendingModesById.delete(id);
      resolve([]);
    }
  });
}

/**
 * Streams metadata from the worker, then transports mono PCM in chunks and returns a finished WAV blob.
 */
export async function runChiptunomaticGeneration(
  file: File,
  onSongInfo: (info: ChiptuneSongInfo) => void,
  options?: {
    signal?: AbortSignal;
    streaming?: ChiptuneStreamingHooks;
    mode?: string;
  },
): Promise<Uint8Array> {
  if (typeof window === "undefined") {
    throw new Error("Chiptunomatic generation runs only in the browser.");
  }

  const buf = await file.arrayBuffer();

  if (options?.signal?.aborted) {
    throw new DOMException("Aborted", "AbortError");
  }

  const id = serial++;
  const w = acquireWorker();
  const wasmScriptHref = wasmGlueHref();
  const ac = options?.signal;

  return new Promise<Uint8Array>((resolve, reject) => {
    const cleanupAbort =
      ac == null
        ? () => {
            //
          }
        : () => {
            ac.removeEventListener("abort", onAbort);
          };

    let songInfoEmitted = false;

    const bail = (e: Error) => {
      cleanupAbort();
      pendingById.delete(id);
      reject(e);
    };

    const onAbort = () => bail(new DOMException("Aborted", "AbortError"));

    if (ac) {
      ac.addEventListener("abort", onAbort);
    }

    pendingById.set(id, {
      fireSongInfoOnce: (info) => {
        if (songInfoEmitted) return;
        songInfoEmitted = true;
        onSongInfo(info);
      },
      finalizeWav: (u8) => {
        cleanupAbort();
        resolve(u8);
      },
      bail,
      streaming: options?.streaming,
      wavStream: null,
    });

    const msg: MainGenerateMessage = {
      type: "generate",
      id,
      wasmScriptHref,
      fileName: file.name,
      buffer: buf,
      mode: options?.mode ?? 'chiptune',
    };
    try {
      w.postMessage(msg, [buf]);
    } catch (e) {
      bail(e instanceof Error ? e : new Error(String(e)));
    }
  });
}
