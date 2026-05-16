#[cfg(feature = "std")]
use crate::ReadSongNotes;
use core::cell::RefCell;
use core::fmt::Debug;
#[cfg(feature = "std")]
use std::{io::Read, path::Path};

use crate::constants::{NOTE_NAMES, PENTATONIC_MINOR, SAMPLE_RATE};
use crate::plugin::chiptune::ChiptunePlugin;
use crate::plugin::koto::KotoPlugin;
use crate::plugin::metal::MetalPlugin;
use crate::plugin::rap::RapPlugin;
use crate::plugin::rock::RockPlugin;
use crate::plugin::samba::SambaPlugin;
use crate::plugin::toy::ToyPlugin;
use crate::plugin::trap::TrapPlugin;
use crate::plugin::{Plugin, Random};
use crate::{DrumSampleGenerator, MixGenerator, SampleDrumSteps, SongNoteReader};
use crate::{SongMetadata, Timing};
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use rand::{Rng, SeedableRng};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GenerateError {
    #[error("empty input")]
    EmptyInput,
    #[error("invalid seed")]
    InvalidSeed,
}

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
#[derive(Error, Debug)]
pub enum SongMetadataFromPathError {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("file too large")]
    FileTooLarge,
    #[error("error generating")]
    Generate(#[from] GenerateError),
}

#[cfg(all(
    feature = "std",
    feature = "wav",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
#[derive(Error, Debug)]
#[error(transparent)]
pub enum ConvertWavError {
    IoError(#[from] std::io::Error),
    SongMetadataError(#[from] SongMetadataFromPathError),
    HoundError(#[from] hound::Error),
}

#[derive(Error, Debug)]
pub enum ChiptunomaticError {
    #[error("no mode {0}")]
    InvalidMode(String),
}

pub struct RandomWrapper<R: Rng + ?Sized> {
    rng: Rc<RefCell<R>>,
}

impl<R: Rng + ?Sized> Debug for RandomWrapper<R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RandomWrapper").finish()
    }
}

impl<R: Rng + SeedableRng> RandomWrapper<R> {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            rng: Rc::new(RefCell::new(R::seed_from_u64(seed))),
        }
    }
}

impl<R: Rng + ?Sized> Random for RandomWrapper<R> {
    fn next_float(&self) -> f32 {
        self.rng.borrow_mut().gen::<f32>()
    }
}

pub struct Chiptunomatic {
    plugins: Vec<Rc<dyn Plugin>>,
    selected_plugin: Rc<dyn Plugin>,
    song_metadata: SongMetadata,
    sample_rate: u32,
}

impl Chiptunomatic {
    /// Initialize with the default plugin
    pub fn new() -> Self {
        let plugin = Rc::new(ChiptunePlugin::new());
        let mut chiptunomatic = Chiptunomatic {
            plugins: Default::default(),
            selected_plugin: plugin.clone(),
            song_metadata: Default::default(),
            sample_rate: SAMPLE_RATE,
        };

        chiptunomatic.plugins.push(plugin);
        chiptunomatic
    }

    pub fn with_default_plugins(self) -> Self {
        let mut other = Self { ..self };
        other.register_default_plugins();
        other
    }

    pub fn with_sample_rate(self, sample_rate: u32) -> Self {
        Self {
            sample_rate,
            ..self
        }
    }

    pub fn register_plugin(&mut self, plugin: Rc<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn register_default_plugins(&mut self) {
        // self.register_plugin(Rc::new(LofiPlugin {}));
        self.register_plugin(Rc::new(RockPlugin {}));
        self.register_plugin(Rc::new(MetalPlugin {}));
        // self.register_plugin(Rc::new(PersianPlugin {}));
        self.register_plugin(Rc::new(RapPlugin {}));
        self.register_plugin(Rc::new(TrapPlugin {}));
        self.register_plugin(Rc::new(ToyPlugin {}));
        self.register_plugin(Rc::new(SambaPlugin {}));
        self.register_plugin(Rc::new(KotoPlugin {}));
        // self.register_plugin(Rc::new(MedievalPlugin {}));
    }

    pub fn plugin(&self) -> Rc<dyn Plugin> {
        self.selected_plugin.clone()
    }

    /// Return the possible modes
    pub fn modes(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|p| p.mode()).collect()
    }

