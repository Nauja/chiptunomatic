/// <reference lib="webworker" />

import { streamPcmChunksFromSynthesizer } from './wav-mono-i16';
import { songInfoFromMetadataView } from './chiptunomatic-metadata';
import type {
  ChiptuneSongInfo,
  ChiptunomaticWasmModule,
  IncrementalSynthesizerHandle,
  SongMetadataJs,
} from './chiptunomatic-types';
import type {
  MainWorkerMessage,
  MainGenerateMessage,
  MainGetModesMessage,
  WorkerGenerateReply,
} from './chiptunomatic-worker-messages';

const PCM_CHUNK_SAMPLES = 8192;
const INPUT_CHUNK_BYTES = 4096;

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

async function handleGetModes(msg: MainGetModesMessage) {
  try {
    const glue = await loadGlue(msg.wasmScriptHref);
    const names = glue.musicModeNames().split(',');
    post({ type: 'modes', id: msg.id, names });
  } catch {
    post({ type: 'modes', id: msg.id, names: [] });
  }
}

self.onmessage = async (evt: MessageEvent<MainWorkerMessage>) => {
  const msg = evt.data;
  if (msg.type === 'get_modes') { await handleGetModes(msg); return; }
  if (msg.type !== 'generate') return;

  const { id, wasmScriptHref, fileName, buffer, mode } = msg;

  let metadataView: SongMetadataJs | undefined;
  let synth: IncrementalSynthesizerHandle | undefined;

  try {
    const glue = await loadGlue(wasmScriptHref);
    const dataByteLen = BigInt(buffer.byteLength);

    let info: ChiptuneSongInfo;
    try {
      metadataView = glue.createSongMetadataFromStringWithMode(fileName, dataByteLen, mode ?? 'chiptune');
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
      post({ type: 'wav_begin', id, sampleRateHz: hz, pcmSampleCapacityHint });

      synth = glue.IncrementalSynthesizer.withMetadata(metadataView);

      const totalPcmSamples = streamPcmChunksFromSynthesizer(
        synth,
        input,
        INPUT_CHUNK_BYTES,
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
    synth?.free();
    metadataView?.free();
  }
};

export {};
