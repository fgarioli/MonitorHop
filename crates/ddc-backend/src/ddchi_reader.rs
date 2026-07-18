//! Read-only monitor/capability discovery via the published `ddc-hi` crate.
//! Works on both Windows and macOS (unlike the write path, which needs
//! OS-specific backends).
//!
//! **Correction to an earlier assumption:** this file used to claim `ddc-hi`
//! hardcoding DDC/CI source address `0x51` was "fine for reads" — a real
//! manual-test session found reads via that path (`Backend::WinApi`) fail
//! almost every time on this monitor, the same underlying problem
//! DECISIONS.md #4 already diagnosed for writes. `ddc-hi` (default features)
//! also ships its own NVAPI-backed backend (`Backend::Nvapi`) that sets I2C
//! source address `0x50` internally (see its `Display::enumerate()`) — the
//! same override the write path needs. `select_display` below prefers that
//! entry for reads when one exists, falling back to `display_index`-based
//! `WinApi` selection (previous behavior) otherwise.

use crate::capabilities::parse_input_codes;
use crate::{MonitorInfo, MonitorReader};
use anyhow::{anyhow, Result};
use ddc_hi::{Backend, Ddc, Display};

/// Prefers an NVAPI-backed display (source address `0x50`, matches the
/// write path) over the generic-Windows-API one `display_index` would
/// otherwise select — see the module doc comment for why. In the current
/// single-monitor setup this is unambiguous; a multi-monitor NVIDIA setup
/// would need `display_index` threaded into the NVAPI-preference search too,
/// which isn't built since there's no hardware to validate it against yet.
fn select_display(displays: &mut [Display], display_index: u32) -> Result<&mut Display> {
    if let Some(pos) = displays.iter().position(|d| d.info.backend == Backend::Nvapi) {
        return Ok(&mut displays[pos]);
    }
    displays
        .get_mut(display_index as usize)
        .ok_or_else(|| anyhow!("no display at index {}", display_index))
}

pub struct DdcHiMonitorReader;

impl MonitorReader for DdcHiMonitorReader {
    fn enumerate(&self) -> Result<Vec<MonitorInfo>> {
        let _guard = crate::ddc_io_lock();
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
        let _guard = crate::ddc_io_lock();
        crate::retry(3, std::time::Duration::from_millis(50), || {
            let mut displays = Display::enumerate();
            let display = select_display(&mut displays, display_index)?;
            let raw = display
                .handle
                .capabilities_string()
                .map_err(|err| anyhow!("failed to read capabilities for display {}: {:?}", display_index, err))?;
            Ok(parse_input_codes(&String::from_utf8_lossy(&raw)))
        })
    }

    /// Reuses the same `Ddc` trait `enumerate()` already brings into scope
    /// (see the `use` at the top of this file) — `get_vcp_feature` returns a
    /// `mccs::Value` whose `sl` field is the low byte of the current value,
    /// which is all VCP 0x60's single-byte input codes need (mirrors how
    /// `input_codes` above already treats these codes as plain `u8`s).
    fn current_input(&self, display_index: u32) -> Result<u8> {
        let _guard = crate::ddc_io_lock();
        const INPUT_SELECT: u8 = 0x60;
        crate::retry(3, std::time::Duration::from_millis(50), || {
            let mut displays = Display::enumerate();
            let display = select_display(&mut displays, display_index)?;
            let value = display
                .handle
                .get_vcp_feature(INPUT_SELECT)
                .map_err(|err| anyhow!("failed to read current input for display {}: {:?}", display_index, err))?;
            Ok(value.sl)
        })
    }
}
