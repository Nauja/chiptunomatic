use std::path::PathBuf;

use chiptunomatic::{constants::SAMPLE_RATE, file_to_wav, SongMetadata};
use clap::Parser;

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

    let args = Args::parse();

    match (args.input.as_deref(), args.output.as_deref(), args.info) {
        (None, Some(_), _) => anyhow::bail!("--output requires an input file"),
        (None, _, true) => anyhow::bail!("--info requires an input file"),
        (Some(input), _, true) => {
            let metadata = SongMetadata::from_path(input)?;
            print!("{metadata}");
            Ok(())
        }
        (Some(input), Some(output), false) => {
            file_to_wav(input, output, SAMPLE_RATE)?;
            Ok(())
        }
        (Some(input), None, false) => tui::run(Some(input), args.volume),
        (None, None, false) => tui::run(None, args.volume),
    }
}