    pub fn modes_string(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.mode_string()).collect()
    }

    /// Get the selected mode
    pub fn mode(&self) -> &'static str {
        self.selected_plugin.mode()
    }

    pub fn mode_string(&self) -> String {
        self.selected_plugin.mode_string()
    }

    /// Set the selected mode
    pub fn set_mode(&mut self, mode: &String) -> Result<(), ChiptunomaticError> {
        if let Some(plugin) = self.plugins.iter().find(|p| p.mode() == mode) {
            self.selected_plugin = plugin.clone();
            Ok(())
        } else {
            Err(ChiptunomaticError::InvalidMode(mode.clone()))
        }
    }

    /// Builds [`SongMetadata`] from a bytes seed and stream byte length.
    pub fn song_metadata_from_seed(
        &self,
        seed: &[u8],
        data_byte_len: u64,
    ) -> Result<SongMetadata, GenerateError> {
        if data_byte_len == 0 {
            return Err(GenerateError::EmptyInput);
        }
        let digest = md5::compute(seed).0;
        let rng_seed = u64::from(
            u32::from_be_bytes([digest[12], digest[13], digest[14], digest[15]]) % (1u32 << 31),
        );

        let root_seed = u32::from_be_bytes(
            digest[0..4]
                .try_into()
                .map_err(|_| GenerateError::InvalidSeed)?,
        );
        let chord_seed = u32::from_be_bytes(
            digest[4..8]
                .try_into()
                .map_err(|_| GenerateError::InvalidSeed)?,
        );
        let drum_pattern_seed: [u8; 8] = digest[8..16]
            .try_into()
            .map_err(|_| GenerateError::InvalidSeed)?;

        let root = (root_seed % 12) as u8;
        let bpm = self.selected_plugin.tempo_from_seed(root_seed);
        let timing = Timing::from_bpm(bpm);
        let total_beats = data_byte_len / 3;
        let total_duration = (total_beats as f64) * timing.beat_duration;
        let chord_progression = self.selected_plugin.chord_progression_from_seed(chord_seed);
        let drum_pattern = self
            .selected_plugin
            .drum_pattern_from_seed(&drum_pattern_seed);
        let chord_description = Self::build_chord_description(root, &chord_progression);

        Ok(SongMetadata {
            seed: digest.into(),
            rng_seed,
            root_semitone: root,
            bpm,
            total_beats,
            total_duration,
            total_duration_str: Self::format_duration(total_duration),
            chord_progression,
            chord_description,
            timing,
            data_byte_len,
            data_byte_len_str: Self::format_data_byte_size(data_byte_len),
            drum_pattern,
            drum_seed: drum_pattern_seed,
        })
    }

    /// Builds [`SongMetadata`] from a bytes seed and stream byte length.
    pub fn load_song_metadata_from_seed(
        &mut self,
        seed: &[u8],
        data_byte_len: u64,
    ) -> Result<SongMetadata, GenerateError> {
        self.song_metadata = self.song_metadata_from_seed(seed, data_byte_len)?;
        Ok(self.song_metadata.clone())
    }

    pub fn song_metadata_from_string<S: Into<String>>(
        &self,
        s: S,
        data_byte_len: u64,
    ) -> Result<SongMetadata, GenerateError> {
        self.song_metadata_from_seed(s.into().as_bytes(), data_byte_len)
    }

    pub fn load_song_metadata_from_string<S: Into<String>>(
        &mut self,
        s: S,
        data_byte_len: u64,
    ) -> Result<SongMetadata, GenerateError> {
        self.load_song_metadata_from_seed(s.into().as_bytes(), data_byte_len)
    }

    #[cfg(all(
        feature = "std",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ))]
    pub fn song_metadata_from_path<P: std::convert::AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<SongMetadata, SongMetadataFromPathError> {
        let path = path.as_ref();
        let md = std::fs::metadata(path).map_err(SongMetadataFromPathError::Io)?;

        let seed_name = path
            .file_name()
            .map(|os| os.to_string_lossy())
            .unwrap_or_else(|| path.as_os_str().to_string_lossy());

        self.song_metadata_from_string(seed_name, md.len())
            .map_err(SongMetadataFromPathError::Generate)
    }

    #[cfg(all(
        feature = "std",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ))]
    pub fn load_song_metadata_from_path<P: std::convert::AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<SongMetadata, SongMetadataFromPathError> {
        self.song_metadata = self.song_metadata_from_path(path)?;
        Ok(self.song_metadata.clone())
    }

    /// Return a reader for the notes of a byte stream
    pub fn song_note_reader(&self) -> SongNoteReader {
        SongNoteReader::new(self.song_metadata.clone(), self.selected_plugin.clone())
    }

    #[cfg(feature = "std")]
    /// Iterate over the notes of a byte stream
    pub fn read_song_notes<R: Read>(&self, reader: R) -> ReadSongNotes<R, SongNoteReader> {
        ReadSongNotes::new(reader, self.song_note_reader())
    }

    /// Return a generator for the drum samples
    pub fn drum_sample_generator<R: Rng + SeedableRng + 'static>(&self) -> DrumSampleGenerator {
        DrumSampleGenerator::new(
            self.song_metadata.clone(),
            self.selected_plugin.clone(),
            Rc::new(RandomWrapper::<R>::from_seed(self.song_metadata.rng_seed)),
        )
        .with_sample_rate(self.sample_rate)
    }

    /// Iterate over the samples of the drum steps
    pub fn sample_drum_steps<R: Rng + SeedableRng + 'static>(&self) -> SampleDrumSteps {
        SampleDrumSteps::from_generator(
            self.song_metadata.clone(),
            self.drum_sample_generator::<R>(),
        )
    }

    /// Convert a file to WAV. `mode` selects chiptune or lofi synthesis.
    #[cfg(all(feature = "std", feature = "wav"))]
    pub fn file_to_wav<IP: AsRef<Path>, OP: AsRef<Path>>(
        &mut self,
        input_path: IP,
        output_path: OP,
        sample_rate: u32,
        mix_generator: MixGenerator,
    ) -> Result<(), ConvertWavError> {
        use std::{fs::File, io::BufWriter};

        use hound::{SampleFormat, WavSpec, WavWriter};
        use rand::rngs::StdRng;

        self.load_song_metadata_from_path(&input_path)?;

        let wav_spec = WavSpec {
            channels: 1,
            sample_rate: sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let input_file = File::open(&input_path)?;
        let output_file = File::create(&output_path)?;
        let mut wav_writer = WavWriter::new(BufWriter::new(output_file), wav_spec)?;

        // This iterator generates the drum samples on demand using StdRng
        let drum_samples = self.sample_drum_steps::<StdRng>();

        // This generates the song samples and mix them with the drum notes
        for mix in self
            .read_song_notes(input_file)
            .samples()
            .with_sample_rate(sample_rate)
            .mix(drum_samples)
            .with_mix_generator(mix_generator)
        {
            let mix = mix?;
            let pcm = (mix.frequency * 32767.0).clamp(-32768.0, 32767.0) as i16;
            wav_writer.write_sample(pcm)?;
        }

        wav_writer.finalize()?;
        Ok(())
    }

    fn format_duration(secs: f64) -> String {
        let total = secs.floor() as u64;
        if total < 60 {
            format!("{total}s")
        } else {
            let m = total / 60;
            let s = total % 60;
            format!("{m}:{s:02}")
        }
    }

    /// Formats byte length using `K` / `M` / `G` at 1024-based thresholds (`>= 1 KiB`, etc.).
    fn format_data_byte_size(n: u64) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        const GB: f64 = MB * 1024.0;
        let x = n as f64;
        if n >= 1 << 30 {
            format!("{:.2} G", x / GB)
        } else if n >= 1 << 20 {
            format!("{:.2} M", x / MB)
        } else if n >= 1 << 10 {
            format!("{:.2} K", x / KB)
        } else {
            format!("{n} B")
        }
    }

    fn build_chord_description(root: u8, chord_progression: &[usize]) -> alloc::string::String {
        chord_progression
            .iter()
            .map(|&d| {
                let note_idx = (root as usize + usize::from(PENTATONIC_MINOR[d])) % 12;
                NOTE_NAMES[note_idx].to_string()
            })
            .collect::<alloc::vec::Vec<_>>()
            .join(" – ")
    }
}
