#![cfg(target_os = "macos")]

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::bounded;
use sonasub_desktop_lib::audio::{
    self, AudioDevice, AudioDeviceRef, AudioSourceKind, CaptureEvent, CaptureInfo, StartedCapture,
};

#[derive(Default)]
struct CaptureResult {
    info: Option<CaptureInfo>,
    frames: u64,
    peak: f32,
    fatal: Vec<String>,
}

fn preferred_device(kind: AudioSourceKind) -> Option<AudioDevice> {
    let devices = audio::devices(kind).ok()?;
    let default_id = audio::default_device(kind).ok().flatten();
    devices
        .iter()
        .find(|device| {
            device.capture_supported && default_id.as_deref().is_some_and(|id| id == device.id)
        })
        .or_else(|| devices.iter().find(|device| device.capture_supported))
        .cloned()
}

fn start_capture(
    kind: AudioSourceKind,
    device: &AudioDevice,
) -> (
    StartedCapture,
    crossbeam_channel::Receiver<CaptureEvent>,
    Arc<AtomicBool>,
) {
    let device_ref = AudioDeviceRef::new(kind, device.id.clone());
    assert_eq!(device_ref.kind, kind);
    let (events_tx, events_rx) = bounded(512);
    let stop = Arc::new(AtomicBool::new(false));
    let capture = audio::start(
        audio::CaptureOptions {
            kind,
            device_id: Some(device.id.clone()),
        },
        events_tx,
        stop.clone(),
    )
    .unwrap_or_else(|error| panic!("{kind:?} capture failed: {error}"));
    (capture, events_rx, stop)
}

fn finish_capture(
    capture: StartedCapture,
    events: crossbeam_channel::Receiver<CaptureEvent>,
    stop: Arc<AtomicBool>,
) -> CaptureResult {
    stop.store(true, Ordering::SeqCst);
    capture.thread.join().expect("capture thread joins");
    let mut result = CaptureResult {
        info: Some(capture.info),
        ..CaptureResult::default()
    };
    for event in events.try_iter() {
        match event {
            CaptureEvent::Chunk(chunk) => {
                result.frames = result.frames.saturating_add(chunk.frames as u64);
                result.peak = result.peak.max(chunk.peak);
            }
            CaptureEvent::Warning(message) => eprintln!("capture warning: {message}"),
            CaptureEvent::DeviceLost(message) | CaptureEvent::Fatal(message) => {
                result.fatal.push(message);
            }
        }
    }
    result
}

fn write_tone(path: &std::path::Path) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("creates test WAV");
    for frame in 0..(44_100 * 3) {
        let value = (2.0 * std::f64::consts::PI * 440.0 * frame as f64 / 44_100.0).sin();
        let sample = (value * 0.2 * i16::MAX as f64) as i16;
        writer.write_sample(sample).expect("writes left sample");
        writer.write_sample(sample).expect("writes right sample");
    }
    writer.finalize().expect("finalizes test WAV");
}

#[test]
#[ignore = "requires real macOS audio hardware and permissions"]
fn captures_system_output_and_microphone_simultaneously() {
    if !audio::is_supported() {
        eprintln!("audio capture is unsupported on this platform");
        return;
    }
    let system =
        preferred_device(AudioSourceKind::SystemOutput).expect("a capturable system output device");
    let microphone =
        preferred_device(AudioSourceKind::Microphone).expect("a capturable microphone device");
    eprintln!(
        "system output: {} ({:?}), microphone: {} ({:?})",
        system.name, system.id, microphone.name, microphone.id
    );

    let (system_capture, system_events, system_stop) =
        start_capture(AudioSourceKind::SystemOutput, &system);
    let (microphone_capture, microphone_events, microphone_stop) =
        start_capture(AudioSourceKind::Microphone, &microphone);

    let tone = std::env::temp_dir().join(format!("sonasub-cpal-test-{}.wav", uuid::Uuid::new_v4()));
    write_tone(&tone);
    let started = Instant::now();
    let mut playback = std::process::Command::new("/usr/bin/afplay")
        .arg(&tone)
        .spawn()
        .expect("starts afplay");
    let _ = playback.wait();
    if started.elapsed() < Duration::from_millis(2_500) {
        thread::sleep(Duration::from_millis(2_500) - started.elapsed());
    }

    let system = finish_capture(system_capture, system_events, system_stop);
    let microphone = finish_capture(microphone_capture, microphone_events, microphone_stop);
    let _ = std::fs::remove_file(tone);

    let system_info = system.info.as_ref().expect("system capture info");
    let microphone_info = microphone.info.as_ref().expect("microphone capture info");
    eprintln!(
        "system: {} Hz / {} ch / {}, {} frames, peak {:.4}",
        system_info.sample_rate,
        system_info.channels,
        system_info.sample_format,
        system.frames,
        system.peak
    );
    eprintln!(
        "microphone: {} Hz / {} ch / {}, {} frames",
        microphone_info.sample_rate,
        microphone_info.channels,
        microphone_info.sample_format,
        microphone.frames
    );
    assert!(system.fatal.is_empty(), "{:?}", system.fatal);
    assert!(microphone.fatal.is_empty(), "{:?}", microphone.fatal);
    assert!(system_info.sample_rate > 0);
    assert!(microphone_info.sample_rate > 0);
    assert!(
        system.frames > 0,
        "system-output loopback delivered no audio"
    );
    assert!(
        system.peak > 0.01,
        "system-output loopback delivered only silence"
    );
    assert!(microphone.frames > 0, "microphone delivered no audio");
}
