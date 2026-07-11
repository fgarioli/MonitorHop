use anyhow::{Context, Result};
use ddc_backend::windows_nvapi::NvapiBackend;
use kvm_core::config::Configuration;
use kvm_core::orchestrator::{self, DaemonEvent};
use power_fallback::windows_monitorpower::WindowsMonitorPower;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use trigger::usb_hotplug::UsbHotplugTrigger;
use trigger::{TriggerEvent, TriggerSource};

pub struct AppState {
    pub events: Mutex<Sender<DaemonEvent>>,
    /// Filled in by Task 9's tray setup once the menu is built; `None` until
    /// then. The MX Keys forwarder thread below updates its text whenever
    /// presence changes.
    pub mxkeys_status_item: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
}

fn config_path() -> std::path::PathBuf {
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

fn default_exe_path() -> std::path::PathBuf {
    std::path::PathBuf::from("tools/writeValueToDisplay.exe")
}

fn main() {
    init_logging().expect("failed to initialize logging");

    let config = Configuration::load(&config_path()).ok();

    let (tx, rx) = std::sync::mpsc::channel::<DaemonEvent>();

    if let Some(config) = config.clone() {
        // Forward the switch device's hotplug events into the shared channel.
        let switch_tx = tx.clone();
        let switch_device = config.usb_device.clone();
        std::thread::spawn(move || {
            let trigger = UsbHotplugTrigger::new(&switch_device);
            for event in trigger.watch() {
                let _ = switch_tx.send(DaemonEvent::Trigger(event));
            }
        });

        // The single consumer: the only thing that ever calls into the DDC
        // write path. Runs for the life of the process.
        std::thread::spawn(move || {
            let ddc_backend = NvapiBackend::new(default_exe_path());
            let power_fallback = WindowsMonitorPower;
            orchestrator::run(rx, &config, &ddc_backend, &power_fallback);
        });
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
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
