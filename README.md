# chiptunomatic

Turn any file into a chiptune. Drop in a binary, an executable, a document — anything. The file name and its byte length are hashed into a deterministic seed, which drives every musical decision: root note, tempo, chord progression, drum pattern, and waveforms. The same file always produces the same tune.

The output is a mix of three synthesized voices at 44 100 Hz:

- **Square** — melody over a pentatonic minor chord progression
- **Triangle** — bass line
- **Noise** — kick / snare / hi-hat drum pattern

The project is a Rust workspace with three crates and a Next.js web UI:

| Crate | Description |
|---|---|
| `chiptunomatic` | Core library — `no_std` + WASM-compatible, no I/O |
| `chiptunomatic-cli` | Native CLI with interactive TUI and WAV export |
| `chiptunomatic-wasm` | wasm-bindgen bindings for the browser |

## Demo

The web app runs entirely in your browser — no file is ever uploaded. Drop any file, get a chiptune, download the WAV.

**https://nauja.github.io/chiptunomatic**

## CLI

### Download

Pre-built binaries are attached to every [GitHub release](https://github.com/Nauja/chiptunomatic/releases/latest):

| Platform | File |
|---|---|
| Linux x86-64 | `chiptunomatic-x86_64-unknown-linux-gnu` |
| Windows x86-64 | `chiptunomatic-x86_64-pc-windows-msvc.exe` |

### Build from source

**Linux** — install ALSA headers first:

```sh
sudo apt-get install libasound2-dev pkg-config
cargo build --release -p chiptunomatic-cli
# binary at target/release/chiptunomatic
```

**Windows** (no extra dependencies):

```sh
cargo build --release -p chiptunomatic-cli
# binary at target\release\chiptunomatic.exe
```

### Usage

```
chiptunomatic [FILE] [OPTIONS]
```

| Mode | Command |
|---|---|
| Open TUI and start playing `FILE` | `chiptunomatic path/to/file` |
| Open TUI with file browser | `chiptunomatic` |
| Export to WAV and exit | `chiptunomatic path/to/file -o out.wav` |
| Print song metadata and exit | `chiptunomatic path/to/file --info` |

**Options**

```
-o, --output <FILE>    Write generated audio to a WAV file and exit
    --info             Print metadata for the input file and exit
    --volume <FLOAT>   Master playback volume (default: 0.25)
```

### TUI key bindings

| Key | Action |
|---|---|
| `q` / `Esc` | Quit |
| `Enter` | Play selected file; if already playing, pause / resume |
| `h` `j` `k` `l` / arrow keys | Navigate the file browser |
| `Ctrl+h` | Toggle hidden files |

The TUI shows real-time waveform charts for each voice (square, triangle, noise), a volume bar per channel, a playback progress bar, and a metadata panel (root note, BPM, chord progression, beats, duration).
