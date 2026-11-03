//! CoreAudio: the default output device's name and change notifications
//! (ADR-037). mpv itself follows the default device when `audio-device` is
//! `auto`; this only tells the UI which device that is.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_core_audio::{
    AudioObjectAddPropertyListener, AudioObjectGetPropertyData, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectRemovePropertyListener,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal,
    kAudioObjectSystemObject,
};
use objc2_core_foundation::{CFRetained, CFString};

const WATCHED: [u32; 2] = [
    kAudioHardwarePropertyDefaultOutputDevice,
    kAudioHardwarePropertyDevices,
];

fn address(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn system() -> AudioObjectID {
    // `kAudioObjectSystemObject` is 1, declared as c_int in the bindings.
    AudioObjectID::try_from(kAudioObjectSystemObject).unwrap_or(1)
}

/// The system default output device's name, if CoreAudio reports one.
pub fn default_output_device() -> Option<String> {
    let mut device: AudioObjectID = 0;
    let mut size = u32::try_from(size_of::<AudioObjectID>()).ok()?;
    let mut addr = address(kAudioHardwarePropertyDefaultOutputDevice);
    // SAFETY: valid address, size and output buffer for an AudioObjectID.
    let status = unsafe {
        AudioObjectGetPropertyData(
            system(),
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut device).cast(),
        )
    };
    if status != 0 || device == 0 {
        return None;
    }
    let mut name: *const CFString = std::ptr::null();
    let mut size = u32::try_from(size_of::<*const CFString>()).ok()?;
    let mut addr = address(kAudioObjectPropertyName);
    // SAFETY: the property is a CFStringRef returned retained (+1).
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut name).cast(),
        )
    };
    if status != 0 {
        return None;
    }
    let name = NonNull::new(name.cast_mut())?;
    // SAFETY: we own the +1 reference returned by CoreAudio.
    let name = unsafe { CFRetained::from_raw(name) };
    Some(name.to_string())
}

type Callback = Box<dyn Fn() + Send + Sync>;

/// Registered CoreAudio listener; unregisters on drop.
pub struct DeviceWatch {
    callback: *mut Callback,
}

// SAFETY: the callback is `Send + Sync`; the pointer is only freed in Drop
// after the listeners are removed.
unsafe impl Send for DeviceWatch {}

impl std::fmt::Debug for DeviceWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceWatch")
    }
}

unsafe extern "C-unwind" fn listener(
    _object: AudioObjectID,
    _count: u32,
    _addresses: NonNull<AudioObjectPropertyAddress>,
    data: *mut c_void,
) -> i32 {
    // SAFETY: `data` is the `*mut Callback` registered in `watch`, alive
    // until the listeners are removed.
    if let Some(cb) = unsafe { data.cast::<Callback>().as_ref() } {
        cb();
    }
    0
}

/// Call `on_change` (on a CoreAudio thread) when the default output device
/// or the device list changes; `None` if CoreAudio refuses the listener.
pub fn watch(on_change: impl Fn() + Send + Sync + 'static) -> Option<DeviceWatch> {
    let callback: *mut Callback = Box::into_raw(Box::new(Box::new(on_change)));
    let watch = DeviceWatch { callback };
    for selector in WATCHED {
        let mut addr = address(selector);
        // SAFETY: `listener` matches the C signature; `callback` outlives
        // the registration (removed in Drop before freeing).
        let status = unsafe {
            AudioObjectAddPropertyListener(
                system(),
                NonNull::from(&mut addr),
                Some(listener),
                callback.cast(),
            )
        };
        if status != 0 {
            tracing::warn!(status, selector, "cannot watch audio devices");
            return None; // Drop removes what was added.
        }
    }
    Some(watch)
}

impl Drop for DeviceWatch {
    fn drop(&mut self) {
        for selector in WATCHED {
            let mut addr = address(selector);
            // SAFETY: same arguments as the registration.
            unsafe {
                AudioObjectRemovePropertyListener(
                    system(),
                    NonNull::from(&mut addr),
                    Some(listener),
                    self.callback.cast(),
                );
            }
        }
        // SAFETY: allocated in `watch`; no listener refers to it any more.
        drop(unsafe { Box::from_raw(self.callback) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_device_query_and_watch_do_not_fail() {
        // Machines without audio devices return None; never panics.
        let _ = default_output_device();
        let watch = watch(|| {});
        drop(watch);
    }

    /// Needs an audio output (run on a Mac): `cargo test -p ttnr-platform -- --ignored`.
    #[test]
    #[ignore = "needs an audio output device"]
    fn default_device_has_a_name() {
        let name = default_output_device().expect("a default output device");
        assert!(!name.is_empty());
    }
}
