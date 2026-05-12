use crate::{ReadSongNotes, SongMetadata, SongMetadataFromPathError, StdDrumSampleGenerator};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]
pub enum ConvertWavError {
    IoError(#[from] std::io::Error),
    SongMetadataError(#[from] SongMetadataFromPathError),
    HoundError(#[from] hound::Error),
}

/// Convert a file to wav
pub fn file_to_wav<IP: AsRef<Path>, OP: AsRef<Path>>(
    input_path: IP,
    output_path: OP,
    sample_rate: u32,
) -> Result<(), ConvertWavError> {
    use std::{fs::File, io::BufWriter};

    use hound::{SampleFormat, WavSpec, WavWriter};

    let metadata = SongMetadata::from_path(&input_path)?;

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
    let drum_samples = StdDrumSampleGenerator::new(metadata.clone())
        .with_sample_rate(sample_rate)
        .samples();

    // This generates the song samples and mix them with the drum notes
    for mix in ReadSongNotes::new(metadata, input_file)
        .samples()
        .with_sample_rate(sample_rate)
        .mix(drum_samples)
    {
        let mix = mix?;
        let pcm = (mix.frequency * 32767.0).clamp(-32768.0, 32767.0) as i16;
        wav_writer.write_sample(pcm)?;
    }

    wav_writer.finalize()?;
    Ok(())
}
