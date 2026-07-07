use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    HostGainedFocus,
    HostLostFocus,
}

pub trait TriggerSource {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent>;
}

// TODO(v2): bluetooth_hid.rs — native Bluetooth HID watchers per OS.
// TODO(v2): hidpp_receiver.rs — hidapi + HID++ 1.0/2.0 parsing, notification
// 0x41 + feature 0x1814 "Change Host" (see DECISIONS.md #6, #8).

pub mod usb_hotplug;
