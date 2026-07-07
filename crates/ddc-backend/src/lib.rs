//! Backends that write VCP feature values to a monitor over DDC/CI.

use anyhow::Result;

pub trait DdcBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()>;
}

// TODO(macos): macos_ioavservice.rs — IOAVServiceReadI2C/WriteI2C backend,
// blocked on Spike #2 (see DECISIONS.md #5, #7).
// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9).

pub mod windows_generic;
pub mod windows_nvapi;
