//! Backends that write VCP feature values to a monitor over DDC/CI, plus a
//! separate read-only capability for enumerating monitors and their
//! supported inputs (used by the GUI's configuration wizard, never by the
//! orchestrator's write path).

use anyhow::Result;
use std::sync::{Mutex, MutexGuard, OnceLock};

static DDC_IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Serializes all access to the shared DDC/CI I2C channel. Reads
/// (`MonitorReader`) and writes (`DdcBackend::set_vcp`) contend for the same
/// physical bus regardless of which Rust type issues the call — a real
/// manual-test session found a read landing mid-write silently corrupts the
/// write (screen stayed black, success still reported; see
/// `docs/IMPROVEMENTS.md` #3). Callers must hold the returned guard for the
/// full duration of their DDC/CI I/O, not just part of it.
pub fn ddc_io_lock() -> MutexGuard<'static, ()> {
    DDC_IO_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    /// Reads VCP feature `0x60` (input select)'s *current* value — lets the
    /// GUI's main screen highlight which input is already active instead of
    /// presenting every input as equally "not yet chosen". Read-only, same
    /// as `enumerate`/`input_codes`: never touches the orchestrator's write
    /// path.
    fn current_input(&self, display_index: u32) -> Result<u8>;
}

pub mod capabilities;
pub use capabilities::parse_input_codes;

// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9). Out of scope.

#[cfg(any(windows, target_os = "macos"))]
pub mod ddchi_reader;

#[cfg(windows)]
pub mod windows_generic;
#[cfg(windows)]
pub mod windows_nvapi;

#[cfg(target_os = "macos")]
pub mod macos_ioavservice;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    /// Proves `ddc_io_lock()` actually serializes callers: thread `a` holds
    /// the lock across a sleep, thread `b` starts shortly after and must
    /// block until `a` both entered AND exited its critical section — so the
    /// recorded order can only be a, A, b, never a, b, A.
    #[test]
    fn ddc_io_lock_serializes_concurrent_callers() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let order_a = Arc::clone(&order);
        let order_b = Arc::clone(&order);

        let a = thread::spawn(move || {
            let _guard = ddc_io_lock();
            order_a.lock().unwrap().push('a');
            thread::sleep(Duration::from_millis(50));
            order_a.lock().unwrap().push('A');
        });
        thread::sleep(Duration::from_millis(10));
        let b = thread::spawn(move || {
            let _guard = ddc_io_lock();
            order_b.lock().unwrap().push('b');
        });

        a.join().unwrap();
        b.join().unwrap();

        assert_eq!(*order.lock().unwrap(), vec!['a', 'A', 'b']);
    }
}
