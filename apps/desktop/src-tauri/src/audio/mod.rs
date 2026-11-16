pub mod dsp;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossbeam_channel::{Sender, TrySendError};
use flexaudio::{OutputFormat, SourceKind, StreamConfig};
use serde::Serialize;

pub const CAPTURE_RATE: u32 = 48_000;
pub const CAPTURE_CHANNELS: u16 = 2;
pub const CAPTURE_CHUNK_MS: u32 = 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemOutput {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub data: Vec<f32>,
    pub frames: usize,
    pub channels: u16,
    pub frame_index: u64,
    pub pts_ns: i64,
    pub discontinuity: bool,
    pub recovered: bool,
    pub dropped_before: u32,
    pub peak: f32,
    pub rms: f32,
}

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    Chunk(AudioChunk),
    Warning(String),
    Fatal(String),
}

#[derive(Debug, Clone)]
pub struct CaptureOptions {
    pub device_id: Option<String>,
}

/// Opens the macOS system-output capture through Core Audio process taps and
/// starts the bounded delivery thread. The function waits for the native
/// stream to start so permission and device errors reach the caller instead of
/// leaving a session that looks ready but has no audio.
pub fn start(
    options: CaptureOptions,
    events: Sender<CaptureEvent>,
    stop: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    if !is_supported() {
        return Err(unsupported_capture_error());
    }
    let (startup_tx, startup_rx) = mpsc::channel::<Result<(), String>>();
    let thread_stop = stop.clone();
    let handle = thread::Builder::new()
        .name("sonasub-audio".to_owned())
        .spawn(move || run_capture(options, events, thread_stop, startup_tx))
        .map_err(|error| format!("Failed to start the audio capture thread: {error}"))?;
    match startup_rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(error)) => {
            let _ = handle.join();
            Err(error)
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            Err("The system audio stream did not start within 15 seconds.".to_owned())
        }
    }
}

fn run_capture(
    options: CaptureOptions,
    events: Sender<CaptureEvent>,
    stop: Arc<AtomicBool>,
    startup: mpsc::Sender<Result<(), String>>,
) {
    let config = StreamConfig {
        kind: SourceKind::SystemLoopback,
        device_id: options.device_id.filter(|id| !id.is_empty()),
        chunk_ms: CAPTURE_CHUNK_MS,
        ring_capacity_chunks: 250,
        exclude_self: true,
        silence_probe: true,
        output: OutputFormat {
            sample_rate: CAPTURE_RATE,
            channels: CAPTURE_CHANNELS,
        },
        ..StreamConfig::default()
    };
    let mut stream = match flexaudio::open(config) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = startup.send(Err(format_system_audio_error(&error)));
            return;
        }
    };
    if let Err(error) = stream.start() {
        let _ = startup.send(Err(format_system_audio_error(&error)));
        stream.stop();
        return;
    }
    let _ = startup.send(Ok(()));

    let mut dropped_chunks: u64 = 0;
    let mut last_drop_notice = Instant::now() - Duration::from_secs(10);
    while !stop.load(Ordering::SeqCst) {
        let mut received = false;
        while let Some(chunk) = stream.poll_chunk() {
            received = true;
            let event = CaptureEvent::Chunk(AudioChunk {
                data: chunk.data,
                frames: chunk.frames,
                channels: CAPTURE_CHANNELS,
                frame_index: chunk.frame_index,
                pts_ns: chunk.pts_ns,
                discontinuity: chunk.flags.contains(flexaudio::ChunkFlags::DISCONTINUITY),
                recovered: chunk.flags.contains(flexaudio::ChunkFlags::RECOVERED),
                dropped_before: chunk.dropped_before,
                peak: chunk.peak,
                rms: chunk.rms,
            });
            match events.try_send(event) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    dropped_chunks += 1;
                    if last_drop_notice.elapsed() >= Duration::from_secs(1) {
                        last_drop_notice = Instant::now();
                        let _ = events.try_send(CaptureEvent::Warning(format!(
                            "The audio pipeline is behind. {dropped_chunks} capture chunks were dropped; the capture timeline records the gap."
                        )));
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    stream.stop();
                    return;
                }
            }
        }
        while let Some(event) = stream.poll_event() {
            received = true;
            match event {
                flexaudio::Event::ChunkDropped { count } => {
                    let _ = events.try_send(CaptureEvent::Warning(format!(
                        "The capture ring dropped {count} chunks; audio gaps are reflected in subtitle timing."
                    )));
                }
                flexaudio::Event::StreamStalled => {
                    let _ = events.try_send(CaptureEvent::Warning(
                        "System audio stopped arriving; waiting for the capture device to recover."
                            .to_owned(),
                    ));
                }
                flexaudio::Event::StreamRecovered => {
                    let _ = events.try_send(CaptureEvent::Warning(
                        "System audio capture recovered; the new segment starts after the gap."
                            .to_owned(),
                    ));
                }
                flexaudio::Event::DeviceLost => {
                    let _ = events.try_send(CaptureEvent::Warning(
                        "The selected system audio device was lost. Capture will recover on the current default output."
                            .to_owned(),
                    ));
                }
                flexaudio::Event::TerminalError { error } => {
                    let _ = events.try_send(CaptureEvent::Fatal(format!(
                        "System audio capture stopped: {error}"
                    )));
                    stream.stop();
                    return;
                }
                flexaudio::Event::PermissionDenied { permission, detail } => {
                    let _ = events.try_send(CaptureEvent::Fatal(format!(
                        "Audio permission was denied ({permission:?}): {detail}"
                    )));
                    stream.stop();
                    return;
                }
                flexaudio::Event::SilenceWhileSourceActive { detail } => {
                    let _ = events.try_send(CaptureEvent::Warning(detail));
                }
                _ => {}
            }
        }
        if !received {
            thread::sleep(Duration::from_millis(5));
        }
    }
    stream.stop();
}

fn format_system_audio_error(error: &flexaudio::Error) -> String {
    match error {
        flexaudio::Error::PermissionDenied { .. } => format!(
            "System audio permission was denied. Allow SonaSub to record system audio in System Settings > Privacy & Security, then start again. {error}"
        ),
        flexaudio::Error::Unsupported => {
            "System audio capture is unavailable on this platform.".to_owned()
        }
        _ => format!("Failed to open macOS system audio capture: {error}"),
    }
}

pub fn is_supported() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

pub fn unsupported_capture_error() -> String {
    format!(
        "System audio capture is implemented and verified only on macOS 14.4 or later arm64; this build is running on {}-{}.",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// Enumerates output endpoints without asking the microphone provider, which
/// can fail independently of system-audio permission on macOS.
pub fn system_outputs() -> Result<Vec<SystemOutput>, String> {
    if !is_supported() {
        return Ok(Vec::new());
    }
    #[cfg(target_os = "macos")]
    {
        let devices = flexaudio_os_macos::list_output_devices()
            .map_err(|error| format!("Failed to enumerate macOS system audio outputs: {error}"))?;
        return Ok(devices
            .into_iter()
            .map(|device| SystemOutput {
                id: device.id.clone(),
                name: if device.name.is_empty() {
                    device.id
                } else {
                    device.name
                },
                is_default: device.is_default,
            })
            .collect());
    }
    #[allow(unreachable_code)]
    Ok(Vec::new())
}

pub fn default_system_output() -> Option<String> {
    system_outputs()
        .ok()?
        .into_iter()
        .find(|device| device.is_default)
        .map(|device| device.id)
}
