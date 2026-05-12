'use client';

export type {
  ChiptuneSongInfo,
  ChiptunomaticWasmModule,
  SongMetadataJs,
} from './chiptunomatic-types';

/** Metadata mapping (also used by [`lib/chiptunomatic.worker`] via shared bundle). */
export { songInfoFromMetadataView } from './chiptunomatic-metadata';

/** Thrown when the worker fails; use `phase` to decide metadata vs WAV UI. */
export {
  ChiptunomaticGenerationError,
  type ChiptuneStreamingHooks,
  runChiptunomaticGeneration,
} from './chiptunomatic-worker-client';

