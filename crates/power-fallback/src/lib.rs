use anyhow::Result;

pub trait PowerFallback {
    fn blank_and_restore(&self) -> Result<()>;
}

// TODO(macos): macos_pmset.rs — `pmset displaysleepnow` + wake, waits on the
// macOS DDC backend itself (see DECISIONS.md #8).

pub mod windows_monitorpower;
