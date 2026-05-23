# chiptunomatic


<p align="center"><img src="https://raw.githubusercontent.com/Nauja/chiptunomatic/media/preview-v0.2.0.gif"/></p>

Turn any file into a chiptune. Drop in a binary, an executable, a document — anything. The file name and its byte length are hashed into a deterministic seed, which drives every musical decision: root note, tempo, chord progression, drum pattern, and waveforms. The same file always produces the same tune.

The output is a mix of four synthesized stems at 44 100 Hz:

- **Voice** — mode-specific melodic lead layered over the melody (vibrato sine in Chiptune, FM ney flute in Persian, saturated vocal in Metal, …)
- **Square** — melody over a pentatonic minor chord progression
- **Triangle** — bass line
- **Noise** — kick / snare / hi-hat drum pattern

The project is a Rust workspace with three crates and a Next.js web UI:

| Crate | Description |
|---|---|
| `chiptunomatic` | Core library — `no_std` + WASM-compatible, no I/O |
| `chiptunomatic-cli` | Native CLI with interactive TUI and WAV export |
| `chiptunomatic-wasm` | wasm-bindgen bindings for the browser |

## Why

Chiptunomatic is a long-standing personal project: generate procedural chiptune music directly from arbitrary files, with no manual composition involved. The original question was simple — given that every file is just a stream of bytes, how would an AI like Claude decide to map those bytes to music?

The first results were exactly what you'd expect: a wall of essentially random notes with no structure, no rhythm, and no harmonic logic. From there the project was refined entirely through prompting — describing what was missing, explaining what "better" should sound like, and letting Claude propose and implement the changes. Chord progressions were added so notes relate to each other. Structured drum patterns replaced the noise. A classic song form (intro → verse → chorus → bridge → outro) gave each file a shape that feels like a real track rather than an accident. Synthesis was tuned mode by mode until each one had a recognisable character.

The result is less "AI writes music" and more "AI and human iterate together on a rule system until the output is worth listening to" — which turned out to be the more interesting experiment.

**Disclaimer.** The author knows nothing about music. All musical terminology, synthesis descriptions, mode breakdowns, and in-depth technical explanations in this README were written by Claude. If something sounds wrong to a musician, it probably is — but it was Claude's call, not mine.

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
| Play in rock mode | `chiptunomatic path/to/file -m rock` |
| Play in metal mode | `chiptunomatic path/to/file -m metal` |
| Play in trap mode | `chiptunomatic path/to/file -m trap` |
| Play in rap mode | `chiptunomatic path/to/file -m rap` |
| Play in koto mode | `chiptunomatic path/to/file -m koto` |
| Play in toy mode | `chiptunomatic path/to/file -m toy` |
| Play in samba mode | `chiptunomatic path/to/file -m samba` |

**Options**

```
-o, --output <FILE>   Write generated audio to a WAV file and exit
    --info            Print metadata for the input file and exit
-m, --mode <MODE>     Music mode: chiptune (default), rock, metal, rap, trap, toy, samba, koto
    --volume <FLOAT>  Master playback volume (default: 0.25)
    --muted           Mute master output
    --config <FILE>   Path to a custom config file
    --autoplay        Start with autoplay enabled
```

