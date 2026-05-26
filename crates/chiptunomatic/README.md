# chiptunomatic

The core synthesis library. Turns any byte sequence into a deterministic chiptune: the same file name and byte length always produce the same music.

The library is `no_std` + `alloc` with an optional `std` layer, so it works on native targets and in WebAssembly (`wasm32-unknown-unknown`).

## Adding the dependency

```toml
[dependencies]
chiptunomatic = "0.3"
```

For WebAssembly in a JS host, enable the `js` feature so `getrandom` uses its JS backend:

```toml
[dependencies]
chiptunomatic = { version = "0.3", default-features = false, features = ["js", "std"] }
```

## Quick start

```rust
use chiptunomatic::{random::StdRandom, Chiptunomatic};

// Build an instance with all built-in modes and a seeded RNG.
let mut chip = Chiptunomatic::default()
    .with_default_plugins()
    .with_random(Box::new(StdRandom::new()));

// Load metadata from a file (derives seed from filename + byte length).
let meta = chip.load_song_metadata_from_path("my_file.bin")?;
println!("BPM: {}  duration: {}", meta.bpm, meta.total_duration_str);

// Collect every mixed sample into a Vec<Sample>.
let samples: Vec<_> = chip.samples();
for s in &samples {
    // s.value is the mixed mono f32 in roughly [-1, 1].
    // s.stems gives per-stem (voice, square, triangle, noise) floats.
}
```

## Metadata

`SongMetadata` is derived deterministically from a seed (typically the file name) and a byte length. It carries all musical decisions for the track.

```rust
// Non-mutating: compute without changing the instance state.
let meta = chip.song_metadata_from_path("file.bin")?;

// Mutating: compute and install (required before synthesis).
let meta = chip.load_song_metadata_from_path("file.bin")?;

// From a string name + known byte count — works on wasm32 too.
let meta = chip.load_song_metadata_from_string("my_song", 102_400)?;

// From raw seed bytes.
let meta = chip.load_song_metadata_from_seed(b"custom_seed", 102_400)?;
```

Key fields on `SongMetadata`:

| Field | Type | Description |
|---|---|---|
| `bpm` | `i32` | Beats per minute |
| `root_semitone` | `u8` | Root note (0 = C) |
| `chord_description` | `String` | Human-readable chord loop (e.g. `"C – Eb – G – Eb"`) |
| `total_beats` | `u64` | Total number of beats |
| `total_duration` | `f64` | Duration in seconds |
| `data_byte_len` | `u64` | Byte length used as seed |

## Streaming synthesis (`SampleReader`)

For large files, generate samples on the fly from any `std::io::Read` source without loading everything into memory first:

```rust
use std::fs::File;
use chiptunomatic::{random::StdRandom, Chiptunomatic, Sample};

let mut chip = Chiptunomatic::default()
    .with_default_plugins()
    .with_random(Box::new(StdRandom::new()));

chip.load_song_metadata_from_path("big_file.bin")?;

let file = File::open("big_file.bin")?;
let mut reader = chip.sample_reader(file);
let mut buf = [Sample::default(); 1024];

loop {
    let n = reader.next_buf(&mut buf)?;
    if n == 0 {
        break; // end of stream
    }
    for sample in &buf[..n] {
        // process sample.value (mixed mono f32)
    }
}
```

`SampleReader` also implements `Iterator<Item = std::io::Result<Sample>>`.

## Mixer and stem control

Each track has four stems — **voice**, **square** (melody), **triangle** (bass), **noise** (drums) — plus a master output. All are controlled through `MixerConfig`:

```rust
use chiptunomatic::{MasterOutput, MixerConfig, StemOutput};

chip.mixer_mut().set_config(MixerConfig {
    master_output: MasterOutput {
        volume: 0.8,
        muted: false,
    },
    voice_output:    StemOutput { volume: 1.0, muted: false, solo: false },
    square_output:   StemOutput { volume: 1.0, muted: false, solo: true },  // melody only
    triangle_output: StemOutput { volume: 1.0, muted: true,  solo: false }, // bass muted
    noise_output:    StemOutput { volume: 1.0, muted: false, solo: false },
});
```

Multiple solos are additive: every stem whose `solo` flag is `true` is heard; the rest are silenced.

`Sample::value` is the normalised, mixed mono f32. `Sample::stems` exposes the raw per-stem floats before mixing, useful for visualisation or custom mixing.

## Modes

Each mode produces a different musical style. The default instance has only the `chiptune` mode. Call `.with_default_plugins()` to register all built-in modes:

```rust
// List available modes.
let modes = chip.modes(); // &["chiptune", "rock", "metal", ...]

// Switch mode (resets internal generator state automatically).
chip.set_mode(&"rock".to_string())?;
```

| Mode | Character |
|---|---|
| `chiptune` | Classic 8-bit square / triangle / noise |
| `rock` | Power chords, overdriven bass, loud kit |
| `metal` | Hard-clipped power chords, double-kick patterns |
| `rap` | Boom-bap FM piano, punchy sine bass |
| `trap` | FM bell melody, 808 pitch-sweep bass, stutter hi-hats |
| `toy` | FM kalimba tines with octave shimmer |
| `samba` | Reedy FM melody, surdo on beats 2 & 4, teleco-teco tamborim |
| `koto` | Karplus-Strong plucked strings, Hirajoshi pentatonic |

## Custom mode (Plugin)

Implement `Plugin` to register a completely custom mode:

```rust
use chiptunomatic::plugin::Plugin;

#[derive(Debug, Clone)]
struct MyPlugin;

impl Plugin for MyPlugin {
    fn mode(&self) -> &'static str { "my_mode" }
    // … override tempo_from_seed, chord_progression_from_seed, drum_pattern_from_seed,
    //   melody_sample, bass_sample, voice_sample as needed.
}

chip.register_plugin(Box::new(MyPlugin));
chip.set_mode(&"my_mode".to_string())?;
```

## Custom random

Provide any `Random` implementation to control how stochastic elements (drum fills, slight pitch variation, etc.) are generated. Use `NoRandom` (the default) for fully deterministic, RNG-free output:

```rust
use chiptunomatic::random::{NoRandom, StdRandom};

// Fully deterministic — no random calls.
let chip = Chiptunomatic::default();

// Seeded standard RNG — deterministic but with musical variation.
let chip = Chiptunomatic::default()
    .with_random(Box::new(StdRandom::new()));
```

## WAV export

With the `wav` feature (included in `default`), convert any file directly to a WAV on native targets:

```rust
chip.file_to_wav("input.bin", "output.wav")?;
```

This calls `load_song_metadata_from_path` and streams synthesis internally; no large intermediate buffer is allocated.

## Feature flags

| Flag | Default | Description |
|---|---|---|
| `std` | yes | Enables `std`-dependent APIs (`SampleReader`, path helpers, `StdRandom`) |
| `wav` | yes | Enables `file_to_wav` via the `hound` crate (requires `std`) |
| `arpeggio` | yes | Enables the `Arpeggiator` for chord note expansion |
| `js` | no | Wires `getrandom`'s JS backend for `wasm32-unknown-unknown` in a JS host |

## Constants

```rust
use chiptunomatic::constants::SAMPLE_RATE; // 44_100 u32
```

All synthesis runs at 44 100 Hz mono. `Sample::value` is a normalised `f32` in roughly `[-1, 1]` — scale to `i16` with `(value * 32767.0) as i16` for 16-bit PCM.
