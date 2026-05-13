use std::path::PathBuf;

use chiptunomatic::{constants::SAMPLE_RATE, Chiptunomatic, MixGenerator, StemOutput};
use clap::{CommandFactory, FromArgMatches, Parser};

use cli_log::*;

mod stem_plot;
mod synth;
mod tui;

/// chiptunomatic — generate chiptune music from any file (Rust port).
#[derive(Parser, Debug)]
#[command(name = "chiptunomatic", version, about)]
struct Args {
    /// Input file. Without --output, opens the TUI with this file selected and playing.
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,
    /// Write generated audio to this WAV file and exit (no TUI).
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    output: Option<String>,
    /// Print metadata for the input file and exit.
    #[arg(long = "info")]
    info: bool,
    /// Master playback volume (1.0 = full, 0.0 = silent).
    #[arg(long, default_value_t = 0.25)]
    volume: f32,
    /// Mute the master playback.
    #[arg(long, default_value_t = false)]
    muted: bool,
    /// Music generation mode.
    #[arg(short = 'm', long = "mode")]
    mode: String,
    /// Per-stem volume multiplier for the voice stem (default: 1.0).
    #[arg(long, default_value_t = 1.0)]
    voice_volume: f32,
    /// Mute the voice stem.
    #[arg(long, default_value_t = false)]
    voice_muted: bool,
    /// Solo the voice stem (silences all other stems).
    #[arg(long, default_value_t = false)]
    voice_solo: bool,
    /// Per-stem volume multiplier for the square (melody) stem (default: 1.0).
    #[arg(long, default_value_t = 1.0)]
    square_volume: f32,
    /// Mute the square (melody) stem.
    #[arg(long, default_value_t = false)]
    square_muted: bool,
    /// Solo the square (melody) stem (silences all other stems).
    #[arg(long, default_value_t = false)]
    square_solo: bool,
    /// Per-stem volume multiplier for the triangle (bass) stem (default: 1.0).
    #[arg(long, default_value_t = 1.0)]
    triangle_volume: f32,
    /// Mute the triangle (bass) stem.
    #[arg(long, default_value_t = false)]
    triangle_muted: bool,
    /// Solo the triangle (bass) stem (silences all other stems).
    #[arg(long, default_value_t = false)]
    triangle_solo: bool,
    /// Per-stem volume multiplier for the noise (drum) stem (default: 1.0).
    #[arg(long, default_value_t = 1.0)]
    noise_volume: f32,
    /// Mute the noise (drum) stem.
    #[arg(long, default_value_t = false)]
    noise_muted: bool,
    /// Solo the noise (drum) stem (silences all other stems).
    #[arg(long, default_value_t = false)]
    noise_solo: bool,
}

mod audio {
    use anyhow::Result;
    use cli_log::debug;
    use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};

    pub(crate) struct AudioPlayer {
        sample_rate: u32,
        _stream: OutputStream,
        _handle: OutputStreamHandle,
        pub sink: Sink,
    }

    impl AudioPlayer {
        pub(crate) fn new(sample_rate: u32) -> Result<Self> {
            let (_stream, _handle) = OutputStream::try_default()?;
            let audio_player = Self {
                sample_rate,
                sink: Sink::try_new(&_handle)?,
                _stream,
                _handle,
            };
            debug!("Audio player initialized");
            Ok(audio_player)
        }

        pub(crate) fn append(&mut self, samples: &[f32]) {
            self.sink
                .append(SamplesBuffer::new(1, self.sample_rate, samples));
        }

        pub(crate) fn sleep_until_end(&mut self) {
            self.sink.sleep_until_end();
        }
    }
}

fn main() -> anyhow::Result<()> {
    init_cli_log!();

    let mut instance = Chiptunomatic::new().with_default_plugins();

    // Update the default values
    let cmd = Args::command().mut_arg("mode", |a| {
        a.required(false)
            .default_value(instance.mode())
            .value_parser(clap::builder::PossibleValuesParser::new(
                instance.modes().clone(),
            ))
    });

    let matches = cmd.get_matches();
    let args = Args::from_arg_matches(&matches)
        .map_err(|e| e.exit())
        .unwrap();

    instance.set_mode(&args.mode)?;

    let mix_generator = MixGenerator::default()
        .with_master_output(StemOutput {
            volume: args.volume,
            muted: args.muted,
            solo: false,
        })
        .with_voice_output(StemOutput {
            volume: args.voice_volume,
            muted: args.voice_muted,
            solo: args.voice_solo,
        })
        .with_square_output(StemOutput {
            volume: args.square_volume,
            muted: args.square_muted,
            solo: args.square_solo,
        })
        .with_triangle_output(StemOutput {
            volume: args.triangle_volume,
            muted: args.triangle_muted,
            solo: args.triangle_solo,
        })
        .with_noise_output(StemOutput {
            volume: args.noise_volume,
            muted: args.noise_muted,
            solo: args.noise_solo,
        });

    match (args.input.as_deref(), args.output.as_deref(), args.info) {
        (None, Some(_), _) => anyhow::bail!("--output requires an input file"),
        (None, _, true) => anyhow::bail!("--info requires an input file"),
        (Some(input), _, true) => {
            let metadata = instance.load_song_metadata_from_path(input)?;
            print!("{metadata}");
            Ok(())
        }
        (Some(input), Some(output), false) => {
            instance.file_to_wav(input, output, SAMPLE_RATE, mix_generator)?;
            Ok(())
        }
        (Some(input), None, false) => tui::run(instance, Some(input), mix_generator),
        (None, None, false) => tui::run(instance, None, mix_generator),
    }
}
