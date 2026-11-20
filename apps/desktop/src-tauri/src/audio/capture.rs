use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use cpal::traits::{DeviceTrait, StreamTrait};
use crossbeam_channel::{Sender, TrySendError};

use super::{
    convert,
    devices::{self, CapturePlan},
    AudioChunk, AudioSourceKind, CaptureEvent, CaptureInfo, CaptureOptions, StartedCapture,
};

const STREAM_ERROR_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STREAM_ACTIVATION_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn start(
    options: CaptureOptions,
    events: Sender<CaptureEvent>,
    stop: Arc<AtomicBool>,
) -> Result<StartedCapture, String> {
    let mut plan = devices::resolve(options.kind, options.device_id.as_deref())?;
    let info = capture_info(&plan);
    // A macOS process tap is only kept alive by its private aggregate device.
    // Move the Core Audio ownership into the capture thread so the aggregate
    // cannot be destroyed while CPAL still has an open input stream.
    let keepalive = plan.keepalive.take();
    let stream = build_stream(&plan, events, stop.clone())?;
    stream
        .play()
        .map_err(|error| stream_error("start", options.kind, &plan.device_name, error))?;
    let thread = thread::Builder::new()
        .name(format!(
            "sonasub-capture-{}",
            options.kind.as_str().to_lowercase()
        ))
        .spawn(move || {
            let _keepalive = keepalive;
            let _stream = stream;
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(STREAM_ERROR_POLL_INTERVAL);
            }
        })
        .map_err(|error| format!("Failed to start the audio capture thread: {error}"))?;
    Ok(StartedCapture { info, thread })
}

fn build_stream(
    plan: &CapturePlan,
    events: Sender<CaptureEvent>,
    stop: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    let sample_format = plan.config.sample_format();
    let sample_rate = plan.config.sample_rate();
    let channels = plan.config.channels();
    let mut next_frame = 0u64;
    let mut dropped_frames = 0u64;
    let recovered = Arc::new(AtomicBool::new(false));
    let callback_recovered = recovered.clone();
    let callback_events = events.clone();
    let callback_stop = stop.clone();
    let error_events = events;
    let error_stop = stop;
    let replay_kind = plan.mode;
    let device_name = plan.device_name.clone();

    plan.device
        .build_input_stream_raw(
            plan.config.config(),
            sample_format,
            move |data, info| {
                let frames = data.len() / channels as usize;
                if frames == 0 {
                    return;
                }
                // A rejected callback still represents elapsed native time.
                // Advance the frame index before dropping it so the next
                // delivered chunk cannot be placed before the real gap.
                let frame_index = next_frame;
                next_frame = next_frame.saturating_add(frames as u64);
                if callback_events.is_full() {
                    dropped_frames = dropped_frames.saturating_add(frames as u64);
                    return;
                }
                if data.sample_format() != sample_format {
                    let _ = callback_events.try_send(CaptureEvent::Fatal(format!(
                        "The native audio stream changed from `{sample_format}` to `{}`; \
                         restart the capture stream.",
                        data.sample_format()
                    )));
                    callback_stop.store(true, Ordering::SeqCst);
                    return;
                }
                let samples = match convert::data_to_f32(data) {
                    Ok(samples) => samples,
                    Err(error) => {
                        let _ = callback_events.try_send(CaptureEvent::Fatal(error));
                        callback_stop.store(true, Ordering::SeqCst);
                        return;
                    }
                };
                let mut peak = 0.0f32;
                let mut energy = 0.0f64;
                for sample in &samples {
                    peak = peak.max(sample.abs());
                    energy += f64::from(*sample) * f64::from(*sample);
                }
                let rms = (energy / samples.len() as f64).sqrt() as f32;
                let chunk = AudioChunk {
                    data: samples,
                    frames,
                    channels,
                    sample_rate,
                    frame_index,
                    pts_ns: i64::try_from(info.timestamp().capture.as_nanos()).unwrap_or(i64::MAX),
                    discontinuity: dropped_frames > 0,
                    recovered: callback_recovered.swap(false, Ordering::SeqCst),
                    dropped_frames_before: dropped_frames,
                    peak,
                    rms,
                };
                match callback_events.try_send(CaptureEvent::Chunk(chunk)) {
                    Ok(()) => dropped_frames = 0,
                    Err(TrySendError::Full(_)) => {
                        dropped_frames = dropped_frames.saturating_add(frames as u64);
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        callback_stop.store(true, Ordering::SeqCst);
                    }
                }
            },
            move |error| {
                let message = stream_error("read", replay_kind.kind(), &device_name, error.clone());
                match error.kind() {
                    cpal::ErrorKind::DeviceChanged => {
                        // CPAL rerouted a default stream automatically. Mark the
                        // next callback so the downstream timeline records the
                        // discontinuity at the real audio boundary.
                        recovered.store(true, Ordering::SeqCst);
                        let _ = error_events.try_send(CaptureEvent::Warning(message));
                    }
                    cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated => {
                        let _ = error_events.try_send(CaptureEvent::DeviceLost(message));
                    }
                    cpal::ErrorKind::Xrun | cpal::ErrorKind::RealtimeDenied => {
                        let _ = error_events.try_send(CaptureEvent::Warning(message));
                    }
                    _ => {
                        let _ = error_events.try_send(CaptureEvent::Fatal(message));
                        error_stop.store(true, Ordering::SeqCst);
                    }
                }
            },
            Some(STREAM_ACTIVATION_TIMEOUT),
        )
        .map_err(|error| stream_error("open", plan.mode.kind(), &plan.device_name, error))
}

fn capture_info(plan: &CapturePlan) -> CaptureInfo {
    CaptureInfo {
        source_kind: plan.mode.kind(),
        device_id: plan.device_id.clone(),
        device_name: plan.device_name.clone(),
        sample_rate: plan.config.sample_rate(),
        channels: plan.config.channels(),
        sample_format: plan.config.sample_format().to_string(),
    }
}

fn stream_error(
    operation: &str,
    kind: AudioSourceKind,
    device_name: &str,
    error: cpal::Error,
) -> String {
    let permission = match kind {
        AudioSourceKind::SystemOutput => {
            "Allow SonaSub to record system audio in System Settings > Privacy & Security > Screen & System Audio Recording."
        }
        AudioSourceKind::Microphone => {
            "Allow SonaSub to use the microphone in System Settings > Privacy & Security > Microphone."
        }
    };
    let detail = match error.kind() {
        cpal::ErrorKind::PermissionDenied => format!(
            "Permission to capture {} with `{device_name}` was denied. {permission}",
            kind.label().to_lowercase()
        ),
        cpal::ErrorKind::DeviceNotAvailable => format!(
            "The selected {} device `{device_name}` is no longer available. Reconnect it and restart the session.",
            kind.label().to_lowercase()
        ),
        cpal::ErrorKind::DeviceBusy => format!(
            "The selected {} device `{device_name}` is busy. Close the application using it and restart the session.",
            kind.label().to_lowercase()
        ),
        cpal::ErrorKind::UnsupportedConfig => format!(
            "`{device_name}` rejected its reported {} format: {error}",
            kind.label().to_lowercase()
        ),
        cpal::ErrorKind::StreamInvalidated => format!(
            "The {} stream for `{device_name}` became invalid and must be restarted: {error}",
            kind.label().to_lowercase()
        ),
        _ => format!(
            "Failed to {operation} the {} stream for `{device_name}`: {error}",
            kind.label().to_lowercase()
        ),
    };
    detail
}
