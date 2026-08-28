use crate::app_state::AppState;
use crate::paths::default_exe_path;
use kvm_core::config::Configuration;
use kvm_core::orchestrator::{self, DaemonEvent};
use std::sync::mpsc::Sender;
use tauri::{Emitter, Manager};
use trigger::{TriggerEvent, TriggerSource};

/// Forwards the configured switch device's hotplug events into the shared
/// `DaemonEvent` channel. Platform-specific because the underlying
/// `TriggerSource` implementation differs per OS.
pub(crate) fn spawn_switch_trigger(usb_device: String, tx: Sender<DaemonEvent>) {
    use trigger::usb_hotplug::UsbHotplugTrigger;
    std::thread::spawn(move || {
        let trigger = UsbHotplugTrigger::new(&usb_device);
        for event in trigger.watch() {
            let _ = tx.send(DaemonEvent::Trigger(event));
        }
    });
}

/// The single consumer: the only thing that ever calls into the DDC write
/// path. Runs for the life of the process. Platform-specific because the
/// `DdcBackend`/`PowerFallback` implementations differ per OS.
///
/// Emits `current-input-changed` with the newly-applied VCP value whenever a
/// switch actually succeeds, regardless of what triggered it (hotplug or a
/// manual button) — the main screen listens for this so its "Active" input
/// highlight stays correct after a switch it didn't itself initiate, instead
/// of only reflecting whatever `current_input` returned at mount time.
pub(crate) fn spawn_consumer(rx: std::sync::mpsc::Receiver<DaemonEvent>, config: Configuration, app: tauri::AppHandle) {
    use ddc_backend::windows_nvapi::NvapiBackend;
    use power_fallback::windows_monitorpower::WindowsMonitorPower;
    std::thread::spawn(move || {
        let ddc_backend = NvapiBackend::new(default_exe_path());
        let power_fallback = WindowsMonitorPower;
        let on_switched = |value: u16| {
            let _ = app.emit("current-input-changed", value);
        };
        orchestrator::run(rx, &config, &ddc_backend, &power_fallback, &on_switched);
    });
}

/// Forwards the (optional) MX Keys device's hotplug events into the tray's
/// status item and an app-wide `mxkeys-status` event, for UIs that display
/// live keyboard-presence state. Platform-specific for the same reason as
/// `spawn_switch_trigger`.
pub(crate) fn spawn_mxkeys_trigger(mxkeys_device: String, handle: tauri::AppHandle) {
    use trigger::usb_hotplug::UsbHotplugTrigger;
    std::thread::spawn(move || {
        let trigger = UsbHotplugTrigger::new(&mxkeys_device);
        for event in trigger.watch() {
            let connected = matches!(event, TriggerEvent::HostGainedFocus);
            let _ = handle.emit("mxkeys-status", connected);
            let state = handle.state::<AppState>();
            let guard = state.mxkeys_status_item.lock().unwrap();
            if let Some(item) = guard.as_ref() {
                let text = if connected { "MX Keys: connected" } else { "MX Keys: not connected" };
                let _ = item.set_text(text);
            }
        }
    });
}
