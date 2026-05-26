use std::path::PathBuf;

use chiptunomatic::{random::StdRandom, Chiptunomatic, MasterOutput, MixerConfig};
use clap::{CommandFactory, FromArgMatches, Parser};

use cli_log::*;

mod config;
mod stem_plot;
mod synth;
mod tui;

/// chiptunomatic — generate chiptune music from any file (Rust port).
#[derive(Parser, Debug)]
#[command(name = "chiptunomatic", version, about)]
struct Args {
    /// Path to a custom config file (default: ~/.config/chiptunomatic/config.yml).
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
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
    #[arg(long)]
    volume: Option<f32>,
    /// Mute the master playback.
    #[arg(long, default_value_t = false)]
    muted: bool,
    /// Music generation mode.
    #[arg(short = 'm', long = "mode")]
    mode: Option<String>,
    /// Start with autoplay enabled.
    #[arg(long, default_value_t = false)]
    autoplay: bool,
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

    // Pre-scan for --config before the full clap parse so the config is available
    // when building dynamic clap defaults (mode list, default mode).
    let config_path: Option<std::path::PathBuf> = {
        let argv: Vec<String> = std::env::args().collect();
        argv.iter().enumerate().find_map(|(i, arg)| {
            if arg == "--config" {
                argv.get(i + 1).map(std::path::PathBuf::from)
            } else {
                arg.strip_prefix("--config=").map(std::path::PathBuf::from)
            }
        })
    };

    let config = match &config_path {
        Some(path) => config::Config::load_from(path)?,
        None => {
            config::Config::create_default_if_missing();
            config::Config::load()
        }
    };

    let mut instance = match &config.modes {
        Some(modes) if modes.is_empty() => {
            anyhow::bail!("config error: 'modes' must list at least one mode");
        }
        Some(modes) => Chiptunomatic::default()
            .with_plugins_from_list(modes)
            .map_err(|e| {
                anyhow::anyhow!("config error: no valid mode found in 'modes' list ({e})")
            })?,
        None => Chiptunomatic::default().with_default_plugins(),
    }
    .with_random(Box::new(StdRandom::new()));

    // Apply config mode as default before building clap defaults
    if let Some(ref mode) = config.mode {
        let _ = instance.set_mode(mode);
    }

    // Leak the default mode string once so clap can store a 'static reference
    let default_mode: &'static str = Box::leak(instance.mode().to_string().into_boxed_str());
    let cmd = Args::command().mut_arg("mode", |a| {
        a.required(false).default_value(default_mode).value_parser(
            clap::builder::PossibleValuesParser::new(instance.modes().clone()),
        )
    });

    let matches = cmd.get_matches();
    let args = Args::from_arg_matches(&matches)
        .map_err(|e| e.exit())
        .unwrap();

    // CLI arg wins over config; config wins over built-in default
    let mode = args
        .mode
        .as_ref()
        .or(config.mode.as_ref())
        .cloned()
        .unwrap_or_else(|| instance.mode().to_string());
    instance.set_mode(&mode)?;

    // Configure the volumes — CLI > config > built-in default
    instance.mixer_mut().set_config(MixerConfig {
        master_output: MasterOutput {
            volume: args.volume.unwrap_or(config.volume.unwrap_or(0.25)),
            muted: args.muted || config.muted.unwrap_or(false),
        },
        voice_output: config.voice.into(),
        square_output: config.square.into(),
        triangle_output: config.triangle.into(),
        noise_output: config.noise.into(),
        sfx_output: config.sfx.into(),
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
            instance.file_to_wav(input, output)?;
            Ok(())
        }
        (Some(input), None, false) => {
            let autoplay = args.autoplay || config.autoplay.unwrap_or(false);
            tui::run(instance, Some(input), autoplay)
        }
        (None, None, false) => {
            let autoplay = args.autoplay || config.autoplay.unwrap_or(false);
            tui::run(instance, None, autoplay)
        }
    }
}
