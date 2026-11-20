use cpal::traits::{DeviceTrait, HostTrait};

use super::{AudioDevice, AudioSourceKind};

#[cfg(target_os = "macos")]
use super::macos_tap;

#[cfg(target_os = "macos")]
pub(crate) type CaptureKeepalive = Option<macos_tap::OutputTap>;
#[cfg(not(target_os = "macos"))]
pub(crate) type CaptureKeepalive = Option<()>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureMode {
    Microphone,
    SystemLoopback,
}

impl CaptureMode {
    pub(crate) fn kind(self) -> AudioSourceKind {
        match self {
            Self::Microphone => AudioSourceKind::Microphone,
            Self::SystemLoopback => AudioSourceKind::SystemOutput,
        }
    }
}

pub(crate) struct CapturePlan {
    pub(crate) device: cpal::Device,
    pub(crate) device_id: String,
    pub(crate) device_name: String,
    pub(crate) config: cpal::SupportedStreamConfig,
    pub(crate) mode: CaptureMode,
    /// Core Audio objects that must outlive the stream (macOS process taps).
    pub(crate) keepalive: CaptureKeepalive,
}

pub(crate) fn resolve(
    kind: AudioSourceKind,
    device_id: Option<&str>,
) -> Result<CapturePlan, String> {
    if !super::capture_supported(kind) {
        return Err(super::unsupported_capture_error(kind));
    }
    let host = cpal::default_host();
    let device = match device_id.filter(|id| !id.is_empty()) {
        Some(id) => {
            let parsed = id.parse::<cpal::DeviceId>().map_err(|error| {
                format!(
                    "The selected {} device ID `{id}` is invalid: {error}. Select the device again.",
                    kind.label()
                )
            })?;
            if parsed.host() != host.id() {
                return Err(format!(
                    "The selected {} device `{id}` belongs to a different audio host. Select the device again.",
                    kind.label()
                ));
            }
            host.device_by_id(&parsed).ok_or_else(|| {
                format!(
                    "The selected {} device is unavailable. Reconnect it or select another device.",
                    kind.label()
                )
            })?
        }
        None => default_device(&host, kind)?,
    };

    let device_name = device_label(&device);
    let device_id = device
        .id()
        .map_err(|error| {
            format!(
                "Could not identify the selected {} device `{device_name}`: {error}",
                kind.label().to_lowercase()
            )
        })?
        .to_string();
    let (config, mode) = match kind {
        AudioSourceKind::Microphone => {
            if !device.supports_input() {
                return Err(format!(
                    "`{}` does not expose a microphone input.",
                    device_name
                ));
            }
            (
                device.default_input_config().map_err(|error| {
                    format!(
                        "Could not open microphone `{}` with its current format: {error}",
                        device_name
                    )
                })?,
                CaptureMode::Microphone,
            )
        }
        AudioSourceKind::SystemOutput => {
            if !device.supports_output() {
                return Err(format!("`{}` is not an audio output device.", device_name));
            }
            #[cfg(target_os = "macos")]
            if device.supports_input() {
                // CPAL opens the microphone of an input-capable device, so the
                // loopback has to be built here and captured as an aggregate.
                let device_uid = device.id().map(|id| id.id().to_owned()).map_err(|error| {
                    format!("Could not identify `{device_name}` for loopback: {error}")
                })?;
                let tap = macos_tap::OutputTap::create(&device_uid).map_err(|error| {
                    format!("Could not open system loopback for `{device_name}`: {error}")
                })?;
                let aggregate = find_aggregate(&host, tap.aggregate_uid()).ok_or_else(|| {
                    format!(
                        "The system loopback device for `{device_name}` did not appear in Core Audio."
                    )
                })?;
                let config = aggregate.default_input_config().map_err(|error| {
                    format!("Could not read the loopback format of `{device_name}`: {error}")
                })?;
                return Ok(CapturePlan {
                    device: aggregate,
                    device_id,
                    device_name,
                    config,
                    mode: CaptureMode::SystemLoopback,
                    keepalive: Some(tap),
                });
            }
            (
                device.default_output_config().map_err(|error| {
                    format!(
                        "Could not read the current output format of `{}`: {error}",
                        device_name
                    )
                })?,
                CaptureMode::SystemLoopback,
            )
        }
    };

    if config.channels() == 0 {
        return Err(format!(
            "`{}` reports zero audio channels and cannot be captured.",
            device_name
        ));
    }
    Ok(CapturePlan {
        device,
        device_id,
        device_name,
        config,
        mode,
        keepalive: capture_keepalive(),
    })
}

