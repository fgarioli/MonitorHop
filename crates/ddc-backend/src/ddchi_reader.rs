//! Read-only monitor/capability discovery via the published `ddc-hi` crate.
//! Works on both Windows and macOS (unlike the write path, which needs
//! OS-specific backends). `ddc-hi` hardcodes DDC/CI source address `0x51`,
//! which is fine for reads — the source-addr override quirk documented in
//! DECISIONS.md #4 only affects writes on this monitor.

use crate::capabilities::parse_input_codes;
use crate::{MonitorInfo, MonitorReader};
use anyhow::{anyhow, Result};
use ddc_hi::{Ddc, Display};

pub struct DdcHiMonitorReader;

impl MonitorReader for DdcHiMonitorReader {
    fn enumerate(&self) -> Result<Vec<MonitorInfo>> {
        Ok(Display::enumerate()
            .into_iter()
            .enumerate()
            .map(|(index, display)| MonitorInfo {
                display_index: index as u32,
                id: display.info.id.clone(),
                model_name: display.info.model_name.clone(),
            })
            .collect())
    }

    /// **Documented risk (grilled and accepted):** this index is `ddc-hi`'s
    /// own enumeration order, which is not guaranteed to match the NVAPI
    /// ordinal `NvapiBackend::set_vcp`'s `monitor_index` expects on a
    /// multi-GPU machine. No cross-verification step was built for this —
    /// the first real run against hardware (`MANUAL_TEST_GUI.md`) is the
    /// check. If they disagree, `Configuration::display_index` needs a
    /// manual override (already supported — it's a plain optional field).
    fn input_codes(&self, display_index: u32) -> Result<Vec<u8>> {
        let mut displays = Display::enumerate();
        let display = displays
            .get_mut(display_index as usize)
            .ok_or_else(|| anyhow!("no display at index {}", display_index))?;
        let raw = display
            .handle
            .capabilities_string()
            .map_err(|err| anyhow!("failed to read capabilities for display {}: {:?}", display_index, err))?;
        Ok(parse_input_codes(&String::from_utf8_lossy(&raw)))
    }

    /// Reuses the same `Ddc` trait `enumerate()` already brings into scope
    /// (see the `use` at the top of this file) — `get_vcp_feature` returns a
    /// `mccs::Value` whose `sl` field is the low byte of the current value,
    /// which is all VCP 0x60's single-byte input codes need (mirrors how
    /// `input_codes` above already treats these codes as plain `u8`s).
    fn current_input(&self, display_index: u32) -> Result<u8> {
        const INPUT_SELECT: u8 = 0x60;
        let mut displays = Display::enumerate();
        let display = displays
            .get_mut(display_index as usize)
            .ok_or_else(|| anyhow!("no display at index {}", display_index))?;
        let value = display
            .handle
            .get_vcp_feature(INPUT_SELECT)
            .map_err(|err| anyhow!("failed to read current input for display {}: {:?}", display_index, err))?;
        Ok(value.sl)
    }
}
