use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    HostGainedFocus,
    HostLostFocus,
}

pub trait TriggerSource {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent>;
}

/// Lists currently-connected USB devices as `"vvvv:pppp"` VID:PID strings, for
/// the GUI wizard's plug-and-pick device selection (Task 8's `list_usb_devices`
/// Tauri command). Reuses the exact same enumeration `UsbHotplugTrigger` polls
/// internally.
#[cfg(windows)]
pub fn list_usb_devices() -> anyhow::Result<Vec<String>> {
    Ok(usb_hotplug::read_device_list()?.into_iter().collect())
}

/// Lists currently-connected USB devices as `"vvvv:pppp"` VID:PID strings, for
/// the GUI wizard's plug-and-pick device selection (Task 8's `list_usb_devices`
/// Tauri command). macOS has no `usb_hotplug` module to borrow
/// `read_device_list` from, so this enumerates via `rusb::devices()` directly.
#[cfg(target_os = "macos")]
pub fn list_usb_devices() -> anyhow::Result<Vec<String>> {
    Ok(rusb::devices()?
        .iter()
        .filter_map(|device| {
            device
                .device_descriptor()
                .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
                .ok()
        })
        .collect())
}

// TODO(v2): bluetooth_hid.rs — native Bluetooth HID watchers per OS.
// TODO(v2): hidpp_receiver.rs — hidapi + HID++ 1.0/2.0 parsing, notification
// 0x41 + feature 0x1814 "Change Host" (see DECISIONS.md #6, #8).

#[cfg(windows)]
pub mod usb_hotplug;
#[cfg(target_os = "macos")]
pub mod macos_hotplug;
