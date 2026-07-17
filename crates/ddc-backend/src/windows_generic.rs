use crate::DdcBackend;
use anyhow::Result;

/// Fallback for non-NVIDIA GPUs (dxva2 / `SetVCPFeature`). Whether AMD's ADL
/// exposes an equivalent I2C source-address override is unconfirmed — see
/// DECISIONS.md #4 and #10. Not implemented in this milestone.
pub struct GenericDdcBackend;

impl DdcBackend for GenericDdcBackend {
    fn set_vcp(&self, _monitor_index: u32, _code: u8, _value: u16, _source_addr: Option<u8>) -> Result<()> {
        Err(anyhow::anyhow!(
            "GenericDdcBackend (AMD/ADL fallback) is not implemented yet — see DECISIONS.md #4/#10 \
             and IMPROVEMENTS.md #4. This backend is not currently selected by any code path."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_vcp_returns_an_error_instead_of_panicking() {
        let backend = GenericDdcBackend;
        let result = backend.set_vcp(0, 0x60, 0x11, None);
        assert!(result.is_err());
    }
}
