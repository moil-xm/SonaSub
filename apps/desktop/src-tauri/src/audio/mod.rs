mod capture;
mod convert;
pub mod devices;
pub mod dsp;
pub(crate) mod resample;

use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::JoinHandle,
};

use serde::{Deserialize, Serialize};

/// The rate consumed by every downstream speech model.
pub const ASR_RATE: u32 = 16_000;
pub const ASR_CHANNELS: u16 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioSourceKind {
    #[default]
    SystemOutput,
    Microphone,
}

impl AudioSourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::SystemOutput => "System audio",
            Self::Microphone => "Microphone",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemOutput => "systemOutput",
            Self::Microphone => "microphone",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "systemOutput" => Ok(Self::SystemOutput),
            "microphone" => Ok(Self::Microphone),
            _ => Err(format!("Unknown audio source kind `{value}`.")),
        }
    }
}

/// A stable CPAL device reference. The display name is deliberately not part
/// of the identity used by configuration, records or claims.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceRef {
    pub kind: AudioSourceKind,
    pub id: String,
}

impl AudioDeviceRef {
    pub fn new(kind: AudioSourceKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub kind: AudioSourceKind,
    pub name: String,
    pub is_default: bool,
    /// Whether this app can open the requested direction through CPAL.
    pub capture_supported: bool,
    /// A user-facing explanation when `capture_supported` is false.
    pub capture_reason: Option<String>,
    /// The device's current stream format. This is informative; capture does
    /// not assume it stays constant and records the actual callback format.
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub sample_format: Option<String>,
}

impl AudioDevice {
    pub fn device_ref(&self) -> AudioDeviceRef {
        AudioDeviceRef::new(self.kind, self.id.clone())
    }
}

#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// Interleaved f32 samples in the callback's native channel layout.
    pub data: Vec<f32>,
    pub frames: usize,
    pub channels: u16,
    pub sample_rate: u32,
    /// Absolute input frame index for this chunk's first frame.
    pub frame_index: u64,
    /// Native capture timestamp in nanoseconds when the backend provides one.
    pub pts_ns: i64,
    pub discontinuity: bool,
    pub recovered: bool,
    /// Frames dropped between the previous delivered chunk and this one.
    pub dropped_frames_before: u64,
    pub peak: f32,
    pub rms: f32,
}

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    Chunk(AudioChunk),
    Warning(String),
    DeviceLost(String),
    Fatal(String),
}

#[derive(Debug, Clone)]
pub struct CaptureOptions {
    pub kind: AudioSourceKind,
    /// `None` follows the current platform default for `kind`.
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureInfo {
    pub source_kind: AudioSourceKind,
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

pub struct StartedCapture {
    pub info: CaptureInfo,
    pub thread: JoinHandle<()>,
}

/// Opens and starts a CPAL capture stream. The function waits for the native
/// stream to start so permission, device and configuration errors reach the
/// caller instead of leaving a session that looks ready but has no audio.
pub fn start(
    options: CaptureOptions,
    events: crossbeam_channel::Sender<CaptureEvent>,
    stop: Arc<AtomicBool>,
) -> Result<StartedCapture, String> {
    capture::start(options, events, stop)
}

pub fn devices(kind: AudioSourceKind) -> Result<Vec<AudioDevice>, String> {
    devices::list(kind)
}

pub fn system_outputs() -> Result<Vec<AudioDevice>, String> {
    devices(AudioSourceKind::SystemOutput)
}

pub fn microphones() -> Result<Vec<AudioDevice>, String> {
    devices(AudioSourceKind::Microphone)
}

pub fn default_device(kind: AudioSourceKind) -> Result<Option<String>, String> {
    devices::default_id(kind)
}

pub fn default_system_output() -> Option<String> {
    default_device(AudioSourceKind::SystemOutput).ok().flatten()
}

pub fn default_microphone() -> Option<String> {
    default_device(AudioSourceKind::Microphone).ok().flatten()
}

pub fn system_capture_supported() -> bool {
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(target_os = "macos")]
    {
        macos_version_at_least(14, 6)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

pub fn microphone_capture_supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

pub fn is_supported() -> bool {
    system_capture_supported() || microphone_capture_supported()
}

pub fn capture_supported(kind: AudioSourceKind) -> bool {
    match kind {
        AudioSourceKind::SystemOutput => system_capture_supported(),
        AudioSourceKind::Microphone => microphone_capture_supported(),
    }
}

pub fn backend_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "CPAL / Core Audio"
    } else if cfg!(target_os = "windows") {
        "CPAL / WASAPI"
    } else {
        "Unsupported on this platform"
    }
}

pub fn supported_platform_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS 14.6 or later"
    } else if cfg!(target_os = "windows") {
        "Windows 8 or later"
    } else {
        "Unsupported platform"
    }
}

pub fn unsupported_capture_error(kind: AudioSourceKind) -> String {
    #[cfg(target_os = "macos")]
    if kind == AudioSourceKind::SystemOutput {
        let version = macos_product_version().unwrap_or_else(|| "unknown".to_owned());
        return format!(
            "System-output loopback capture requires macOS 14.6 or later; this Mac reports macOS {version}."
        );
    }
    format!(
        "{} capture is implemented only on macOS and Windows; this build is running on {}-{}.",
        kind.label(),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[cfg(target_os = "macos")]
fn macos_version_at_least(required_major: u32, required_minor: u32) -> bool {
    static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        let Some((major, minor)) = macos_product_version().as_deref().and_then(parse_version)
        else {
            return false;
        };
        (major, minor) >= (required_major, required_minor)
    })
}

#[cfg(target_os = "macos")]
fn macos_product_version() -> Option<String> {
    static VERSION: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    VERSION
        .get_or_init(|| {
            let output = std::process::Command::new("/usr/bin/sw_vers")
                .arg("-productVersion")
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let version = String::from_utf8(output.stdout).ok()?;
            let version = version.trim();
            if version.is_empty() {
                None
            } else {
                Some(version.to_owned())
            }
        })
        .clone()
}

#[cfg(target_os = "macos")]
fn parse_version(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor))
}
