use anyhow::{anyhow, Result};
use crate::TriggerEvent;
use rusb::UsbContext;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::{self, Sender};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::ntdef::LPCWSTR;
use winapi::shared::windef::{HBRUSH, HCURSOR, HICON, HWND};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW, PostQuitMessage,
    RegisterClassW, SetWindowLongPtrW, TranslateMessage, GWLP_USERDATA, MSG, WM_CREATE, WM_DESTROY,
    WM_DEVICECHANGE, WNDCLASSW,
};

fn diff_to_events(current: &HashSet<String>, new: &HashSet<String>, watched: &str) -> Vec<TriggerEvent> {
    let mut events = Vec::new();
    if new.contains(watched) && !current.contains(watched) {
        events.push(TriggerEvent::HostGainedFocus);
    }
    if current.contains(watched) && !new.contains(watched) {
        events.push(TriggerEvent::HostLostFocus);
    }
    events
}

pub struct UsbHotplugTrigger {
    usb_device: String,
}

impl UsbHotplugTrigger {
    pub fn new(usb_device: &str) -> Self {
        Self {
            usb_device: usb_device.to_lowercase(),
        }
    }
}

impl crate::TriggerSource for UsbHotplugTrigger {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent> {
        let (tx, rx) = mpsc::channel();
        let usb_device = self.usb_device.clone();
        std::thread::spawn(move || {
            if let Err(err) = run_message_loop(usb_device, tx) {
                log::error!("USB hotplug detection failed: {:?}", err);
            }
        });
        rx
    }
}

fn device_id<T: UsbContext>(device: &rusb::Device<T>) -> Option<String> {
    device
        .device_descriptor()
        .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
        .ok()
}

pub fn read_device_list() -> Result<HashSet<String>> {
    Ok(rusb::devices()?.iter().filter_map(|device| device_id(&device)).collect())
}

struct WindowState {
    usb_device: String,
    sender: Sender<TriggerEvent>,
    current_devices: HashSet<String>,
}

impl WindowState {
    fn handle_hotplug_event(&mut self) {
        let new_devices = match read_device_list() {
            Ok(devices) => devices,
            Err(err) => {
                log::error!("Cannot get list of USB devices: {:?}", err);
                return;
            }
        };
        for event in diff_to_events(&self.current_devices, &new_devices, &self.usb_device) {
            log::debug!("USB device state changed, emitting {:?} for device {}", event, self.usb_device);
            let _ = self.sender.send(event);
        }
        self.current_devices = new_devices;
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = lparam as *mut winapi::um::winuser::CREATESTRUCTW;
            let state_ptr = create_struct.as_ref().unwrap().lpCreateParams;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
        }
        WM_DESTROY => PostQuitMessage(0),
        WM_DEVICECHANGE => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if state_ptr != 0 {
                let state: &mut WindowState = &mut *(state_ptr as *mut WindowState);
                state.handle_hotplug_event();
            }
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

fn run_message_loop(usb_device: String, sender: Sender<TriggerEvent>) -> Result<()> {
    let mut state = Box::new(WindowState {
        current_devices: read_device_list().unwrap_or_default(),
        usb_device,
        sender,
    });

    let class_name: Vec<u16> = OsStr::new("KvmSwitchPnPDetectWindowClass").encode_wide().chain(once(0)).collect();
    let window_name: Vec<u16> = OsStr::new("KvmSwitchPnPDetectWindow").encode_wide().chain(once(0)).collect();
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: 0 as HICON,
        hCursor: 0 as HCURSOR,
        hbrBackground: 0 as HBRUSH,
        lpszMenuName: 0 as LPCWSTR,
        lpszClassName: class_name.as_ptr(),
    };

    let hwnd = unsafe {
        if RegisterClassW(&wc) == 0 {
            return Err(anyhow!("failed to register window class"));
        }
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            state.as_mut() as *mut WindowState as *mut _,
        )
    };
    if hwnd.is_null() {
        return Err(anyhow!("failed to create window"));
    }

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        loop {
            // `WM_QUIT` is a thread message posted with hwnd=NULL (see `PostQuitMessage`
            // above); passing our specific `hwnd` here would filter it out and make the
            // `WM_DESTROY` shutdown path unreachable.
            let val = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if val == 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gained_focus_when_watched_device_appears() {
        let current: HashSet<String> = HashSet::new();
        let new: HashSet<String> = ["17e9:6000".to_string()].into_iter().collect();
        assert_eq!(diff_to_events(&current, &new, "17e9:6000"), vec![TriggerEvent::HostGainedFocus]);
    }

    #[test]
    fn lost_focus_when_watched_device_disappears() {
        let current: HashSet<String> = ["17e9:6000".to_string()].into_iter().collect();
        let new: HashSet<String> = HashSet::new();
        assert_eq!(diff_to_events(&current, &new, "17e9:6000"), vec![TriggerEvent::HostLostFocus]);
    }

    #[test]
    fn no_event_for_unrelated_device_changes() {
        let current: HashSet<String> = HashSet::new();
        let new: HashSet<String> = ["aaaa:bbbb".to_string()].into_iter().collect();
        assert!(diff_to_events(&current, &new, "17e9:6000").is_empty());
    }
}
