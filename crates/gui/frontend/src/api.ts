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
