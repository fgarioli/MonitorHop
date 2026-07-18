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

/// Retries a fallible operation up to `attempts` times with a short delay
/// between tries. DDC/CI over a switch/dongle chain has been observed (real
/// manual-test session) to intermittently fail with transient errors — a
/// checksum mismatch and an invalid message-length field, on two separate
/// reads — that a bare retry resolves. Shared by the read path
/// (`ddchi_reader`) and, as of IMPROVEMENTS.md #5, the macOS write path
/// (`macos_ioavservice`) as well.
pub(crate) fn retry<T>(attempts: u32, delay: std::time::Duration, mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let mut last_err = None;
    for attempt in 0..attempts {
        match f() {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_err = Some(err);
                if attempt + 1 < attempts {
                    std::thread::sleep(delay);
                }
            }
        }
    }
    Err(last_err.unwrap())
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

// TODO(v2): linux_ddcutil.rs — wrapper over the `ddcutil` CLI, invoked as a
// subprocess (same pattern as windows_nvapi.rs's Command::new), NOT linked
// via FFI/libddcutil bindings: ddcutil is GPL-2.0, this project is MIT, and
// --i2c-source-addr's presence in the public C API is unconfirmed anyway.
// See DECISIONS.md #9 and IMPROVEMENTS.md #9. Out of scope for this milestone.

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

    #[test]
    fn retry_returns_ok_immediately_on_first_success() {
        let calls = std::cell::RefCell::new(0);
        let result = retry(3, Duration::from_millis(1), || {
            *calls.borrow_mut() += 1;
            Ok::<_, anyhow::Error>(42)
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn retry_succeeds_after_transient_failures() {
        let calls = std::cell::RefCell::new(0);
        let result = retry(3, Duration::from_millis(1), || {
            *calls.borrow_mut() += 1;
            if *calls.borrow() < 3 {
                Err(anyhow::anyhow!("transient"))
            } else {
                Ok(42)
            }
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*calls.borrow(), 3);
    }

    #[test]
    fn retry_gives_up_after_exhausting_attempts_and_returns_the_last_error() {
        let calls = std::cell::RefCell::new(0);
        let result = retry(3, Duration::from_millis(1), || {
            *calls.borrow_mut() += 1;
            Err::<i32, _>(anyhow::anyhow!("attempt {}", calls.borrow()))
        });
        assert_eq!(*calls.borrow(), 3);
        assert_eq!(result.unwrap_err().to_string(), "attempt 3");
    }
}
