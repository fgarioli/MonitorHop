use anyhow::{Context, Result};
use kvm_core::config::Configuration;
use kvm_core::orchestrator::{self, DaemonEvent};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use trigger::{TriggerEvent, TriggerSource};

mod commands;
mod device_database;

pub struct AppState {
    pub events: Mutex<Sender<DaemonEvent>>,
    /// Filled in by Task 9's tray setup once the menu is built; `None` until
    /// then. The MX Keys forwarder thread below updates its text whenever
    /// presence changes.
    pub mxkeys_status_item: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
    /// Holds the `DaemonEvent` receiver when no configuration existed at
    /// process startup (a genuine first run): `main()` cannot spawn the
    /// trigger/consumer threads without a `Configuration`, so rather than
    /// dropping `rx` it parks it here. `commands::save_config` takes it out
    /// and starts the pipeline the first time the setup wizard writes a
    /// config. Left `None` once the pipeline has been started, whether that
    /// happened at startup or from `save_config` — a second `save_config`
    /// call (a "Reconfigure") finds this empty and knows not to spawn a
    /// second, duplicate set of threads.
    pub pending_rx: Mutex<Option<std::sync::mpsc::Receiver<DaemonEvent>>>,
    /// The tray icon built once in `.setup()`. Stored so `save_config` can
    /// refresh the quick-switch menu items in place (via `TrayIcon::set_menu`)
    /// after a first-run config write starts the pipeline, without building a
    /// second, duplicate tray icon.
    pub tray_icon: Mutex<Option<tauri::tray::TrayIcon<tauri::Wry>>>,
}

