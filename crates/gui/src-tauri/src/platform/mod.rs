//! Compile-time platform dispatch for the GUI crate's OS-specific thread
//! spawners, mirroring the one-file-per-backend convention the adapter crates
//! already use (`ddc-backend/src/lib.rs`, `power-fallback/src/lib.rs`,
//! `trigger/src/lib.rs`).
//!
//! Each per-OS module exposes the same three function signatures —
//! `spawn_switch_trigger`, `spawn_consumer`, `spawn_mxkeys_trigger` — so
//! adding a platform is a new file plus a `cfg` pair here, with no edits to
//! `main.rs`, `commands.rs` or `device_database.rs`. A Linux backend
//! (`docs/IMPROVEMENTS.md` #9) would land as `platform/linux.rs`.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::*;
