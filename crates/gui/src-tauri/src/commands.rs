use ddc_backend::ddchi_reader::DdcHiMonitorReader;
use ddc_backend::{MonitorInfo, MonitorReader};
use kvm_core::config::{Configuration, InputSource};
use kvm_core::orchestrator::DaemonEvent;
use serde::Serialize;

use crate::{
    build_quick_switch_items, config_path, spawn_consumer, spawn_mxkeys_trigger, spawn_switch_trigger, AppState,
};

/// `MonitorInfo` isn't `Serialize` (it lives in `ddc-backend`, which has no
/// reason to depend on `serde`) — this DTO is the frontend-facing shape.
#[derive(Serialize)]
pub struct MonitorInfoDto {
    pub display_index: u32,
    pub id: String,
    pub model_name: Option<String>,
}

impl From<MonitorInfo> for MonitorInfoDto {
    fn from(info: MonitorInfo) -> Self {
        Self {
            display_index: info.display_index,
            id: info.id,
            model_name: info.model_name,
        }
    }
}

#[tauri::command]
pub fn list_usb_devices() -> Result<Vec<String>, String> {
    trigger::list_usb_devices().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_monitors() -> Result<Vec<MonitorInfoDto>, String> {
    DdcHiMonitorReader
        .enumerate()
        .map(|monitors| monitors.into_iter().map(MonitorInfoDto::from).collect())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_inputs(display_index: u32) -> Result<Vec<u8>, String> {
    DdcHiMonitorReader.input_codes(display_index).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn current_input(display_index: u32) -> Result<u8, String> {
    DdcHiMonitorReader.current_input(display_index).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn load_device_database() -> Result<String, String> {
    crate::device_database::load_or_seed(&crate::device_database::device_database_path())
        .map_err(|err| err.to_string())
}

/// Writes `config` to disk, then — if this is a genuine first run (no
/// config existed when the process started, so `main()` parked the
/// `DaemonEvent` receiver in `AppState.pending_rx` instead of starting the
/// pipeline; see Bug 1 of the 2026-07-10 cross-platform-gui final-review
/// fix) — starts the switch-trigger, mxkeys-trigger (if configured), and
/// consumer threads using the config that was just saved, via the same
/// `spawn_switch_trigger`/`spawn_mxkeys_trigger`/`spawn_consumer` helpers
/// `main()` uses for the "config existed at startup" path. It also
/// refreshes the tray's quick-switch menu items in place so they reflect
/// the new config without requiring a restart.
///
/// If the pipeline was ALREADY running (this call is a "Reconfigure" — the
/// wizard was re-run in a session that started with a config already on
/// disk), the file is still written successfully so the new config is
/// picked up on the next launch, but no second set of threads is spawned:
/// the already-running consumer thread took its `Configuration` by value at
/// spawn time, so hot-swapping it isn't possible without a larger
/// refactor, and spawning a second consumer/trigger set risks double
/// switching. This is a deliberate scope boundary, not an oversight — the
/// caller must restart the app for a reconfigure to fully take effect
/// (tray items and the running consumer both keep using the pre-reconfigure
/// state until then).
#[tauri::command]
pub fn save_config(config: Configuration, app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    config.save(&config_path()).map_err(|err| err.to_string())?;

    let rx = state.pending_rx.lock().map_err(|err| err.to_string())?.take();

    let Some(rx) = rx else {
        log::info!(
            "Configuration saved while the switch pipeline was already running (reconfigure); \
             a restart is required for the new configuration to take effect."
        );
        return Ok(());
    };

    let tx = state.events.lock().map_err(|err| err.to_string())?.clone();
    spawn_switch_trigger(config.usb_device.clone(), tx);

    if let Some(mxkeys_device) = config.mxkeys_usb_device.clone() {
        spawn_mxkeys_trigger(mxkeys_device, app.clone());
    }

    spawn_consumer(rx, config.clone(), app.clone());

    if let Err(err) = refresh_tray_quick_switch_items(&app, &state, &config) {
        log::warn!("Failed to refresh tray quick-switch items after first-run save: {err}");
    }

    Ok(())
}

/// Rebuilds the tray's menu items against `config` and swaps them onto the
/// tray icon `.setup()` already built, via `TrayIcon::set_menu`, rather than
/// building a second, duplicate tray icon.
fn refresh_tray_quick_switch_items(
    app: &tauri::AppHandle,
    state: &tauri::State<AppState>,
    config: &Configuration,
) -> tauri::Result<()> {
    let items = build_quick_switch_items(app, Some(config))?;
    *state.mxkeys_status_item.lock().unwrap() = Some(items[1].clone());

    let item_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>).collect();
    let menu = tauri::menu::Menu::with_items(app, &item_refs)?;

    if let Some(tray) = state.tray_icon.lock().unwrap().as_ref() {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

#[tauri::command]
pub fn load_config() -> Result<Option<Configuration>, String> {
    match Configuration::load(&config_path()) {
        Ok(config) => Ok(Some(config)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub fn switch_input(input_value: u16, state: tauri::State<AppState>) -> Result<(), String> {
    let events = state.events.lock().map_err(|err| err.to_string())?;
    events
        .send(DaemonEvent::ManualSwitch(InputSource::Raw(input_value)))
        .map_err(|err| err.to_string())
}
