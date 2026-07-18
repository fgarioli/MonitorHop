use crate::DdcBackend;
use anyhow::{anyhow, Result};
use ddc_hi::{Ddc, Display};

pub struct MacosIoavserviceBackend;

impl DdcBackend for MacosIoavserviceBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        let _guard = crate::ddc_io_lock();
        if source_addr.is_some() {
            log::warn!(
                "source_addr override ({:?}) requested but unsupported on macOS (ddc-hi hardcodes 0x51); ignoring",
                source_addr
            );
        }
        crate::retry(3, std::time::Duration::from_millis(50), || {
            let mut displays = Display::enumerate();
            let display = displays
                .get_mut(monitor_index as usize)
                .ok_or_else(|| anyhow!("no display at index {}", monitor_index))?;
            display
                .handle
                .set_vcp_feature(code, value)
                .map_err(|err| anyhow!("failed to set VCP {:#04x}={:#06x}: {:?}", code, value, err))
        })
    }
}
