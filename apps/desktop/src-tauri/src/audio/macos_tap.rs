//! Loopback capture for macOS output devices that also expose input channels.
//!
//! CPAL 0.18 only builds its process-tap aggregate device when the selected
//! device has no input channels; otherwise it opens the device's microphone
//! instead. This module creates the same tap and private aggregate device
//! itself and hands the aggregate back to CPAL as an ordinary input device.

use std::{
    ffi::{c_void, CStr},
    mem::MaybeUninit,
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

use objc2::AnyThread;
use objc2_core_audio::{
    kAudioAggregateDeviceNameKey, kAudioAggregateDeviceTapAutoStartKey,
    kAudioAggregateDeviceTapListKey, kAudioAggregateDeviceUIDKey, kAudioEndPointDeviceIsPrivateKey,
    kAudioSubTapDriftCompensationKey, kAudioSubTapUIDKey, AudioHardwareCreateAggregateDevice,
    AudioHardwareCreateProcessTap, AudioHardwareDestroyAggregateDevice,
    AudioHardwareDestroyProcessTap, AudioObjectID, CATapDescription, CATapMuteBehavior,
};
use objc2_core_foundation::{
    kCFAllocatorDefault, kCFTypeArrayCallBacks, kCFTypeDictionaryKeyCallBacks,
    kCFTypeDictionaryValueCallBacks, CFArray, CFDictionary, CFMutableDictionary, CFRetained,
    CFString,
};
use objc2_foundation::{NSArray, NSNumber, NSString};

/// UID prefix used for the private aggregate devices this module creates.
/// Device enumeration skips them so they never show up as user-selectable.
pub(crate) const AGGREGATE_UID_PREFIX: &str = "com.sonasub.loopback.";

static AGGREGATE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A process tap plus the private aggregate device that exposes it to CPAL.
/// Both Core Audio objects are destroyed when this value is dropped.
pub(crate) struct OutputTap {
    tap_id: AudioObjectID,
    aggregate_id: AudioObjectID,
    aggregate_uid: String,
}

impl OutputTap {
    /// Taps the system mix destined for `device_uid` without muting playback.
    pub(crate) fn create(device_uid: &str) -> Result<Self, String> {
        let pid = std::process::id();
        let instance = AGGREGATE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let device_uid = NSString::from_str(device_uid);
        // An empty process list with `exclusive` means "tap every process".
        let processes = NSArray::new();
        let description = unsafe {
            CATapDescription::initWithProcesses_andDeviceUID_withStream(
                CATapDescription::alloc(),
                &processes,
                &device_uid,
                0,
            )
        };
        unsafe {
            description.setMuteBehavior(CATapMuteBehavior::Unmuted);
            description.setName(&NSString::from_str(&format!(
                "SonaSub system audio {pid}.{instance}"
            )));
            description.setPrivate(true);
            description.setExclusive(true);
        }

        let mut tap_id = MaybeUninit::<AudioObjectID>::uninit();
        let status = unsafe {
            AudioHardwareCreateProcessTap(Some(description.as_ref()), tap_id.as_mut_ptr())
        };
        if status != 0 {
            return Err(format!(
                "Failed to create the macOS system-audio tap: OSStatus {status}."
            ));
        }
        let tap_id = unsafe { tap_id.assume_init() };
        let tap_uid = unsafe { description.UUID().UUIDString() };
        let aggregate_uid = format!("{AGGREGATE_UID_PREFIX}{pid}.{instance}");
        let properties = aggregate_properties(
            &tap_uid,
            &aggregate_uid,
            &format!("SonaSub loopback {pid}.{instance}"),
        );

        let mut aggregate_id: AudioObjectID = 0;
        let status = unsafe {
            AudioHardwareCreateAggregateDevice(
                properties.as_ref(),
                NonNull::from(&mut aggregate_id),
            )
        };
        if status != 0 {
            unsafe { AudioHardwareDestroyProcessTap(tap_id) };
            return Err(format!(
                "Failed to create the macOS loopback aggregate device: OSStatus {status}."
            ));
        }

        Ok(Self {
            tap_id,
            aggregate_id,
            aggregate_uid,
        })
    }

    pub(crate) fn aggregate_uid(&self) -> &str {
        &self.aggregate_uid
    }
}

impl Drop for OutputTap {
    fn drop(&mut self) {
        unsafe {
            // Errors during drop cannot be reported usefully; the devices are
            // private and disappear with the process even if this fails.
            let _ = AudioHardwareDestroyAggregateDevice(self.aggregate_id);
            let _ = AudioHardwareDestroyProcessTap(self.tap_id);
        }
    }
}

fn aggregate_properties(
    tap_uid: &NSString,
    aggregate_uid: &str,
    name: &str,
) -> CFRetained<CFDictionary> {
    let tap_entry = unsafe {
        let entry = CFMutableDictionary::new(
            kCFAllocatorDefault,
            2,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        )
        .unwrap();
        CFMutableDictionary::set_value(
            Some(entry.as_ref()),
            &*to_cfstring(kAudioSubTapUIDKey) as *const _ as *const c_void,
            tap_uid as *const _ as *const c_void,
        );
        CFMutableDictionary::set_value(
            Some(entry.as_ref()),
            &*to_cfstring(kAudioSubTapDriftCompensationKey) as *const _ as *const c_void,
            &*NSNumber::initWithBool(NSNumber::alloc(), true) as *const _ as *const c_void,
        );
        entry
    };
    let tap_list = [tap_entry];
    let taps = unsafe {
        CFArray::new(
            kCFAllocatorDefault,
            tap_list.as_ptr() as *mut *const c_void,
            tap_list.len() as isize,
            &kCFTypeArrayCallBacks,
        )
        .unwrap()
    };

    unsafe {
        let properties = CFMutableDictionary::new(
            kCFAllocatorDefault,
            5,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        )
        .unwrap();
        CFMutableDictionary::set_value(
            Some(properties.as_ref()),
            &*to_cfstring(kAudioAggregateDeviceNameKey) as *const _ as *const c_void,
            &*CFString::from_str(name) as *const _ as *const c_void,
        );
        CFMutableDictionary::set_value(
            Some(properties.as_ref()),
            &*to_cfstring(kAudioAggregateDeviceUIDKey) as *const _ as *const c_void,
            &*CFString::from_str(aggregate_uid) as *const _ as *const c_void,
        );
        CFMutableDictionary::set_value(
            Some(properties.as_ref()),
            &*to_cfstring(kAudioAggregateDeviceTapListKey) as *const _ as *const c_void,
            &*taps as *const _ as *const c_void,
        );
        CFMutableDictionary::set_value(
            Some(properties.as_ref()),
            &*to_cfstring(kAudioAggregateDeviceTapAutoStartKey) as *const _ as *const c_void,
            &*NSNumber::initWithBool(NSNumber::alloc(), true) as *const _ as *const c_void,
        );
        CFMutableDictionary::set_value(
            Some(properties.as_ref()),
            &*to_cfstring(kAudioEndPointDeviceIsPrivateKey) as *const _ as *const c_void,
            &*NSNumber::initWithBool(NSNumber::alloc(), true) as *const _ as *const c_void,
        );
        CFRetained::cast_unchecked::<CFDictionary>(properties)
    }
}

fn to_cfstring(value: &'static CStr) -> CFRetained<CFString> {
    unsafe {
        CFString::with_c_string(
            kCFAllocatorDefault,
            value.as_ptr(),
            0x0800_0100, /* UTF-8 */
        )
    }
    .unwrap()
}