/// Resolves and creates the per-OS application-support directory
/// (`%APPDATA%\kvm-switch-gui` on Windows, `$HOME/Library/Application
/// Support/kvm-switch-gui` on macOS), returning `None` if the relevant
/// environment variable isn't set or the directory can't be created —
/// callers fall back to a CWD-relative path in that case. Shared by
/// `config_path` below and `device_database::device_database_path`, so both
/// resolve to the same directory.
pub(crate) fn app_support_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let base = std::env::var("APPDATA").ok().map(std::path::PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var("HOME")
        .ok()
        .map(|home| std::path::PathBuf::from(home).join("Library/Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base: Option<std::path::PathBuf> = None;

    let dir = base?.join("kvm-switch-gui");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Resolves the config file to a stable, OS-independent location rather than
/// the process's current working directory. This matters because
/// `tauri-plugin-autostart` launches the app at login with an unpredictable
/// CWD on both Windows (often `C:\Windows\System32`) and macOS (a
/// `LaunchAgent`'s CWD is similarly not the install directory), so a
/// CWD-relative path would silently fail to find the config on an
/// autostarted run. Falls back to the old CWD-relative behavior if
/// `app_support_dir` returns `None`, which should not happen on a real
/// Windows or macOS machine but keeps this defensive rather than panicking.
pub(crate) fn config_path() -> std::path::PathBuf {
    app_support_dir()
        .map(|dir| dir.join("kvm-switch-config.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("kvm-switch-config.json"))
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

/// Resolves the switching tool relative to the running executable's own
/// directory rather than the CWD — this matches how a real installed/
/// autostarted build is laid out (the tool ships alongside the binary,
/// modulo the bundling caveat below), and autostart launches with an
/// unpredictable CWD (see `config_path` above). Falls back to a path anchored
/// on `CARGO_MANIFEST_DIR` (baked in at compile time as this crate's own
/// source directory) if the exe-relative one doesn't exist on disk, which
/// keeps `cargo tauri dev`/local development working regardless of the
/// process's *runtime* CWD. A plain CWD-relative fallback was tried first and
/// found broken: `cargo tauri dev` runs its DevCommand with CWD set to this
/// crate's own directory (`crates/gui/src-tauri`), not the repo root, so
/// `"tools/writeValueToDisplay.exe"` resolved to a directory that doesn't
/// exist there (confirmed via manual testing — `os error 3`, path not found).
///
/// NOTE: whether a real Tauri bundle actually places `tools/
/// writeValueToDisplay.exe` next to the installed binary is a packaging
/// concern (`tauri.conf.json`'s `bundle.resources`) that this function does
/// not address — it only fixes path *resolution* logic. See the final-review
/// fix report for details; resource bundling remains a follow-up.
#[cfg(windows)]
fn default_exe_path() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("tools/writeValueToDisplay.exe");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/writeValueToDisplay.exe")
}

/// Forwards the configured switch device's hotplug events into the shared
/// `DaemonEvent` channel. Platform-specific because the underlying
/// `TriggerSource` implementation differs per OS.
#[cfg(windows)]
pub(crate) fn spawn_switch_trigger(usb_device: String, tx: Sender<DaemonEvent>) {
    use trigger::usb_hotplug::UsbHotplugTrigger;
    std::thread::spawn(move || {
        let trigger = UsbHotplugTrigger::new(&usb_device);
        for event in trigger.watch() {
            let _ = tx.send(DaemonEvent::Trigger(event));
        }
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn spawn_switch_trigger(usb_device: String, tx: Sender<DaemonEvent>) {
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
///
/// Emits `current-input-changed` with the newly-applied VCP value whenever a
/// switch actually succeeds, regardless of what triggered it (hotplug or a
/// manual button) — the main screen listens for this so its "Active" input
/// highlight stays correct after a switch it didn't itself initiate, instead
/// of only reflecting whatever `current_input` returned at mount time.
#[cfg(windows)]
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

#[cfg(target_os = "macos")]
pub(crate) fn spawn_consumer(rx: std::sync::mpsc::Receiver<DaemonEvent>, config: Configuration, app: tauri::AppHandle) {
    use ddc_backend::macos_ioavservice::MacosIoavserviceBackend;
    use power_fallback::macos_pmset::MacosPmset;
    std::thread::spawn(move || {
        let ddc_backend = MacosIoavserviceBackend;
        let power_fallback = MacosPmset;
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
#[cfg(windows)]
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

#[cfg(target_os = "macos")]
pub(crate) fn spawn_mxkeys_trigger(mxkeys_device: String, handle: tauri::AppHandle) {
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

/// Builds the tray's menu items: Open, the MX Keys status item, one
/// "Switch to <code>" item per DDC input code the configured display
/// reports (when `config` is `Some` and the display responds), and Quit.
///
/// Shared by `.setup()` (which builds the tray, and these items, once at
/// startup) and `commands::save_config` (which rebuilds just the items and
/// swaps them onto the existing tray icon via `TrayIcon::set_menu` after a
/// first-run config write starts the pipeline) so both stay in sync.
pub(crate) fn build_quick_switch_items(
    app: &tauri::AppHandle,
    config: Option<&Configuration>,
) -> tauri::Result<Vec<tauri::menu::MenuItem<tauri::Wry>>> {
    use ddc_backend::MonitorReader;
    use tauri::menu::MenuItem;

    let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let status_i = MenuItem::with_id(app, "mxkeys-status", "MX Keys: unknown", false, None::<&str>)?;

    // `Menu::with_items` wants `&[&dyn IsMenuItem<Wry>]`; keep the concrete
    // `MenuItem<Wry>`s owned here and build trait-object refs at the call
    // site once the full set is known.
    let mut items: Vec<MenuItem<tauri::Wry>> = vec![open_i, status_i];

    if let Some(config) = config {
        if let Ok(codes) = ddc_backend::ddchi_reader::DdcHiMonitorReader.input_codes(config.display_index()) {
            for code in codes {
                let id = format!("switch:{code:#04x}");
                let label = format!("Switch to {code:#04x}");
                items.push(MenuItem::with_id(app, id, label, true, None::<&str>)?);
            }
        }
    }

    items.push(MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?);
    Ok(items)
}

fn main() {
    init_logging().expect("failed to initialize logging");

    let config = Configuration::load(&config_path()).ok();

    let (tx, rx) = std::sync::mpsc::channel::<DaemonEvent>();

    // When a config already exists at startup, start the pipeline right
    // away, same as before. When it doesn't (a genuine first run), `rx`
    // must NOT just be dropped here — that would silently discard the
    // receive half of the channel, so any later `switch_input`/tray/trigger
    // send would go into a channel nobody reads. Instead, park it in
    // `AppState.pending_rx` so `commands::save_config` can start the
    // pipeline once the setup wizard writes a config for the first time.
    //
    // `spawn_consumer` now needs a `tauri::AppHandle` (to emit
    // `current-input-changed`), which doesn't exist yet at this point in
    // `main()` — so unlike `spawn_switch_trigger` above, its call is
    // deferred into `.setup()` below, mirroring how `spawn_mxkeys_trigger`
    // already has to wait for the same reason.
    let mut pending_rx = None;
    let mut startup_consumer: Option<(std::sync::mpsc::Receiver<DaemonEvent>, Configuration)> = None;

    if let Some(config) = config.clone() {
        spawn_switch_trigger(config.usb_device.clone(), tx.clone());
        startup_consumer = Some((rx, config));
    } else {
        log::warn!(
            "No configuration found at {:?} yet; switching is disabled until the setup wizard runs.",
            config_path()
        );
        pending_rx = Some(rx);
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
            pending_rx: Mutex::new(pending_rx),
            tray_icon: Mutex::new(None),
        })
        .setup(move |app| {
            if let Some((rx, config)) = startup_consumer {
                let handle = app.handle().clone();
                spawn_consumer(rx, config, handle);
            }

            if let Some(config) = config.clone() {
                if let Some(mxkeys_device) = config.mxkeys_usb_device.clone() {
                    let handle = app.handle().clone();
                    spawn_mxkeys_trigger(mxkeys_device, handle);
                }
            }

            {
                use kvm_core::config::InputSource;
                use tauri::menu::{IsMenuItem, Menu};
                use tauri::tray::TrayIconBuilder;

                let handle = app.handle().clone();
                let items = build_quick_switch_items(&handle, config.as_ref())?;
                *app.state::<AppState>().mxkeys_status_item.lock().unwrap() = Some(items[1].clone());

                let item_refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
                    items.iter().map(|item| item as &dyn IsMenuItem<tauri::Wry>).collect();
                let menu = Menu::with_items(app, &item_refs)?;

                let tray = TrayIconBuilder::new()
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

                *app.state::<AppState>().tray_icon.lock().unwrap() = Some(tray);
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
            commands::current_input,
            commands::load_device_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for the `os error 3` (path not found) bug found during
    /// manual testing: the exe-relative candidate never exists in a `cargo
    /// tauri dev` session (nothing copies the tool into `target/debug/tools`),
    /// so this asserts the `CARGO_MANIFEST_DIR`-anchored fallback actually
    /// resolves to the real file in this checkout, regardless of the test
    /// runner's CWD.
    #[cfg(windows)]
    #[test]
    fn default_exe_path_fallback_resolves_to_real_file() {
        assert!(default_exe_path().exists(), "{:?} does not exist", default_exe_path());
    }

    #[cfg(windows)]
    #[test]
    fn app_support_dir_uses_appdata_on_windows() {
        let dir = app_support_dir().expect("APPDATA should be set in the test environment");
        assert!(dir.ends_with("kvm-switch-gui"));
        assert!(dir.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_support_dir_uses_library_application_support_on_macos() {
        let dir = app_support_dir().expect("HOME should be set in the test environment");
        assert!(dir.ends_with("kvm-switch-gui"));
        assert!(dir.to_string_lossy().contains("Library/Application Support"));
    }
}
