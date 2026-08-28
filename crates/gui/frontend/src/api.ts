import { invoke } from "@tauri-apps/api/core";

export interface MonitorInfo {
  display_index: number;
  id: string;
  model_name: string | null;
}

export interface Configuration {
  usb_device: string;
  mxkeys_usb_device: string | null;
  on_usb_connect: string | null;
  on_usb_disconnect: string | null;
  on_usb_connect_source_addr: number | null;
  on_usb_connect_vcp_code: number | null;
  display_index: number | null;
}

export const listUsbDevices = () => invoke<string[]>("list_usb_devices");
export const listMonitors = () => invoke<MonitorInfo[]>("list_monitors");
export const listInputs = (displayIndex: number) => invoke<number[]>("list_inputs", { displayIndex });
export const saveConfig = (config: Configuration) => invoke<void>("save_config", { config });
export const loadConfig = () => invoke<Configuration | null>("load_config");
export const switchInput = (inputValue: number) => invoke<void>("switch_input", { inputValue });
export const currentInput = (displayIndex: number) => invoke<number>("current_input", { displayIndex });

/** Loads the runtime "known USB devices" lookup (device-database.json),
 * normalizing all keys to lowercase so `usbDeviceLabel`'s lookups don't
 * need to worry about casing a human might use when hand-editing the
 * file. */
export const loadDeviceDatabase = () =>
  invoke<string>("load_device_database").then((raw) => {
    const parsed = JSON.parse(raw) as Record<string, string>;
    return Object.fromEntries(Object.entries(parsed).map(([key, value]) => [key.toLowerCase(), value]));
  });

/** Downloads and installs the pending update, then restarts into it. Never
 * resolves on success — the process is replaced — so callers only need to
 * handle rejection. */
export const installUpdate = () => invoke<void>("install_update");