#[cfg(target_os = "macos")]
fn capture_keepalive() -> CaptureKeepalive {
    None
}

#[cfg(not(target_os = "macos"))]
fn capture_keepalive() -> CaptureKeepalive {
    None
}

#[cfg(target_os = "macos")]
fn find_aggregate(host: &cpal::Host, uid: &str) -> Option<cpal::Device> {
    let id = cpal::DeviceId::new(host.id(), uid);
    for _ in 0..20 {
        if let Some(device) = host.device_by_id(&id) {
            return Some(device);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    None
}

pub fn list(kind: AudioSourceKind) -> Result<Vec<AudioDevice>, String> {
    if !super::capture_supported(kind) {
        return Err(super::unsupported_capture_error(kind));
    }
    let host = cpal::default_host();
    let default_id = default_id_from_host(&host, kind);
    let devices = host.devices().map_err(|error| {
        format!(
            "Failed to enumerate {} devices through {}: {error}",
            kind.label(),
            super::backend_label()
        )
    })?;
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for device in devices {
        let supported_direction = match kind {
            AudioSourceKind::SystemOutput => device.supports_output(),
            AudioSourceKind::Microphone => device.supports_input(),
        };
        if !supported_direction {
            continue;
        }
        let Ok(id) = device.id().map(|id| id.to_string()) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        #[cfg(target_os = "macos")]
        if id
            .strip_prefix("coreaudio:")
            .is_some_and(|uid| uid.starts_with(macos_tap::AGGREGATE_UID_PREFIX))
        {
            // Private loopback aggregates are ours, not user-selectable devices.
            continue;
        }
        let name = device_label(&device);
        let config = match kind {
            AudioSourceKind::SystemOutput => device.default_output_config().ok(),
            AudioSourceKind::Microphone => device.default_input_config().ok(),
        };
        result.push(AudioDevice {
            id: id.clone(),
            kind,
            name,
            is_default: default_id.as_deref() == Some(id.as_str()),
            capture_supported: true,
            capture_reason: None,
            sample_rate: config
                .as_ref()
                .map(cpal::SupportedStreamConfig::sample_rate),
            channels: config.as_ref().map(cpal::SupportedStreamConfig::channels),
            sample_format: config
                .as_ref()
                .map(|config| format!("{:?}", config.sample_format())),
        });
    }
    result.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(result)
}

pub fn default_id(kind: AudioSourceKind) -> Result<Option<String>, String> {
    if !super::capture_supported(kind) {
        return Err(super::unsupported_capture_error(kind));
    }
    Ok(default_id_from_host(&cpal::default_host(), kind))
}

fn default_device(host: &cpal::Host, kind: AudioSourceKind) -> Result<cpal::Device, String> {
    let device = match kind {
        AudioSourceKind::SystemOutput => host.default_output_device(),
        AudioSourceKind::Microphone => host.default_input_device(),
    };
    device.ok_or_else(|| {
        format!(
            "No default {} device is available through {}.",
            kind.label(),
            super::backend_label()
        )
    })
}

fn default_id_from_host(host: &cpal::Host, kind: AudioSourceKind) -> Option<String> {
    default_device(host, kind)
        .ok()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string())
}

fn device_label(device: &cpal::Device) -> String {
    device
        .description()
        .map(|description| description.name().to_owned())
        .unwrap_or_else(|_| device.to_string())
}
