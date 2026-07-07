use crate::PowerFallback;
use anyhow::Result;
use std::{thread, time};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{mouse_event, SendMessageW, MOUSEEVENTF_MOVE, SC_MONITORPOWER, WM_SYSCOMMAND};

const HWND_BROADCAST: HWND = 0xffff as HWND;
/// Second `WM_SYSCOMMAND`/`SC_MONITORPOWER` parameter: 2 = off, 1 = low power, -1 = on.
const MONITOR_OFF: isize = 2;
const BLANK_DURATION_MS: u64 = 500;

pub struct WindowsMonitorPower;

impl PowerFallback for WindowsMonitorPower {
    fn blank_and_restore(&self) -> Result<()> {
        unsafe {
            SendMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER as usize, MONITOR_OFF);
        }
        thread::sleep(time::Duration::from_millis(BLANK_DURATION_MS));
        // Jiggle the mouse to wake the display back up.
        unsafe {
            mouse_event(MOUSEEVENTF_MOVE, 0, 1, 0, 0);
            thread::sleep(time::Duration::from_millis(50));
            mouse_event(MOUSEEVENTF_MOVE, 0, 0xffffffff, 0, 0);
        }
        Ok(())
    }
}
