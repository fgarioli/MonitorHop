use crate::DdcBackend;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Command;

/// Validated override for the LG 34GL750's I2C source address — see
/// DECISIONS.md #4. Windows' standard DDC API hardcodes 0x51 and does not
/// expose an override; this is only reachable via NVAPI raw I2C access.
const DEFAULT_SOURCE_ADDR: u8 = 0x50;

pub struct NvapiBackend {
    exe_path: PathBuf,
}

impl NvapiBackend {
    pub fn new(exe_path: PathBuf) -> Self {
        Self { exe_path }
    }
}

/// Builds the exact argument order `writeValueToDisplay.exe` expects:
/// `[display_index] [input_value] [command_code] [register_address]`
/// (verified by running the exe with no arguments — note this is
/// value-then-code, not code-then-value).
fn build_args(monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> [String; 4] {
    let addr = source_addr.unwrap_or(DEFAULT_SOURCE_ADDR);
    [
        monitor_index.to_string(),
        format!("0x{value:02X}"),
        format!("0x{code:02X}"),
        format!("0x{addr:02X}"),
    ]
}

impl DdcBackend for NvapiBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        let _guard = crate::ddc_io_lock();
        let args = build_args(monitor_index, code, value, source_addr);
        log::debug!("Running {:?} {:?}", self.exe_path, args);
        let status = Command::new(&self.exe_path).args(&args).status()?;
        if !status.success() {
            return Err(anyhow!(
                "writeValueToDisplay.exe exited with {:?} (args: {:?})",
                status.code(),
                args
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_uses_validated_default_source_addr() {
        let args = build_args(0, 0x60, 0x11, None);
        assert_eq!(args, ["0", "0x11", "0x60", "0x50"]);
    }

    #[test]
    fn build_args_honors_explicit_source_addr_override() {
        let args = build_args(0, 0x60, 0x11, Some(0x51));
        assert_eq!(args, ["0", "0x11", "0x60", "0x51"]);
    }
}
