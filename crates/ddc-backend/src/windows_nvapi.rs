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

fn describe_failure(args: &[String; 4], output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "writeValueToDisplay.exe exited with {:?} (args: {:?}). stdout: {:?}, stderr: {:?}. \
         This tool depends on an NVIDIA GPU/driver (NVAPI) being available — if this machine has \
         switched to an AMD/integrated-only graphics mode (e.g. a laptop's \"Eco\"/iGPU-only \
         setting), switching back to the NVIDIA GPU is currently the only known fix (see \
         DECISIONS.md #4/#10, IMPROVEMENTS.md #4).",
        output.status.code(),
        args,
        stdout.trim(),
        stderr.trim(),
    )
}

impl DdcBackend for NvapiBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        let _guard = crate::ddc_io_lock();
        let args = build_args(monitor_index, code, value, source_addr);
        log::debug!("Running {:?} {:?}", self.exe_path, args);
        let output = Command::new(&self.exe_path).args(&args).output()?;
        if !output.status.success() {
            return Err(anyhow!(describe_failure(&args, &output)));
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

    #[test]
    fn describe_failure_includes_captured_output_and_the_nvidia_hint() {
        let output = std::process::Output {
            status: fake_exit_status(1),
            stdout: b"".to_vec(),
            stderr: b"NVAPI initialization failed".to_vec(),
        };
        let args = build_args(0, 0x60, 0x11, None);

        let message = describe_failure(&args, &output);

        assert!(message.contains("NVAPI initialization failed"));
        assert!(message.contains("NVIDIA GPU"));
    }

    fn fake_exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}
