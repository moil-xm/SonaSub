use std::{
    fs::{self, File},
    io::{BufWriter, Seek, SeekFrom, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

use crate::audio::AudioChunk;

const AUDIO_WRITE_QUEUE_DEPTH: usize = 256;
const HEADER_UPDATE_INTERVAL: Duration = Duration::from_secs(1);
const MAX_GAP_SECS: f64 = 60.0;
const WAV_HEADER_BYTES: u64 = 44;
const MAX_WAV_DATA_BYTES: u64 = u32::MAX as u64 - WAV_HEADER_BYTES;

pub enum AudioWriteOutcome {
    Written,
    Dropped,
    Failed(String),
}

enum AudioWriteCommand {
    Chunk(AudioChunk),
    Finish,
}

/// Writes one capture stream to a float WAV file without touching the
/// realtime callback or blocking the source loop on disk I/O.
pub struct AudioRecorder {
    sender: Sender<AudioWriteCommand>,
    error: Arc<Mutex<Option<String>>>,
    handle: Option<JoinHandle<()>>,
}

impl AudioRecorder {
    pub fn start(path: &Path, sample_rate: u32, channels: u16) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("Audio recording requires a positive sample rate.".to_owned());
        }
        if channels == 0 {
            return Err("Audio recording requires at least one channel.".to_owned());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create the audio recording directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let file = File::create(path).map_err(|error| {
            format!(
                "Failed to create the audio recording {}: {error}",
                path.display()
            )
        })?;
        let (sender, receiver) = bounded(AUDIO_WRITE_QUEUE_DEPTH);
        let error = Arc::new(Mutex::new(None));
        let writer_error = error.clone();
        let handle = thread::Builder::new()
            .name("sonasub-record-audio".to_owned())
            .spawn(move || write_wav(file, sample_rate, channels, receiver, writer_error))
            .map_err(|error| format!("Failed to start the audio recording thread: {error}"))?;
        Ok(Self {
            sender,
            error,
            handle: Some(handle),
        })
    }

    pub fn write(&self, chunk: AudioChunk) -> AudioWriteOutcome {
        if let Some(error) = self.error() {
            return AudioWriteOutcome::Failed(error);
        }
        match self.sender.try_send(AudioWriteCommand::Chunk(chunk)) {
            Ok(()) => AudioWriteOutcome::Written,
            Err(TrySendError::Full(_)) => AudioWriteOutcome::Dropped,
            Err(TrySendError::Disconnected(_)) => AudioWriteOutcome::Failed(
                self.error()
                    .unwrap_or_else(|| "The audio recording writer stopped.".to_owned()),
            ),
        }
    }

    pub fn finish(mut self) -> Result<(), String> {
        let error = self.error.clone();
        let _ = self
            .sender
            .send_timeout(AudioWriteCommand::Finish, Duration::from_secs(5));
        drop(self.sender);
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| "The audio recording thread panicked.".to_owned())?;
        }
        let failure = error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn error(&self) -> Option<String> {
        self.error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn write_wav(
    file: File,
    sample_rate: u32,
    channels: u16,
    receiver: Receiver<AudioWriteCommand>,
    error: Arc<Mutex<Option<String>>>,
) {
    let result = write_wav_inner(file, sample_rate, channels, receiver);
    if let Err(failure) = result {
        *error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(failure);
    }
}

fn write_wav_inner(
    file: File,
    sample_rate: u32,
    channels: u16,
    receiver: Receiver<AudioWriteCommand>,
) -> Result<(), String> {
    let mut writer = BufWriter::new(file);
    let mut data_bytes = 0u64;
    let mut expected_frame = 0u64;
    let mut sample_bytes = Vec::new();
    let mut last_header_update = Instant::now();
    write_wav_header(&mut writer, sample_rate, channels, data_bytes)?;

    while let Ok(command) = receiver.recv() {
        let AudioWriteCommand::Chunk(chunk) = command else {
            break;
        };
        if chunk.sample_rate != sample_rate || chunk.channels != channels {
            return Err(format!(
                "Audio capture changed from {sample_rate} Hz/{channels} channels to {} Hz/{} channels; recording stopped.",
                chunk.sample_rate, chunk.channels
            ));
        }
        let expected_samples = chunk
            .frames
            .checked_mul(usize::from(channels))
            .ok_or_else(|| "Audio chunk sample count overflowed.".to_owned())?;
        if chunk.data.len() != expected_samples {
            return Err("Audio chunk sample count does not match its frame count.".to_owned());
        }
        let chunk_end = chunk.frame_index.saturating_add(chunk.frames as u64);
        if chunk_end <= expected_frame {
            continue;
        }
        let start_frame = chunk.frame_index.max(expected_frame);
        let skipped_frames = start_frame.saturating_sub(chunk.frame_index);
        if start_frame > expected_frame {
            let gap_frames = start_frame - expected_frame;
            let max_gap_frames = (MAX_GAP_SECS * f64::from(sample_rate)) as u64;
            if gap_frames > max_gap_frames {
                return Err(format!(
                    "Audio was missing for more than {MAX_GAP_SECS:.0} seconds; recording stopped so the WAV stays aligned with its captions."
                ));
            }
            let silence_samples = gap_frames.saturating_mul(u64::from(channels));
            data_bytes = write_silence(&mut writer, data_bytes, silence_samples)?;
        }
        let first_sample = usize::try_from(skipped_frames)
            .ok()
            .and_then(|frames| frames.checked_mul(usize::from(channels)))
            .ok_or_else(|| "Audio frame offset is too large.".to_owned())?;
        let samples = chunk
            .data
            .get(first_sample..)
            .ok_or_else(|| "Audio chunk ended before its declared frame count.".to_owned())?;
        if samples.len() % usize::from(channels) != 0 {
            return Err("Audio chunk is not aligned to its channel layout.".to_owned());
        }
        sample_bytes.clear();
        for sample in samples {
            sample_bytes.extend_from_slice(&sample.to_le_bytes());
        }
        data_bytes = append_data_bytes(&mut writer, data_bytes, &sample_bytes)?;
        expected_frame = chunk_end;
        if last_header_update.elapsed() >= HEADER_UPDATE_INTERVAL {
            rewrite_wav_header(&mut writer, sample_rate, channels, data_bytes)?;
            last_header_update = Instant::now();
        }
    }

    rewrite_wav_header(&mut writer, sample_rate, channels, data_bytes)?;
    writer
        .flush()
        .map_err(|error| format!("Failed to flush the audio recording: {error}"))
}

fn append_data_bytes(
    writer: &mut BufWriter<File>,
    data_bytes: u64,
    bytes: &[u8],
) -> Result<u64, String> {
    let next = data_bytes
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| "The audio recording size overflowed.".to_owned())?;
    if next > MAX_WAV_DATA_BYTES {
        return Err(
            "The WAV recording reached its 4 GiB limit; audio recording stopped.".to_owned(),
        );
    }
    writer
        .write_all(bytes)
        .map_err(|error| format!("Failed to write the audio recording: {error}"))?;
    Ok(next)
}

fn write_silence(
    writer: &mut BufWriter<File>,
    data_bytes: u64,
    samples: u64,
) -> Result<u64, String> {
    let bytes = samples
        .checked_mul(4)
        .ok_or_else(|| "The audio gap size overflowed.".to_owned())?;
    let next = data_bytes
        .checked_add(bytes)
        .ok_or_else(|| "The audio recording size overflowed.".to_owned())?;
    if next > MAX_WAV_DATA_BYTES {
        return Err(
            "The WAV recording reached its 4 GiB limit; audio recording stopped.".to_owned(),
        );
    }
    let zeros = [0u8; 4096];
    let mut remaining = bytes;
    while remaining > 0 {
        let chunk = remaining.min(zeros.len() as u64) as usize;
        writer
            .write_all(&zeros[..chunk])
            .map_err(|error| format!("Failed to write the audio gap: {error}"))?;
        remaining -= chunk as u64;
    }
    Ok(next)
}

fn rewrite_wav_header(
    writer: &mut BufWriter<File>,
    sample_rate: u32,
    channels: u16,
    data_bytes: u64,
) -> Result<(), String> {
    writer
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("Failed to seek in the audio recording: {error}"))?;
    write_wav_header(writer, sample_rate, channels, data_bytes)?;
    writer
        .seek(SeekFrom::End(0))
        .map_err(|error| format!("Failed to seek in the audio recording: {error}"))?;
    Ok(())
}

