use crate::DdcBackend;
use anyhow::Result;

/// Fallback for non-NVIDIA GPUs (dxva2 / `SetVCPFeature`). Whether AMD's ADL
/// exposes an equivalent I2C source-address override is unconfirmed — see
/// DECISIONS.md #4 and #10. Not implemented in this milestone.
pub struct GenericDdcBackend;

impl DdcBackend for GenericDdcBackend {
    fn set_vcp(&self, _monitor_index: u32, _code: u8, _value: u16, _source_addr: Option<u8>) -> Result<()> {
        todo!("windows_generic backend: AMD/ADL source-addr override not yet implemented, see DECISIONS.md #10")
    }
}
