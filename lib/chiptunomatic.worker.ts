/// <reference lib="webworker" />

import {
  streamPcmChunksFromIncrementalPlan,
} from './wav-mono-i16';
import { DrumSampleGenerator } from './drum-synth';
import { songInfoFromMetadataView } from './chiptunomatic-metadata';
import type {
  ChiptuneSongInfo,
  ChiptunomaticWasmModule,
  SongMetadataJs,
} from './chiptunomatic-types';
import type {
  MainGenerateMessage,
  WorkerGenerateReply,
} from './chiptunomatic-worker-messages';

const PCM_CHUNK_SAMPLES = 8192;

const gluePromisesByHref = new Map<string, Promise<ChiptunomaticWasmModule>>();

function loadGlue(scriptHref: string): Promise<ChiptunomaticWasmModule> {
  let p = gluePromisesByHref.get(scriptHref);
  if (!p) {
    p = (async () => {
      const mod = (await import(
        /* webpackIgnore: true */
        scriptHref
      )) as ChiptunomaticWasmModule;
      await mod.default();
      return mod;
    })().catch((e) => {
      gluePromisesByHref.delete(scriptHref);
      throw e;
    });
    gluePromisesByHref.set(scriptHref, p);
  }
  return p;
}

function post(reply: WorkerGenerateReply, transfer: Transferable[] = []) {
  (self as DedicatedWorkerGlobalScope).postMessage(reply, transfer);
}

self.onmessage = async (evt: MessageEvent<MainGenerateMessage>) => {
  const msg = evt.data;
  if (msg.type !== 'generate') return;

  const { id, wasmScriptHref, fileName, buffer } = msg;

  let metadataView: SongMetadataJs | undefined;

  try {
    const glue = await loadGlue(wasmScriptHref);
    const dataByteLen = BigInt(buffer.byteLength);

    let info: ChiptuneSongInfo;
    try {
      metadataView = glue.createSongMetadataFromString(fileName, dataByteLen);
      info = songInfoFromMetadataView(metadataView);
    } catch {
      post({
        type: 'error',
        id,
        phase: 'metadata',
        message:
          buffer.byteLength === 0
            ? 'input is empty'
            : 'Could not load song metadata',
      });
      return;
    }

    post({ type: 'metadata', id, info });

    try {
      const input = new Uint8Array(buffer);
      const hz = glue.chiptuneSampleRate();

      const pcmSampleCapacityHint = Math.max(
        1,
        Math.ceil(metadataView.totalDuration * hz + hz / 10),
      );
      post({
        type: 'wav_begin',
        id,
        sampleRateHz: hz,
        pcmSampleCapacityHint,
      });

      const drumGen = new DrumSampleGenerator(metadataView, hz);

      const totalPcmSamples = streamPcmChunksFromIncrementalPlan(
        glue.SongNoteReader,
        metadataView,
        input,
        hz,
        glue.SampleGenerator,
        glue.IterMix,
        drumGen,
        PCM_CHUNK_SAMPLES,
        (pcmBytesOwned) => {
          const u = pcmBytesOwned.slice();
          post({ type: 'wav_pcm_chunk', id, pcm: u.buffer }, [u.buffer]);
        },
      );

      post({ type: 'wav_end', id, totalPcmSamples });
    } catch (e: unknown) {
      const message =
        e instanceof Error ? e.message : 'Could not generate audio';
      post({ type: 'error', id, phase: 'wav', message });
    }
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e);
    post({ type: 'error', id, phase: 'metadata', message });
  } finally {
    metadataView?.free();
  }
};

export {};
