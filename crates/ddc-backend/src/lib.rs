//! Backends that write VCP feature values to a monitor over DDC/CI, plus a
//! separate read-only capability for enumerating monitors and their
//! supported inputs (used by the GUI's configuration wizard, never by the
//! orchestrator's write path).

use anyhow::Result;

pub trait DdcBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()>;
}

/// A monitor detected by `MonitorReader::enumerate`. `display_index` is the
/// ordinal used by `DdcBackend::set_vcp`'s `monitor_index` argument — see the
/// documented risk in this plan's Task 2 about whether `ddc-hi`'s enumeration
/// order matches the NVAPI-backed write path's own indexing.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub display_index: u32,
    pub id: String,
    pub model_name: Option<String>,
}

/// Read-only monitor/capability discovery, used only by the GUI's
/// configuration wizard. Deliberately separate from `DdcBackend`: the
/// orchestrator's write path (`DdcBackend::set_vcp`) never depends on this
/// trait, so nothing about the already-tested orchestrator changes here.
pub trait MonitorReader {
    fn enumerate(&self) -> Result<Vec<MonitorInfo>>;
    fn input_codes(&self, display_index: u32) -> Result<Vec<u8>>;
}

pub mod capabilities;
pub use capabilities::parse_input_codes;

// TODO(macos): macos_ioavservice.rs — IOAVServiceReadI2C/WriteI2C backend,
// blocked on Spike #2 (see DECISIONS.md #5, #7). Implemented design/type-check
// only in this plan's Task 9.
// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9). Out of scope.

#[cfg(any(windows, target_os = "macos"))]
pub mod ddchi_reader;

#[cfg(windows)]
pub mod windows_generic;
#[cfg(windows)]
pub mod windows_nvapi;