fn write_wav_header(
    writer: &mut impl Write,
    sample_rate: u32,
    channels: u16,
    data_bytes: u64,
) -> Result<(), String> {
    let data_size = u32::try_from(data_bytes)
        .map_err(|_| "The WAV recording reached its 4 GiB limit.".to_owned())?;
    let riff_size = WAV_HEADER_BYTES
        .checked_sub(8)
        .and_then(|header| header.checked_add(u64::from(data_size)))
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| "The WAV recording size overflowed.".to_owned())?;
    let block_align = channels
        .checked_mul(4)
        .ok_or_else(|| "The WAV channel layout overflowed.".to_owned())?;
    let byte_rate = sample_rate
        .checked_mul(u32::from(block_align))
        .ok_or_else(|| "The WAV byte rate overflowed.".to_owned())?;

    writer
        .write_all(b"RIFF")
        .and_then(|()| writer.write_all(&riff_size.to_le_bytes()))
        .and_then(|()| writer.write_all(b"WAVEfmt "))
        .and_then(|()| writer.write_all(&16u32.to_le_bytes()))
        .and_then(|()| writer.write_all(&3u16.to_le_bytes()))
        .and_then(|()| writer.write_all(&channels.to_le_bytes()))
        .and_then(|()| writer.write_all(&sample_rate.to_le_bytes()))
        .and_then(|()| writer.write_all(&byte_rate.to_le_bytes()))
        .and_then(|()| writer.write_all(&block_align.to_le_bytes()))
        .and_then(|()| writer.write_all(&32u16.to_le_bytes()))
        .and_then(|()| writer.write_all(b"data"))
        .and_then(|()| writer.write_all(&data_size.to_le_bytes()))
        .map_err(|error| format!("Failed to write the WAV header: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    fn chunk(frame_index: u64, samples: &[f32], sample_rate: u32, channels: u16) -> AudioChunk {
        AudioChunk {
            data: samples.to_vec(),
            frames: samples.len() / usize::from(channels),
            channels,
            sample_rate,
            frame_index,
            pts_ns: 0,
            discontinuity: false,
            recovered: false,
            dropped_frames_before: 0,
            peak: 0.0,
            rms: 0.0,
        }
    }

    #[test]
    fn writes_native_format_float_wav_and_preserves_gaps() {
        let path = std::env::temp_dir().join(format!(
            "sonasub-wav-{}.wav",
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let recorder = AudioRecorder::start(&path, 8_000, 2).unwrap();
        recorder.write(chunk(0, &[0.25, -0.25], 8_000, 2));
        recorder.write(chunk(2, &[0.5, -0.5], 8_000, 2));
        recorder.finish().unwrap();

        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[20..22], 3u16.to_le_bytes());
        assert_eq!(&bytes[22..24], 2u16.to_le_bytes());
        assert_eq!(&bytes[24..28], 8_000u32.to_le_bytes());
        assert_eq!(&bytes[34..36], 32u16.to_le_bytes());
        assert_eq!(&bytes[40..44], 24u32.to_le_bytes());
        assert_eq!(bytes.len(), 44 + 3 * 2 * 4);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_a_sample_rate_change() {
        let path = std::env::temp_dir().join(format!(
            "sonasub-wav-{}.wav",
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let recorder = AudioRecorder::start(&path, 8_000, 1).unwrap();
        recorder.write(chunk(0, &[0.1], 8_000, 1));
        recorder.write(chunk(1, &[0.2], 16_000, 1));
        assert!(recorder.finish().is_err());
        fs::remove_file(path).unwrap();
    }
}
