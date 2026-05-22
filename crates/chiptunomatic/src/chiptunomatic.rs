use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;
#[cfg(feature = "std")]
use std::{io::Read, path::Path};

use crate::constants::{NOTE_NAMES, PENTATONIC_MINOR, SAMPLE_RATE};
use crate::consumer::Consume;
use crate::plugin::chiptune::ChiptunePlugin;
use crate::plugin::koto::KotoPlugin;
use crate::plugin::metal::MetalPlugin;
use crate::plugin::rap::RapPlugin;
use crate::plugin::rock::RockPlugin;
use crate::plugin::samba::SambaPlugin;
use crate::plugin::toy::ToyPlugin;
use crate::plugin::trap::TrapPlugin;
use crate::plugin::Plugin;
use crate::random::{NoRandom, Random};
use crate::{Arpeggiator, Mixer, NoteGenerator, Sample, SampleDrum, Sampler};
use crate::{SongMetadata, Timing};
use alloc::format;
use alloc::string::{String, ToString};
use getset::{Getters, MutGetters, Setters, WithSetters};
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

#[derive(Debug, Clone, Getters, MutGetters, Setters, WithSetters)]
pub struct Chiptunomatic {
    #[getset(get = "pub", get_mut = "pub", set = "pub", set_with = "pub")]
    plugins: Vec<Box<dyn Plugin>>,
    #[getset(get = "pub", get_mut = "pub", set = "pub", set_with = "pub")]
    plugin: Box<dyn Plugin>,
    #[getset(get = "pub")]
    metadata: SongMetadata,
    #[getset(get = "pub", set = "pub", set_with = "pub")]
    sample_rate: u32,
    note_generator: NoteGenerator,
    #[getset(get = "pub", get_mut = "pub")]
    arpeggiator: Arpeggiator,
    sampler: Sampler,
    drum_samples: SampleDrum,
    #[getset(get = "pub", get_mut = "pub")]
    mixer: Mixer,
    #[getset(set = "pub", set_with = "pub")]
    random: Box<dyn Random>,
}

impl Consume for Chiptunomatic {
    type Item = u8;

    fn push(&mut self, byte: u8) {
        self.note_generator.push(byte);
    }

    fn extend(&mut self, bytes: &[u8]) {
        self.note_generator.extend(bytes);
    }

    fn capacity(&self) -> usize {
        self.note_generator.capacity()
    }

    fn len(&self) -> usize {
        self.note_generator.len()
    }

    fn is_empty(&self) -> bool {
        self.note_generator.is_empty()
    }
}

impl Default for Chiptunomatic {
    fn default() -> Self {
        let plugin = Box::new(ChiptunePlugin::new());
        let mut chiptunomatic = Self {
            plugins: Default::default(),
            plugin: plugin.clone(),
            metadata: Default::default(),
            sample_rate: SAMPLE_RATE,
            note_generator: Default::default(),
            arpeggiator: Default::default(),
            sampler: Default::default(),
            drum_samples: Default::default(),
            mixer: Default::default(),
            random: Box::new(NoRandom::default()),
        };

        chiptunomatic.plugins.push(plugin);
        chiptunomatic
    }
}

