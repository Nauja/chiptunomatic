/**
 * TypeScript port of chiptunomatic's drum synthesis pipeline
 * (drum.rs + synth.rs), matching the CLI's StdDrumSampleGenerator output.
 */

export interface DrumPatternMetadata {
  readonly drumKick: Uint8Array;
  readonly drumSnare: Uint8Array;
  readonly drumHat: Uint8Array;
  readonly openHatStep: number | undefined;
  /** Full 16-byte MD5 seed; color = seed[stepIdx % 8] (matches drum.rs sample_step). */
  readonly seed: Uint8Array;
  readonly rngSeed: bigint;
  readonly sixteenth: number;
}

/** Mulberry32 PRNG — deterministic substitute for rand::rngs::StdRng seeded from rng_seed. */
function mulberry32(seed: number): () => number {
  let s = seed >>> 0;
  return (): number => {
    s = (s + 0x6d2b79f5) >>> 0;
    let z = Math.imul(s ^ (s >>> 15), 1 | s);
    z = (z + Math.imul(z ^ (z >>> 7), 61 | z)) >>> 0;
    return ((z ^ (z >>> 14)) >>> 0) / 4294967296;
  };
}

function squareWave(
  sampleRate: number, freq: number, dur: number, amp: number, duty: number,
): Float32Array {
  const n = Math.floor(sampleRate * dur);
  const threshold = 2.0 * duty - 1.0;
  const twoPiF = 2.0 * Math.PI * freq;
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const s = Math.sin(twoPiF * (i / sampleRate));
    out[i] = amp * (s - threshold > 0 ? 1 : s - threshold < 0 ? -1 : 0);
  }
  return out;
}

function noiseBurst(sampleRate: number, rng: () => number, dur: number, amp: number): Float32Array {
  const n = Math.floor(sampleRate * dur);
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    out[i] = amp * (rng() * 2.0 - 1.0);
  }
  return out;
}

/**
 * ADSR envelope matching synth::envelope (linspace semantics: linspace(a, b, 1) = [a]).
 */
function applyEnvelope(
  sampleRate: number,
  samples: Float32Array,
  attack: number, decay: number, sustain: number, release: number,
): Float32Array {
  const n = samples.length;
  const a = Math.min(Math.floor(attack * sampleRate), n);
  const d = Math.min(Math.floor(decay * sampleRate), Math.max(0, n - a));
  const r = Math.min(Math.floor(release * sampleRate), n);
  const sLen = Math.max(0, n - a - d - r);

  const env = new Float32Array(a + d + sLen + r);
  let pos = 0;

  for (let i = 0; i < a; i++, pos++) {
    env[pos] = a <= 1 ? 0.0 : i / (a - 1);
  }
  for (let i = 0; i < d; i++, pos++) {
    env[pos] = d <= 1 ? 1.0 : 1.0 + (sustain - 1.0) * (i / (d - 1));
  }
  for (let i = 0; i < sLen; i++, pos++) {
    env[pos] = sustain;
  }
  for (let i = 0; i < r; i++, pos++) {
    env[pos] = r <= 1 ? sustain : sustain * (1.0 - i / (r - 1));
  }

  const out = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    out[i] = samples[i] * (env[i] ?? 0);
  }
  return out;
}

function overlayInto(src: Float32Array, dst: Float32Array): void {
  const len = Math.min(src.length, dst.length);
  for (let i = 0; i < len; i++) {
    dst[i] += src[i];
  }
}

/**
 * Port of chiptunomatic's SampleDrumSteps iterator.
 * Pre-generates samples for one 16th-note step at a time, looping the pattern.
 * The RNG state advances across steps (not reset per loop), matching the CLI.
 */
export class DrumSampleGenerator {
  private readonly rng: () => number;
  private readonly pattern: DrumPatternMetadata;
  private readonly sampleRate: number;
  private readonly stepLen: number;

  private currentStepSamples: Float32Array;
  private stepCursor: number = 0;
  private stepIdx: number = 0;

  constructor(pattern: DrumPatternMetadata, sampleRate: number) {
    this.rng = mulberry32(Number(pattern.rngSeed) >>> 0);
    this.pattern = pattern;
    this.sampleRate = sampleRate;
    this.stepLen = Math.floor(sampleRate * pattern.sixteenth);
    this.currentStepSamples = this.generateStep(0);
  }

  nextSample(): number {
    if (this.stepLen === 0) return 0;
    const s = this.currentStepSamples[this.stepCursor] ?? 0;
    if (++this.stepCursor >= this.stepLen) {
      this.stepCursor = 0;
      this.stepIdx = (this.stepIdx + 1) % 16;
      this.currentStepSamples = this.generateStep(this.stepIdx);
    }
    return s;
  }

  private generateStep(idx: number): Float32Array {
    const { drumKick, drumSnare, drumHat, openHatStep, seed, sixteenth } = this.pattern;
    const { sampleRate, rng } = this;

    const totalSamples = Math.floor(sampleRate * sixteenth);
    if (totalSamples === 0) return new Float32Array(0);

    const color = seed[idx % 8] ?? 0;
    const dst = new Float32Array(totalSamples);

    if (drumKick[idx]) {
      const freq = 60 + (color % 12);
      const dur = Math.min(0.12, sixteenth * 2);
      const raw = squareWave(sampleRate, freq, dur, 0.4, 0.2);
      overlayInto(applyEnvelope(sampleRate, raw, 0.002, 0.09, 0.0, 0.01), dst);
    }

    if (drumSnare[idx]) {
      const accent = idx === 4 || idx === 12;
      const amp = accent ? 0.22 : 0.09;
      const dur = Math.min(0.07, sixteenth);
      const raw = noiseBurst(sampleRate, rng, dur, amp);
      overlayInto(applyEnvelope(sampleRate, raw, 0.001, 0.055, 0.0, 0.015), dst);
    }

    if (drumHat[idx]) {
      if (openHatStep === idx) {
        const dur = Math.min(0.09, sixteenth * 3);
        const raw = noiseBurst(sampleRate, rng, dur, 0.12);
        overlayInto(applyEnvelope(sampleRate, raw, 0.001, 0.07, 0.25, 0.03), dst);
      } else {
        const amp = idx % 2 === 0 ? 0.10 : 0.05;
        const dur = Math.min(0.018, sixteenth * 0.45);
        overlayInto(noiseBurst(sampleRate, rng, dur, amp), dst);
      }
    }

    return dst;
  }
}
