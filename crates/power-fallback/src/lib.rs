use anyhow::Result;

pub trait PowerFallback {
    fn blank_and_restore(&self) -> Result<()>;
}

#[cfg(windows)]
pub mod windows_monitorpower;
#[cfg(target_os = "macos")]
pub mod macos_pmset;
