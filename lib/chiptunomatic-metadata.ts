import type { ChiptuneSongInfo, SongMetadataJs } from './chiptunomatic-types';

const NOTE_NAMES = [
  'C',
  'C#',
  'D',
  'D#',
  'E',
  'F',
  'F#',
  'G',
  'G#',
  'A',
  'A#',
  'B',
] as const;

/** Maps bindgen `SongMetadataView` accessors into [`ChiptuneSongInfo`] (DOM-free for workers). */
export function songInfoFromMetadataView(view: SongMetadataJs): ChiptuneSongInfo {
  const root = view.rootSemitone;
  const rootNoteName = root >= 0 && root < 12 ? NOTE_NAMES[root] : '?';
  return {
    rootNoteName,
    bpm: view.bpm,
    chordDescription: view.chordDescription,
    totalDurationSec: view.totalDuration,
    totalBeats: view.totalBeats.toString(),
  };
}