Per-stem settings (volume, mute, solo) are configured through the [config file](#config-file).

### Config file

On first run the CLI creates a commented-out default config at:

| Platform | Path |
|---|---|
| Linux / macOS | `~/.config/chiptunomatic/config.yml` |
| Windows | `%LOCALAPPDATA%\chiptunomatic\config.yml` |

All fields are optional. Uncomment and edit the ones you want to change:

```yaml
# Enabled music modes (at least one required).
# Comment out the whole block to enable all modes.
#modes:
#  - chiptune
#  - rock
#  - metal
#  - rap
#  - trap
#  - toy
#  - samba
#  - koto

# Default music mode (must be one of the enabled modes above)
#mode: chiptune

# Start with autoplay enabled (automatically advance to the next file when playback ends)
#autoplay: false

# Master volume (0.0 = silent, 1.0 = full)
#volume: 0.25

# Mute master output
#muted: false

# Per-stem settings
#voice:
#  volume: 1.0
#  muted: false
#  solo: false

#square:
#  volume: 1.0
#  muted: false
#  solo: false

#triangle:
#  volume: 1.0
#  muted: false
#  solo: false

#noise:
#  volume: 1.0
#  muted: false
#  solo: false

#sfx:
#  volume: 1.0
#  muted: false
#  solo: false
```

The `modes` list restricts which modes are available in the TUI's mode switcher (`m` / `Shift+M`). If omitted, all modes are enabled. At least one valid mode name must be listed — the CLI exits with an error otherwise.

CLI flags (`--volume`, `--muted`, `--mode`) override the config file. Multiple solos are additive — if both `square` and `triangle` are soloed, only those two stems are heard.

### TUI key bindings

| Key | Action |
|---|---|
| `q` / `Esc` | Quit (if a stem button is focused, unfocuses it first) |
| `Enter` | Play selected file; if already playing, pause / resume |
| `h` `j` `k` `l` / arrow keys | Navigate the file browser |
| `Ctrl+h` | Toggle hidden files |
| `m` | Advance to next music mode (Chiptune → Rock → Metal → …); restarts the current track immediately in the new mode |
| `Shift+M` | Go back to previous music mode; restarts the current track immediately in the new mode |
| `Tab` / `Shift+Tab` | Cycle focus forward / backward through the stem M/S buttons |
| `Space` | Toggle the focused M (mute) or S (solo) button |

The TUI shows real-time waveform charts for all four stems in a 2×2 grid (voice top-left, square top-right, triangle bottom-left, noise bottom-right), a peak bar per stem, a playback progress bar, and a metadata panel (root note, BPM, chord progression, beats, duration).

### Mode comparison

| | Chiptune | Lofi | Rock | Metal | Persian | Trap | Rap | Medieval | Koto | Toy | Samba |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **BPM** | 150–179 | 74–82 | 115–145 | 140–175 | 72–90 | 125–154 | 84–99 | 80–100 | 80–99 | 95–114 | 100–115 |
| **Scale** | Pentatonic minor | Pentatonic minor | Pentatonic minor | Pentatonic minor | Persian (b2, M3, P4, P5) | Pentatonic minor | Pentatonic minor | Dorian pentatonic | Hirajoshi pentatonic | Major pentatonic | Major pentatonic |
| **Melody** | Square wave | Two-voice FM Rhodes (tine attack + warm body) + brass pad | Power chord square + soft clip | Power chord square + hard clip | FM ney flute (1:1, β=0.35) + octave whistle | FM bell (4:1, β=2.5) | FM piano (1.5:1, β=1.0) | Additive recorder (3 partials) + KS lute | Karplus-Strong string | FM tine (3:1, β=1.2) + octave shimmer | FM reedy (2:1, β=0.6) |
| **Bass** | Triangle wave | 3-string KS guitar (root + P5 + octave) + warm sine | Overdriven triangle | Saturated square, very heavy | Karplus-Strong setar/oud | 808 pitch-sweep sine | Clean punchy sine | Open-fifth organum drone (root + P5) | Karplus-Strong string | Gentle sine | Punchy plucked sine |
| **Kick** | Square wave | Pitch-sweep sine, soft | Square + sine sub, heavy | Square + sine sub, loud | Sine thump 130–145 Hz (tombak dom) | Pitch-sweep sine, very loud | Square + sine sub, tight | Sine thump + noise transient (tabor) | Sine thump (taiko) | Sine thump, very soft | Pitch-sweep sine, beats 2+4 (surdo) |
| **Snare** | Noise burst | Noise + 180 Hz body, soft | Loud noise + 200 Hz body | Explosive noise + 220 Hz body | Noise + 380 Hz body, short (tombak tak) | Layered double-hit clap | Noise + 220 Hz body ("bap") | Noise + 100 Hz body (frame drum) + tambourine jingles | Noise + 200 Hz body, very soft | Noise + 150 Hz body, barely audible | Dense noise + 250 Hz body (caixa) |
| **Hi-hat** | Moderate noise | Very quiet, short | Crisp, loud | Crisp, aggressive (8ths or 16ths) | Very sparse, restrained (finger cymbals) | Tight, metallic | Moderate, unhurried | 4-partial inharmonic bell (cymbala) + metallic transient | Barely audible shimmer | Nearly silent tick | Teleco-teco syncopation, noise + tok tone (tamborim) |
| **Chords** | Pentatonic minor | Jazz-flavoured minor | I–V–IV rock patterns | Dark minor loops (i–m3–P5, i–m7) | Modal drone (i–M3–i–P4, i–b2–i–M3) | Dark minor loops | Soul/funk minor loops | Root–P4–P5 modal | Hirajoshi m3/P5/m6 loops | Major pentatonic bright loops | Major pentatonic circular loops |

### Chiptune mode

Chiptune mode emulates the sound of a late-1980s game console — NES-style pulse waves, a triangle bass, noise percussion, and short video-game SFX interjections. It is the default mode.

**Tempo and chords.** BPM ranges from 150 to 179 (base 165, ±14 from the seed). Chord progressions are drawn from a shared pentatonic-minor table.

**Melody — NES pulse waves.** The melody combines two square-wave voices:
- *Lead* — amplitude 0.28, duty cycle tied to the scale degree (0.5 / 0.25 / 0.5 / 0.125 cycling through the four NES duty positions), pitched one octave below the raw MIDI value for a warmer register.
- *Harmony* — amplitude 0.18, duty cycle offset from the lead (0.25 / 0.5 / 0.125 / 0.5) so the two voices never share the same waveform shape at the same time.

**Bass.** Triangle wave at the raw MIDI pitch, ADSR-shaped for a smooth attack and decay.

**Drums.** All three voices use a lightweight square/noise synthesis:
- *Kick* — short square wave (60–72 Hz depending on the seed's colour byte), hard-decay envelope, ~120 ms.
- *Snare* — noise burst; backbeats (steps 4 and 12) hit at 0.22 amplitude, off-beats at 0.09.
- *Hi-hat* — closed: 18 ms noise at alternating amplitude (0.10 / 0.05). Open: 90 ms noise with a gentle sustain.

**SFX stem.** Roughly once every 24 melody notes the SFX stem fires a short video-game sound effect. Five types cycle deterministically from the seed:

| # | Name | Synthesis | Duration |
|---|---|---|---|
| 0 | Coin | Two percussive pings (C6 → G6), zero sustain | ~95 ms |
| 1 | Jump | Linear upward pitch glide 180 → 650 Hz, narrow duty | ~180 ms |
| 2 | Power-up | 10-note arpeggio sweep C3 → C6 (18 ms per step) | ~180 ms |
| 3 | Laser | Exponential downward sweep 1800 → 80 Hz, 10 % duty | ~220 ms |
| 4 | 1-UP | E5–G5–E6–C6–D6–G6 jingle, short-short-long rhythm | ~480 ms |

**Song structure.** The section layout is generated from the seed rather than being fixed, producing ~1 500 distinct song shapes while keeping the same high-level arc. The structure is always: intro → verse → pre-chorus → chorus → verse → pre-chorus → chorus → [bridge →] chorus → outro.

Each structural parameter is derived from a dedicated bit range of the root seed (bits 0–3 are the root note; bits 8–12 are the BPM offset):

| Seed bits | Parameter | Choices |
|---|---|---|
| 16–17 | Verse length | 24 beats (25 %), 28 beats (25 %), 32 beats (50 %) |
| 18 | Intro / outro length | 16 beats (50 %) or 8 beats (50 %) |
| 19 | Pre-chorus length | 16 beats (50 %) or 12 beats (50 %) |
| 20 | Chorus length | 32 beats (50 %) or 28 beats (50 %) |
| 21–22 | Silence after each chorus | 0.3 / 0.5 / 0.7 / 1.0 s (25 % each) |
| 23–24 | Voice active in verse | Yes (~25 %, both bits set) or No (~75 %) |
| 25 | Bridge present | Yes (50 %) or No (50 %) |
| 26 | Bridge length | 16 beats (50 %) or 12 beats (50 %) |

### Lofi mode

Lofi mode replaces every layer of the synthesis with something warmer and more laid-back.

**Tempo and chords.** BPM is pulled down to 74–82, and the chord progressions are drawn from a jazz-flavoured table (e.g. i – m7 – P5 – m7) rather than the straight pentatonic loops used in chiptune mode.

**Melody — Rhodes electric piano + warm brass.** The melody uses a two-voice FM model that reproduces the characteristic bright-then-warm sound of a struck Rhodes tine:
- *Tine attack* — a high modulation index (β=2.8, 2:1 ratio) creates a bright initial "ding" at 25 % amplitude. An ADSR envelope collapses it to silence in ~55 ms — it only exists for the transient.
- *Tine body* — a low modulation index (β=0.45, 2:1 ratio) gives the warm, slightly bell-like sustain at 20 % amplitude. A 3 ms attack, 100 ms decay to a 55 % sustain, and 120 ms release let it carry the note.
- *Harmony* — the same low-β FM formula applied to the harmony pitch at reduced amplitude (9 %), giving each chord note a soft accompaniment voice.
- *Warm brass pad* — FM synthesis at a 1:1 modulation ratio with β=1.8 on the harmony pitch. The 1:1 ratio concentrates energy in low-order harmonics and the moderate β produces the closed, buzzy spectrum of a muted brass section at 11 % amplitude. A slow 40 ms attack lets it swell in under the Rhodes attack transient rather than competing with it.

**Bass — three-string guitar.** Instead of a sine wave, the bass uses three Karplus-Strong voices voiced as an open guitar chord — root (20 %), perfect fifth at 1.4983× (11 %), and octave at 2× (5 %) — each seeded independently so they sound like distinct strings rather than copies. An 8 ms linear attack ramp softens the hard pluck onset into a mellow pluck while leaving the natural KS pitch-dependent decay intact. A warm sine at the root (10 %) reinforces the fundamental and rounds off high-frequency noise from the KS strings.

**Drums.** All three drum voices are softened:
- *Kick* — a pitch-sweeping sine from 100 Hz down to 45 Hz over ~150 ms, shaped like a boom-bap thud.
- *Snare* — a short noise burst blended with a 180 Hz sine body tone. Backbeats (steps 4 and 12) hit at full amplitude; other snare hits are halved.
- *Hi-hat* — very quiet and short; open hats are slightly longer noise at low amplitude.
- *Tape hiss* — a continuous low-level noise floor (1.4 % amplitude) is added to every step window, including silent ones, giving the whole track the characteristic analogue warmth of a tape-recorded beat.
- *Vinyl crackle* — approximately 8 % of steps receive a short sharp pop (2–5 ms, 7–16 % amplitude) at the start of the window. Amplitude and duration vary so no two crackles sound identical.

### Persian mode

Persian mode is the most harmonically distinctive mode in the collection. It replaces the pentatonic minor scale with a Persian pentatonic that contains an augmented second — the interval that gives Middle Eastern music its instantly recognisable sound.

**Tempo and chords.** BPM sits between 72 and 90, the slowest and most meditative of all modes. Chord progressions are drone-centred and circular, built to emphasise the two most characteristic scale degrees: the flat second (b2) and the major third (M3). Typical patterns: i – M3 – i – P4, or i – b2 – i – M3 — neither resolves in the Western sense; they simply circle back to the root.

**Scale.** Persian mode uses a custom 5-note scale: `[0, 1, 4, 5, 7]` — root, b2 (1 semitone), M3 (4 semitones), P4 (5 semitones), P5 (7 semitones). The crucial interval is the **augmented second** between b2 and M3 — a 3-semitone jump, compared to the standard 2-semitone whole step — which gives every melodic phrase that distinctive Eastern quality. Every note in the melody and every chord root is drawn from this scale.

**Melody — ney flute.** FM synthesis at a 1:1 modulator ratio with β=0.35. The 1:1 ratio means the modulator oscillates at the same frequency as the carrier; this adds mild sidebands that thicken the fundamental without adding bright high harmonics, producing the hollow, slightly reedy tone of the Persian ney reed flute. A faint second voice at twice the frequency (amplitude 0.04) mimics the weak octave overtone present in real ney playing. The envelope has a 40 ms attack — the time the air column takes to establish — followed by a high 78 % sustain and a slow 120 ms release for legato note flow.

**Bass — setar / oud.** Karplus-Strong plucked-string synthesis at the chord root. A 6 ms linear ramp softens the initial pluck transient into the mellow, warm attack of a gut or nylon string. Natural pitch-dependent decay from the algorithm means low notes sustain much longer than high notes, matching the acoustic behaviour of a real setar or oud.

**Voice.** A slow (4.5 Hz), deep (±2.5 %) vibrato sine with a long 60 ms onset. The slow rate and wide depth produce the winding, melismatic character of Persian classical singing — long sustained tones with expressive pitch inflection.

**Drums — tombak goblet drum.** The tombak is the primary Persian percussion instrument. Two stroke types dominate:
- *Dom (kick)* — the deep bass stroke, synthesised as a pitched sine at 130–145 Hz (seeded per file) plus a sub-octave sine layer. Both decay in ~65–70 ms to silence, creating the resonant body thump of a goblet drum head. Patterns are drawn from three dedicated tables that reflect common tombak rhythms: dom on beats 1, &2, 4; a 3-beat feel inside 4/4; or a sparse ornate figure.
- *Tak (snare)* — the high-pitched finger-snap stroke on the drum edge. A short (≤40 ms) noise burst is blended with a 380 Hz body tone for the crisp, bright crack of a hard edge hit. Patterns fill the spaces left by the dom strokes, creating the interlocking quality of real tombak playing.
- *Finger cymbals (hat)* — very sparse metallic accents at low amplitude (10–12 %). Closed hits are under 18 ms; an open ring (120 ms with 20 % sustain) appears at most once per pattern on beat 3, giving the percussion section a moment to breathe.

### Rap mode

Rap mode targets the boom-bap sound of classic hip-hop — groove-driven, head-nodding, built around a strong kick-and-snare backbone rather than the heavy sub-bass and frantic hi-hats of trap.

**Tempo and chords.** BPM sits between 84 and 99 — slower than any other mode and aimed at that deliberate, swaggering feel. Chord progressions are soul/funk-influenced minor loops with simple, hypnotic movement (e.g. i – IV – i – V, i – m3 – IV – m3) that repeat comfortably under a verse.

**Melody.** The lead uses FM synthesis with a moderate modulation ratio (1.5:1) and a low modulation index (1.0). This produces a warm, piano-like tone with mid-range presence — closer to a Fender Rhodes or a soul keyboard than the lofi electric piano (which uses a 2:1 ratio) or the metallic trap bell (4:1). The envelope has a 4 ms attack, a 180 ms decay to a 45 % sustain, and an 80 ms release, giving each note a clear pluck with a mellow body.

**Bass.** A clean sine wave follows the chord root at moderate amplitude (0.45), with a fast 7 ms decay to a 60 % sustain. No pitch sweep, no overdrive — the bass is melodic and punchy, staying present in the mix without dominating it the way an 808 does.

**Drums.** The boom-bap pattern is the defining element:
- *Kick ("boom")* — a square wave at ~72–87 Hz layered with a 52 Hz sine sub. Both have a short 80–100 ms decay to silence. Patterns are syncopated from a dedicated table: the "boom" always lands on beat 1, but the second hit falls on an off-beat rather than squarely on beat 3 — either at beat 1 + 3 + &3, beat 1 + &2 + 3, or beat 1 + &3 + 4. That loose, swaggering second kick is the rhythmic signature of boom-bap.
- *Snare ("bap")* — noise burst blended with a 220 Hz pitched body tone. Backbeats (steps 4 and 12) hit at full amplitude. Some pattern variants include ghost notes at off-beat positions (e.g. &2 or &3); those positions fall outside the accent check so they play at half amplitude automatically, giving the characteristic ghost-hit texture of boom-bap drumming.
- *Hi-hat* — sparse and unhurried, drawn from a dedicated table: quarter notes only, sparse 8ths, or straight 8ths. No rapid subdivisions. Open hat on beat 3 (seed-dependent) adds a brief breath between the backbeats.

### Trap mode

Trap mode is built around the three defining elements of the genre: the 808 bass, the stuttered hi-hat, and the punchy clap.

**Tempo and chords.** BPM sits between 125 and 154. Chord progressions are drawn from a dark, minor-heavy table that stays close to the root — loops like i – m3 – i – m7 and i – m7 – m3 – P5 — giving the harmonic motion that ominous, circular quality typical of trap.

**Melody.** The lead uses FM synthesis with a high modulation ratio (4:1) and a moderate modulation index (2.5). This pushes energy into the upper harmonics and produces the bright, metallic, bell-like pluck sound common in trap beats. The envelope decays very fast — 2 ms attack, 150 ms decay to a low 10 % sustain — so each note has a sharp transient and almost no sustained body. A quieter harmony voice is layered underneath using the same formula.

**Bass.** The iconic 808 sound is approximated with a pitch-sweeping sine wave. The note starts at 2.2× its target frequency and sweeps down to the actual pitch with an exponential decay (rate 12), then sustains there at 70 % amplitude for the rest of the note duration. This produces the "wub" or "slide" character — the sound starts bright and settles into the sub-bass register. Amplitude is high (0.60) to make it physically felt in the mix.

**Drums.** The drum voices are the heart of the trap sound:
- *Kick* — a pitch sweep from 180 Hz down to 42 Hz spanning the entire 16th-note step window (the full envelope is used — attack, decay into silence, and a tail release). Very loud (80 % amplitude) for maximum impact. Patterns are sparse — beat 1 alone, beats 1+3, or a syncopated beat-1 + &2 — so the kick stays out of the way of the heavy 808 bass.
- *Clap* — replaces the standard snare and is locked to beats 2 and 4 (steps 4 and 12) in every pattern variant. On those accented positions two noise bursts are layered: the second arrives 8 ms after the first, widening the clap and giving it that characteristic slapped, layered sound. Non-accented hits are a single quieter burst.
- *Hi-hat* — crisp and short, drawn from dedicated rapid-stutter pattern tables rather than the generic 8th/16th-note defaults. Three variants: a 3+2+3 stutter grouping, a triplet-cluster pattern, and a rolling 16th-note figure. An open hat is placed on the "and" of beat 2 (step 6) or the "and" of beat 4 (step 14) depending on the seed, adding a longer ring at a rhythmically prominent position.

### Metal mode

Metal mode pushes everything as hard and fast as it goes — distorted power chords, double-kick patterns, and an explosive snare.

**Tempo and chords.** BPM sits between 140 and 175 — the fastest of all modes. Chord progressions are locked to dark, circular minor loops (i – m3 – P5 – m3, i – m7 – m3 – i, etc.) that never resolve, holding that relentlessly ominous quality characteristic of the genre.

**Melody.** Two square waves form a power chord — root and perfect fifth (7 semitones). Both go through a very fast 1 ms attack and a high 92 % sustain envelope, then the combined signal is **hard-clipped** at ±0.65 and scaled down. Hard clipping (as opposed to the soft `x / (1 + |x|)` saturation used in rock) squares off the peaks completely, adding the dense harmonic cloud that distinguishes a truly distorted guitar tone. A quieter harmony voice is layered underneath with the same treatment.

**Bass.** A square wave driven through a soft saturator `d / (1 + |d|)` at gain × 2.2, resulting in a thick, compressed bass tone with significant upper harmonics. The envelope has a very fast 1 ms attack, 18 ms decay to a 75 % sustain — always present, never sustained loosely.

**Voice.** The default vibrato sine is replaced with a harsher vocal: faster vibrato (7 Hz), wider pitch depth (±4 %), then the result is pushed through the same soft saturator at gain × 2.2. This produces the clipped, harmonic-rich character of metal harsh vocals without generating noise.

**Drums.** The kit is the punchiest of all modes, with tight double-kick patterns and a cracking snare:
- *Kick* — a square wave at 55–63 Hz (higher than rock, punchier) layered with a 42 Hz sine sub. Both are enveloped tightly (40–55 ms decay) for a sharp double-kick attack at 48–26 % amplitude respectively. Patterns are drawn from three dedicated tables: 8th-note double-kick (every two 16th steps), gallop (1+&1+2), or syncopated 16ths — the specific pattern is seeded so each file gets a consistent feel.
- *Snare* — a noise burst at up to 36 % amplitude on the backbeats (steps 4 and 12), with a 220 Hz sine crack layered on accented positions. Non-backbeat hits drop to 16 % amplitude.
- *Hi-hat* — crisp noise bursts at 14 % amplitude, very short (under 14 ms on closed steps). Three hat patterns — 8th notes, relentless 16ths, or syncopated 16ths — driven by the seed. An open hat on beat 3 or &4 (seed-dependent) gives a brief ring between sections.

### Rock mode

Rock mode pushes every layer harder and faster.

**Tempo and chords.** BPM ranges from 115 to 145. Chord progressions use driving I–V–IV patterns (e.g. I – V – IV – V, I – IV – V – IV) to keep the harmonic motion energetic.

**Melody.** Two square waves are stacked to form a power chord — the root note and the perfect fifth 7 semitones above it. Both are passed through soft saturation (`x / (1 + |x|)`) after mixing, which rounds off the harsh square-wave peaks and adds a subtle distortion character. The envelope has a very fast 2 ms attack, minimal decay, and a high 80 % sustain so the notes hold aggressively.

**Bass.** A triangle wave is driven hard (amplitude × 1.8) then soft-clipped, adding upper harmonics that give the bass line a gritty, overdriven quality. A snappy 2 ms attack and 75 % sustain keep it punchy and forward in the mix.

**Drums.** All three drum voices are significantly louder and more present:
- *Kick* — a square wave at ~65 Hz layered with a 50 Hz sine sub for physical low-end weight. Both are enveloped tightly for a sharp, heavy thud.
- *Snare* — a loud noise burst (35 % amplitude on backbeats 2 and 4, 15 % elsewhere), with a 200 Hz sine tone blended in on the accented backbeats for a cracking snap.
- *Hi-hat* — crisp and tight, roughly twice the amplitude of the chiptune hi-hat; open hats ring for up to 120 ms with a slow decay.

### Medieval mode

Medieval mode is the most harmonically distinctive — it replaces the pentatonic minor scale with a Dorian pentatonic (root, M2, P4, P5, m7) built entirely on perfect consonances, and replaces every synthesised voice with period-appropriate timbres.

**Tempo and chords.** BPM sits between 80 and 100 — unhurried and processional. Chord progressions are restricted to the root, perfect fourth, and perfect fifth (e.g. i – P5 – P4 – P5, i – P4 – i – P5). These are the intervals that defined medieval polyphony: no thirds, no sevenths, just open perfect consonances that give the music its characteristic spaciousness and modal ambiguity.

**Scale.** Unlike every other mode, medieval uses a different melodic scale: `[0, 2, 5, 7, 10]` — a Dorian pentatonic. The walking melody algorithm operates on this scale, so all melodic steps and harmony notes are drawn from it. The absence of the minor third makes the mode feel neither clearly major nor minor — exactly the tonal quality of Gregorian and early secular music.

**Melody.** Two voices layer to form a two-part texture:
- *Recorder (lead)* — additive synthesis with three near-harmonic partials on the melody pitch: a strong fundamental (amplitude 0.32), a faint second harmonic at ~8 % (0.026), and a fainter third harmonic at ~4 % (0.013). Stacking these partials gives the slightly hollow, breathy character of a wooden recorder without any buzz or edge. The envelope has a slow 22 ms attack — real recorders need time before the tone speaks cleanly — a short 30 ms decay to a high 88 % sustain, and a 100 ms release so notes flow legato.
- *Lute (accompaniment)* — Karplus-Strong plucked-string synthesis on the harmony pitch at amplitude 0.20. The physical model produces natural pitch-dependent decay: high notes ring briefly, low notes sustain longer, matching the behaviour of a real lute string.

**Bass.** An open-fifth organum drone: root sine (amplitude 0.28) plus a perfect fifth 7 semitones up (amplitude 0.14), both shaped by a 20 ms attack, a 50 ms decay to a 70 % sustain, and a 200 ms release. Sustained parallel fifths were the defining bass texture of early medieval polyphony (organum).

**Drums.** Patterns are sparse and processional — at most 2–4 hits per bar, drawn from dedicated medieval pattern tables rather than the denser patterns used by other modes. All three voices are restrained, evoking period percussion rather than a modern kit:
- *Tabor (kick)* — a pitched sine at 150–170 Hz, shaped to a 120 ms decay to silence, with a short 12 ms beater-on-skin noise transient layered at the attack. The noise burst gives the dry initial impact of a mallet striking a small drum; the sine carries the body resonance.
- *Frame drum + tambourine jingles (snare)* — a noise burst blended with a 100 Hz membrane tone produces the soft thud of a hand-held frame drum. Overlaid on every hit are three detuned tambourine jingles — sines at 3 100–3 700 Hz at ratios 1 : 1.09 : 1.21 plus a noise component — giving the shimmering metallic jangle of zill pairs. Accented positions hit at full amplitude; others are halved.
- *Cymbala (hat)* — four inharmonic bell-mode partials at ratios 1 : 2.76 : 5.4 : 8.93 (hum, tierce, quint, and nominal modes of a cast-bronze resonator), with a fundamental at 1 000–1 580 Hz, combined with a brief 12 ms broadband noise burst for the metallic clang of the strike. The four inharmonic partials remove the "electronic bip" character that two pure sines produce and give the cymbal its cast-metal shimmer. Closed hits decay in ~110 ms; open-hat steps ring for up to 300 ms.

### Koto mode

Koto mode is the only mode that uses a genuine physical model rather than waveform synthesis for the melody: the Karplus-Strong algorithm, which physically models a plucked string.

**Tempo and chords.** BPM sits between 80 and 99. Chord progressions are drawn from the Hirajoshi pentatonic scale (degrees: root, m3, P5, m6) — e.g. root – m3 – P5 – m3 and root – P5 – m6 – P5 — keeping the characteristic semitone intervals of the scale in the foreground.

**Scale.** Like medieval mode, koto uses a different melodic scale: Hirajoshi pentatonic `[0, 2, 3, 7, 8]` (root, M2, m3, P5, m6). The minor third (one semitone above the major second) and the minor sixth (one semitone above the perfect fifth) are the intervals that give Japanese koto music its instantly recognisable sound. The melody walking algorithm runs on this scale, so every step and harmony note is drawn from it.

**Melody — Karplus-Strong synthesis.** A delay line of length `sample_rate / frequency` is initialised with deterministic pseudo-random noise seeded from the note pitch, then on every sample the output is the average of the current and the previous delay-line value. This one-pole averaging filter damps high-frequency energy faster than low-frequency energy on each pass through the buffer, so the initial noisy burst decays into a near-pure tone over time — exactly what happens when a string vibrates and loses energy to friction. Because higher notes have shorter delay lines, they complete more filter passes per unit time and decay faster; lower notes ring longer. No explicit envelope is needed: the decay is a natural consequence of the physics. A 10 ms linear fade-out is applied only at the very end of the note window to prevent a click at the boundary.

**Bass.** A second Karplus-Strong voice at the bass register. The longer delay line at lower pitches means the bass strings ring noticeably longer than the melody strings, naturally providing a sustained, resonant low-end under the plucked melody.

**Percussion.** Kept very minimal so as not to compete with the delicate plucked texture:
- *Taiko kick* — a pitched sine at ~78–96 Hz, moderate amplitude, rounded 80 ms decay.
- *Hand percussion* — noise + 200 Hz body tone at very low amplitude (6–12 %). Barely audible on non-accented steps.
- *Shimmer* — tiny noise bursts at 3–5 % amplitude, under 12 ms. Essentially inaudible; they exist only to avoid complete silence on hat steps.

### Toy mode

Toy mode is built around the sound of a kalimba — a small lamellaphone whose tines produce a bright, bell-like pluck with a natural octave shimmer. The overall character is intentionally small, sweet, and light.

**Tempo and chords.** BPM sits between 95 and 114 — moderate and playful. Chord progressions are drawn from the major pentatonic scale in simple, upbeat loops (e.g. root – M3 – P5 – M3, root – M6 – P5 – root) that feel unambiguously cheerful.

**Scale.** Toy mode uses the major pentatonic: `[0, 2, 4, 7, 9]` (root, M2, M3, P5, M6). Every melodic step and harmony note is drawn from this scale, keeping the mood bright and avoiding any minor-mode tension.

**Melody — FM tine with octave shimmer.** Unlike the koto (which uses Karplus-Strong), the kalimba melody uses FM synthesis to capture the character of a struck metal tine. A 3:1 modulator ratio and a modulation index of 1.2 introduce the slight inharmonicity of metal without going into bell territory — the result is bright and clear rather than twangy or plucked. The envelope has a very fast 1 ms attack (the immediate snap of the tine), a 220 ms decay to a low 8 % sustain, and a 100 ms release — so each note has a prominent "tink" front edge that then settles into a quiet ring.

The octave shimmer is a second FM voice at the same parameters pitched one octave up (amplitude 0.28). Its envelope decays in just 100 ms to a 2 % sustain, so it vanishes well before the fundamental — matching exactly what happens on a real kalimba tine, where the upper partial fades within the first fraction of a second and leaves the fundamental to sustain alone.

**Bass.** A gentle sine wave at moderate amplitude (0.28) with a slow-ish 120 ms decay to a 45 % sustain. Warm harmonic support without competing with the tines.

**Percussion.** Deliberately minimal and delicate — nothing that would overpower the subtle plucked texture:
- *Toy kick* — a soft pitched sine at ~100–120 Hz, low amplitude (22 %), short 80 ms decay. A gentle thump rather than a heavy hit.
- *Toy tap* — a very quiet noise burst blended with a 150 Hz body tone. Barely audible on non-accented steps; accented backbeats are still restrained.
- *Shimmer hat* — tiny noise bursts at 2–4 % amplitude, under 10 ms. Essentially inaudible on closed steps; open-hat steps get a slightly longer airy puff.

### Samba mode

Samba mode captures the energy and drive of Brazilian samba — fast, bright, and built around a strong rhythmic backbone of layered percussion.

**Tempo and chords.** BPM sits between 100 and 115 — urgent and dancing. Chord progressions are drawn from the major pentatonic scale in circular, hypnotic loops (e.g. root – P5 – M3 – P5, root – M3 – root – P5) that feel perpetually in motion without a strong resolve.

**Scale.** Like Toy mode, Samba uses the major pentatonic: `[0, 2, 4, 7, 9]` (root, M2, M3, P5, M6). The bright, tension-free intervals keep the mood celebratory and upbeat throughout.

**Melody.** FM synthesis with a 2:1 modulator ratio and β=0.6. This sits between the warmth of the lofi electric piano (β=0.7) and the pure recorder (β=0.25) — crisp and reedy without going metallic. The envelope has a 2 ms attack, 150 ms decay to a 25 % sustain, and a 60 ms release: each note articulates clearly and quickly, matching the fast note density of samba phrasing. A harmony voice is layered at half the amplitude using the same formula.

**Bass.** A punchy sine wave with a very fast 70 ms decay to a 15 % sustain — the amplitude falls away almost immediately after the attack, mimicking the short, popping character of a plucked bass guitar string. This rhythmically tight bass locks in with the surdo rather than hanging in the mix as a sustained tone.

**Drums.** Samba's identity is almost entirely rhythmic, so each voice has its own dedicated pattern table instead of the generic shared patterns used by other modes.

- *Surdo (kick)* — marks the contratempos on **beats 2 and 4** (steps 4 and 12 in the 16-step grid), not beats 1 and 3. This reversed beat placement is the single most defining characteristic of samba. Three pattern variants are available: the standard marcação (beats 2+4 only), a version that adds a hit on beat 1, and a cortador-flavoured variant that inserts an off-beat hit between beats 3 and 4. Synthesis uses a pitch sweep starting at 1.5× the target frequency (60–75 Hz) and settling to the fundamental over a 280 ms resonant decay, giving the boom-and-settle character of a large bass drum head.

- *Caixa (snare)* — the driving force of samba, played dense and relentlessly. Three patterns: all 16th notes (the most characteristic caixa texture), all 8th notes, and a repinique-style syncopation. Accents fall on 8th-note positions (even steps) regardless of which pattern is active; 16th-note offbeats are played at roughly half amplitude. Synthesis is a very fast noise burst (30 ms zero-to-silence decay) blended with a 250 Hz body tone for crack.

- *Tamborim (hi-hat)* — plays one of three classic samba syncopated figures rather than a simple subdivision: the dense teleco-teco (1 . 1 . 1 1 . 1 . 1 . 1 1 . 1 .), the sparse teleco-teco (1 . . 1 . 1 . . 1 . . 1 . 1 . .), or the cruzado 3+3+2 pattern (1 . . 1 . . 1 . 1 . . 1 . . 1 .). Synthesis adds a brief 1 400–1 700 Hz sine "tok" tone on top of the noise burst, giving the instrument the wood-frame-and-metal-head timbre of a real tamborim rather than plain noise.
