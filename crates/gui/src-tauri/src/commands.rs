use ddc_backend::ddchi_reader::DdcHiMonitorReader;
use ddc_backend::{MonitorInfo, MonitorReader};
use kvm_core::config::{Configuration, InputSource};
use kvm_core::orchestrator::DaemonEvent;
use serde::Serialize;

use crate::{config_path, AppState};

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
pub fn save_config(config: Configuration) -> Result<(), String> {
    config.save(&config_path()).map_err(|err| err.to_string())
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
