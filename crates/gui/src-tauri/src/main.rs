use anyhow::{Context, Result};
use kvm_core::config::Configuration;
use kvm_core::orchestrator::{self, DaemonEvent};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use trigger::{TriggerEvent, TriggerSource};

mod commands;

pub struct AppState {
    pub events: Mutex<Sender<DaemonEvent>>,
    /// Filled in by Task 9's tray setup once the menu is built; `None` until
    /// then. The MX Keys forwarder thread below updates its text whenever
    /// presence changes.
    pub mxkeys_status_item: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
}

pub(crate) fn config_path() -> std::path::PathBuf {
    std::path::PathBuf::from("kvm-switch-config.json")
}

fn init_logging() -> Result<()> {
    use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .context("failed to initialize logging")
}

#[cfg(windows)]
fn default_exe_path() -> std::path::PathBuf {
    std::path::PathBuf::from("tools/writeValueToDisplay.exe")
}

/// Forwards the configured switch device's hotplug events into the shared
/// `DaemonEvent` channel. Platform-specific because the underlying
/// `TriggerSource` implementation differs per OS.
#[cfg(windows)]
fn spawn_switch_trigger(usb_device: String, tx: Sender<DaemonEvent>) {
    use trigger::usb_hotplug::UsbHotplugTrigger;
    std::thread::spawn(move || {
        let trigger = UsbHotplugTrigger::new(&usb_device);
        for event in trigger.watch() {
            let _ = tx.send(DaemonEvent::Trigger(event));
        }
    });
}

#[cfg(target_os = "macos")]
fn spawn_switch_trigger(usb_device: String, tx: Sender<DaemonEvent>) {
    use trigger::macos_hotplug::MacosHotplugTrigger;
    std::thread::spawn(move || {
        let trigger = MacosHotplugTrigger::new(&usb_device);
        for event in trigger.watch() {
            let _ = tx.send(DaemonEvent::Trigger(event));
        }
    });
}

/// The single consumer: the only thing that ever calls into the DDC write
/// path. Runs for the life of the process. Platform-specific because the
/// `DdcBackend`/`PowerFallback` implementations differ per OS.
#[cfg(windows)]
fn spawn_consumer(rx: std::sync::mpsc::Receiver<DaemonEvent>, config: Configuration) {
    use ddc_backend::windows_nvapi::NvapiBackend;
    use power_fallback::windows_monitorpower::WindowsMonitorPower;
    std::thread::spawn(move || {
        let ddc_backend = NvapiBackend::new(default_exe_path());
        let power_fallback = WindowsMonitorPower;
        orchestrator::run(rx, &config, &ddc_backend, &power_fallback);
    });
}

#[cfg(target_os = "macos")]
fn spawn_consumer(rx: std::sync::mpsc::Receiver<DaemonEvent>, config: Configuration) {
    use ddc_backend::macos_ioavservice::MacosIoavserviceBackend;
    use power_fallback::macos_pmset::MacosPmset;
    std::thread::spawn(move || {
        let ddc_backend = MacosIoavserviceBackend;
        let power_fallback = MacosPmset;
        orchestrator::run(rx, &config, &ddc_backend, &power_fallback);
    });
}

/// Forwards the (optional) MX Keys device's hotplug events into the tray's
/// status item and an app-wide `mxkeys-status` event, for UIs that display
/// live keyboard-presence state. Platform-specific for the same reason as
/// `spawn_switch_trigger`.
#[cfg(windows)]
fn spawn_mxkeys_trigger(mxkeys_device: String, handle: tauri::AppHandle) {
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

#[cfg(target_os = "macos")]
fn spawn_mxkeys_trigger(mxkeys_device: String, handle: tauri::AppHandle) {
    use trigger::macos_hotplug::MacosHotplugTrigger;
    std::thread::spawn(move || {
        let trigger = MacosHotplugTrigger::new(&mxkeys_device);
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

fn main() {
    init_logging().expect("failed to initialize logging");

    let config = Configuration::load(&config_path()).ok();

    let (tx, rx) = std::sync::mpsc::channel::<DaemonEvent>();

    if let Some(config) = config.clone() {
        spawn_switch_trigger(config.usb_device.clone(), tx.clone());
        spawn_consumer(rx, config);
    } else {
        log::warn!(
            "No configuration found at {:?} yet; switching is disabled until the setup wizard runs.",
            config_path()
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState {
            events: Mutex::new(tx),
            mxkeys_status_item: Mutex::new(None),
        })
        .setup(move |app| {
            if let Some(config) = config.clone() {
                if let Some(mxkeys_device) = config.mxkeys_usb_device.clone() {
                    let handle = app.handle().clone();
                    spawn_mxkeys_trigger(mxkeys_device, handle);
                }
            }

            {
                use ddc_backend::MonitorReader;
                use kvm_core::config::InputSource;
                use tauri::menu::{IsMenuItem, Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;

                let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
                let status_i = MenuItem::with_id(app, "mxkeys-status", "MX Keys: unknown", false, None::<&str>)?;
                *app.state::<AppState>().mxkeys_status_item.lock().unwrap() = Some(status_i.clone());

                // `Menu::with_items` wants `&[&dyn IsMenuItem<Wry>]`; keep the
                // concrete `MenuItem<Wry>`s owned here and build trait-object
                // refs into `item_refs` below once the full set is known.
                let mut items: Vec<MenuItem<tauri::Wry>> = vec![open_i, status_i];

                if let Some(config) = config.clone() {
                    if let Ok(codes) = ddc_backend::ddchi_reader::DdcHiMonitorReader.input_codes(config.display_index()) {
                        for code in codes {
                            let id = format!("switch:{code:#04x}");
                            let label = format!("Switch to {code:#04x}");
                            items.push(MenuItem::with_id(app, id, label, true, None::<&str>)?);
                        }
                    }
                }

                items.push(MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?);

                let item_refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
                    items.iter().map(|item| item as &dyn IsMenuItem<tauri::Wry>).collect();
                let menu = Menu::with_items(app, &item_refs)?;

                TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, event| {
                        let id = event.id.as_ref();
                        match id {
                            "open" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.unminimize();
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "quit" => app.exit(0),
                            id if id.starts_with("switch:") => {
                                if let Ok(value) = u16::from_str_radix(id.trim_start_matches("switch:0x"), 16) {
                                    let state = app.state::<AppState>();
                                    let events = state.events.lock().unwrap();
                                    let _ = events.send(DaemonEvent::ManualSwitch(InputSource::Raw(value)));
                                }
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_usb_devices,
            commands::list_monitors,
            commands::list_inputs,
            commands::save_config,
            commands::load_config,
            commands::switch_input,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