impl Chiptunomatic {
    pub fn with_default_plugins(self) -> Self {
        let mut other = Self { ..self };
        other.register_default_plugins();
        other
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> &mut Self {
        self.plugins.push(plugin);
        self
    }

    pub fn register_default_plugins(&mut self) {
        // self.register_plugin(Rc::new(LofiPlugin {}));
        self.register_plugin(Box::new(RockPlugin {}));
        self.register_plugin(Box::new(MetalPlugin {}));
        // self.register_plugin(Rc::new(PersianPlugin {}));
        self.register_plugin(Box::new(RapPlugin {}));
        self.register_plugin(Box::new(TrapPlugin {}));
        self.register_plugin(Box::new(ToyPlugin {}));
        self.register_plugin(Box::new(SambaPlugin {}));
        self.register_plugin(Box::new(KotoPlugin {}));
        // self.register_plugin(Rc::new(MedievalPlugin {}));
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
        self.plugin.mode()
    }

    pub fn mode_string(&self) -> String {
        self.plugin.mode_string()
    }

    /// Set the selected mode
    pub fn set_mode(&mut self, mode: &String) -> Result<(), ChiptunomaticError> {
        if let Some(plugin) = self.plugins.iter().find(|p| p.mode() == mode) {
            self.plugin = plugin.clone();
            Ok(())
        } else {
            Err(ChiptunomaticError::InvalidMode(mode.clone()))
        }
    }

    pub fn position(&self) -> u64 {
        self.note_generator.position
    }

    pub fn melody_time(&self) -> f64 {
        self.note_generator.melody_time
    }

    pub fn bass_time(&self) -> f64 {
        self.note_generator.bass_time
    }

    /// Reset the state
    pub fn reset(&mut self) {
        self.note_generator = Default::default();
        self.sampler = Default::default();
        self.arpeggiator.reset();
        self.mixer.reset();

        // Also reset the RNG
        self.random.set_seed(self.metadata.rng_seed);
    }

    fn set_metadata(&mut self, metadata: SongMetadata) {
        // The RNG must be reset with the new seed
        self.random.set_seed(metadata.rng_seed);

        // Update the drum pattern in the sampler
        self.drum_samples.set_drum_pattern(metadata.drum_pattern);

        self.metadata = metadata;
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
        let bpm = self.plugin.tempo_from_seed(root_seed);
        let timing = Timing::from_bpm(bpm);
        let total_beats = data_byte_len / 3;
        let total_duration = (total_beats as f64) * timing.beat_duration;
        let chord_progression = self.plugin.chord_progression_from_seed(chord_seed);
        let drum_pattern = self.plugin.drum_pattern_from_seed(&drum_pattern_seed);
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
        self.set_metadata(self.song_metadata_from_seed(seed, data_byte_len)?);
        Ok(self.metadata.clone())
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
        self.set_metadata(self.song_metadata_from_path(path)?);
        Ok(self.metadata.clone())
    }

    /// Generate the samples from the input stream
    pub fn samples(&mut self) -> Vec<Sample> {
        let mut samples: Vec<Sample> = Default::default();
        while let Some(sample) = self.next() {
            samples.push(sample);
        }

        samples
    }

    pub fn samples_buf(&mut self, buffer: &mut [Sample]) -> usize {
        let mut samples_written = 0;
        for sample in self {
            buffer[samples_written] = sample;
            samples_written += 1;
        }

        samples_written
    }

    /// Convert a file to WAV. `mode` selects chiptune or lofi synthesis.
    #[cfg(all(feature = "std", feature = "wav"))]
    pub fn file_to_wav<IP: AsRef<Path>, OP: AsRef<Path>>(
        &mut self,
        input_path: IP,
        output_path: OP,
    ) -> Result<(), ConvertWavError> {
        use std::{fs::File, io::BufWriter};

        use hound::{SampleFormat, WavSpec, WavWriter};

        self.reset();
        self.load_song_metadata_from_path(&input_path)?;

        let wav_spec = WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let mut input_file = File::open(&input_path)?;
        let output_file = File::create(&output_path)?;
        let mut wav_writer = WavWriter::new(BufWriter::new(output_file), wav_spec)?;

        // Generate chunk by chunk
        let mut buffer = [0u8; 1024];

        loop {
            // Consume available samples
            use std::io::Read;
            let samples = self.samples();
            for sample in samples {
                let pcm = (sample.value * 32767.0).clamp(-32768.0, 32767.0) as i16;
                wav_writer.write_sample(pcm)?;
            }

            // Push next bytes
            let bytes_read = input_file.read(&mut buffer)?;
            if bytes_read == 0 {
                // Reached EOF and no samples remaining
                break;
            }

            self.note_generator.extend(&buffer[0..bytes_read]);
        }

        wav_writer.finalize()?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn sample_reader<R: Read>(&mut self, reader: R) -> SampleReader<R> {
        SampleReader::new(self, reader)
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

impl Iterator for Chiptunomatic {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Drain available samples first
            if let Some(mut sample) = self.sampler.sample(&self.plugin, self.sample_rate) {
                // Add drums
                sample.noise = self.drum_samples.next(
                    &self.metadata,
                    &self.plugin,
                    self.sample_rate,
                    &mut self.random,
                );

                // Mix
                return Some(self.mixer.mix_sample(sample));
            }

            // Push the next note to generate new samples
            match self.note_generator.note(&self.metadata, &self.plugin) {
                None => return None,
                Some(note) => {
                    // Generate the arpeggios
                    let notes = self
                        .arpeggiator
                        .expand_note(&self.metadata, &self.plugin, &note);

                    self.sampler.extend(notes.as_slice());
                }
            }
        }
    }
}

#[cfg(feature = "std")]
mod iter {
    use std::io::Read;

    use getset::{Getters, MutGetters};

    use crate::{consumer::Consume, Chiptunomatic, Sample};

    #[derive(Getters, MutGetters)]
    pub struct SampleReader<'a, R: Read> {
        #[getset(get = "pub", get_mut = "pub")]
        chiptunomatic: &'a mut Chiptunomatic,
        reader: R,
        buffer: [u8; 1024],
    }

    impl<'a, R: Read> SampleReader<'a, R> {
        pub fn new(chiptunomatic: &'a mut Chiptunomatic, reader: R) -> Self {
            Self {
                chiptunomatic,
                reader,
                buffer: [0; 1024],
            }
        }

        pub fn next_buf(&mut self, buffer: &mut [Sample]) -> std::io::Result<usize> {
            for i in 0..buffer.len() {
                match self.next() {
                    None => return Ok(i),
                    Some(Err(e)) => return Err(e),
                    Some(Ok(sample)) => buffer[i] = sample,
                }
            }

            Ok(buffer.len())
        }
    }

    impl<'a, R: Read> Iterator for SampleReader<'a, R> {
        type Item = std::io::Result<Sample>;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                // Drain available samples first
                if let Some(sample) = self.chiptunomatic.next() {
                    return Some(Ok(sample));
                }

                // Push the next bytes to generate new notes
                match self.reader.read(&mut self.buffer) {
                    Err(e) => return Some(Err(e)),
                    Ok(0) => return None,
                    Ok(n) => self.chiptunomatic.extend(&self.buffer[0..n]),
                }
            }
        }
    }
}

#[cfg(feature = "std")]
pub use iter::*;
