# KVM Switch Fork MVP — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fork `haimgel/display-switch` into a 5-crate Rust workspace and ship one
working vertical slice: USB hotplug detected on Windows -> NVAPI DDC backend
(source-addr override, via `writeValueToDisplay.exe`) -> LG 34GL750 switches to
HDMI1 (Mac).

**Architecture:** `trigger` crate emits `TriggerEvent`s from a Windows
`WM_DEVICECHANGE` watcher; `kvm_core` resolves the configured target (VCP value,
source-addr, display index) and calls into a `DdcBackend` trait implemented by
`ddc-backend`'s `NvapiBackend`, which shells out to the pre-validated
`tools/writeValueToDisplay.exe`; on failure it falls back to
`power-fallback`'s Windows monitor-power blank/restore and retries once.
`daemon` wires all four crates together behind a CLI.

**Tech Stack:** Rust (workspace, stable 1.89.0, `x86_64-pc-windows-gnu` host),
`rusb`, `winapi` 0.3, `config` 0.15 (ini), `serde`, `clap` 4.6, `simplelog`,
`anyhow`, `log`.

## Global Constraints

- Toolchain: Rust stable **1.89.0**, host **`x86_64-pc-windows-gnu`** (user
  chose GNU over MSVC to avoid installing Visual Studio Build Tools — see
  decision below). `rust-toolchain.toml` (from upstream merge) pins the
  channel to `1.89.0` with no host triple, so the GNU host is picked up from
  the default rustup host set in Task 1.
- **Known risk:** `rusb` depends on `libusb1-sys`, which may need a C
  compiler (`cc`) to build/link on the GNU toolchain (MSVC ships one; a bare
  GNU rustup install does not). If `cargo build -p trigger` fails with a
  missing `cc`/linker error in Task 5, install a standalone MinGW-w64 GCC
  (`winget install BrechtSanders.WinLibs.POSIX.UCRT`, then add its `bin/` to
  `PATH`) and retry. Do not silently switch to `msvc` without checking with
  the user first, since that was an explicit decision.
- Validated hardware recipe (`DECISIONS.md` #4, #7 — do not change these
  values without new hardware evidence): `display_index=0`, VCP code
  `0x60` (`INPUT_SELECT`), value `0x11` (HDMI1), source address `0x50`.
- `tools/writeValueToDisplay.exe` CLI contract (verified by running
  `./writeValueToDisplay.exe` with no args): positional order is
  `[display_index] [input_value] [command_code] [register_address]` — value
  before code, **not** code before value. Exit code is `0` on success,
  non-zero on failure (verified: exit `1` with `NvAPI_GetAssociatedDisplayOutputId() failed`
  when called with an out-of-range index). This corrects an ordering mistake
  in the earlier design spec (`docs/superpowers/specs/2026-07-06-kvm-switch-fork-mvp-design.md`).
  from Command result
- Deviations from the design spec, decided while writing this plan (all for
  documented reasons, see rationale in each task):
  - `crates/core` is renamed to **`crates/kvm_core`** (directory and package
    name) to avoid a Rust package literally named `core` shadowing the
    sysroot `core` crate.
  - `DdcBackend::get_vcp` is dropped — `writeValueToDisplay.exe` is
    write-only, so a `get_vcp` method could never be implemented against it.
  - `DdcBackend`/`PowerFallback` methods take `monitor_index: u32` (an NVAPI
    display ordinal), not `monitor_id: &str` (an EDID string) — matches what
    the exe actually takes as its first argument. Multi-monitor EDID-based
    resolution (upstream's `[monitor1]..[monitor6]` config sections,
    matched by substring against a `ddc_hi`-derived display name) is cut
    entirely for this milestone: the user has one physical monitor, so
    config carries a single optional `nvapi_display_index` (default `0`).
  - Upstream's proactive "jiggle the mouse before every connect" wake nudge
    (`platform::wake_displays`, called unconditionally in `app.rs` before
    every switch) is cut. It is not part of the validated recipe or the
    milestone's success criteria. The equivalent mouse-jiggle primitive is
    kept, but only inside `power-fallback`'s `blank_and_restore`, which is
    the documented fallback path.
  - `Configuration::load` takes an explicit path (CLI flag, default
    `display-switch.ini` in the working directory). Upstream's OS-specific
    config-directory resolution (`dirs` crate, `%APPDATA%`/`~/Library`) and
    `DISPLAY_SWITCH_*` environment variable overlay are cut — not needed for
    a single-machine manual-test milestone, and cutting them removes the
    `dirs` dependency entirely.
  - Upstream's per-monitor `on_usb_connect_execute` / external-command hooks
    are cut — not required by the milestone's success criteria.
  - File-based logging (`WriteLogger` to `%LOCALAPPDATA%\display-switch\...`)
    is cut in favor of terminal-only logging (`TermLogger`), for the same
    reason as the config-path cut above.

---

## Task 1: Install Rust toolchain (GNU host)

No files change in the repo; this is host setup, verified by running
`cargo`/`rustc`. No commit at the end of this task.

- [ ] **Step 1: Install rustup with a GNU default host**

Run:
```
winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements --override "-y --default-host x86_64-pc-windows-gnu --default-toolchain stable"
```

Expected: winget reports a successful install. Rustup and cargo are placed
under `%USERPROFILE%\.cargo\bin`.

- [ ] **Step 2: Open a new shell and verify the toolchain**

Run (in a **new** terminal so `PATH` picks up `.cargo\bin`):
```
rustc --version
cargo --version
rustup show
```

Expected: `rustc`/`cargo` both print version `1.` something recent (stable at
install time), and `rustup show` lists the active toolchain host as
`x86_64-pc-windows-gnu`.

- [ ] **Step 3: Install the pinned toolchain version ahead of time**

The upstream repo (merged in Task 2) pins `channel = "1.89.0"` via
`rust-toolchain.toml`. Install it now so Task 2's post-merge build isn't
blocked on a fresh download:
```
rustup toolchain install 1.89.0-x86_64-pc-windows-gnu
rustup default 1.89.0-x86_64-pc-windows-gnu
```

Expected: `rustc --version` now reports `1.89.0`.

---

## Task 2: Fork upstream history into this repo

**Files:**
- Modify: repo root (merge brings in upstream's `src/`, `Cargo.toml`,
  `Cargo.lock`, `build.rs`, `LICENSE`, `README.md`, `Makefile`,
  `rustfmt.toml`, `rust-toolchain.toml`, `CLAUDE.md`,
  `dev.haim.display-switch.daemon.plist`, `.github/`, `.gitignore`)

**Interfaces:** none (repo-structure task only).

- [ ] **Step 1: Add the upstream remote and fetch**

Run:
```
git remote add upstream https://github.com/haimgel/display-switch
git fetch upstream
```

Expected: fetch succeeds, `git branch -r` shows `upstream/main`.

- [ ] **Step 2: Merge upstream history, preserving both trees**

Run:
```
git merge upstream/main --allow-unrelated-histories -m "Merge upstream haimgel/display-switch history into fork"
```

Expected: merge completes with **no conflicts** (the existing files —
`DECISIONS.md`, `display-switch.ini`, `writeValueToDisplay.exe`, `.vscode/`,
`.claude/`, `docs/` — don't overlap with any path in upstream's tree). Run
`git status` after to confirm a clean working tree and `ls` to confirm
`src/`, `Cargo.toml`, `LICENSE` etc. now exist at the repo root alongside the
pre-existing files.

- [ ] **Step 3: Confirm the pinned toolchain builds the pre-merge upstream code**

Run:
```
cargo build
```

Expected: this builds upstream's *original* single-crate layout (not yet
restructured) using the `1.89.0-x86_64-pc-windows-gnu` toolchain pulled in by
`rust-toolchain.toml`. This is a sanity check only, to catch GNU-toolchain
build problems (see Global Constraints risk note) before Task 3 restructures
everything. If it fails on `rusb`/`libusb1-sys`, apply the MinGW-GCC
workaround from Global Constraints and retry; do not proceed to Task 3 until
this succeeds. If it fails on `nvapi`/`nvapi-sys` for an unrelated reason,
that's fine — Task 3 deletes that dependency (we shell out to
`writeValueToDisplay.exe` instead) so it isn't worth debugging.

- [ ] **Step 4: Commit**

The merge commit was already created in Step 2; nothing further to stage.
Confirm with `git log --oneline -3` that the merge commit is present on top
of `996cf99`.

---

## Task 3: Convert to a Cargo workspace skeleton

**Files:**
- Delete: `src/` (upstream's single-crate source, fully ported in Tasks 4-10), `build.rs`, `Cargo.lock`
- Modify: `Cargo.toml` (root, becomes a virtual workspace manifest)
- Create: `crates/kvm_core/Cargo.toml`, `crates/kvm_core/src/lib.rs`
- Create: `crates/trigger/Cargo.toml`, `crates/trigger/src/lib.rs`
- Create: `crates/ddc-backend/Cargo.toml`, `crates/ddc-backend/src/lib.rs`
- Create: `crates/power-fallback/Cargo.toml`, `crates/power-fallback/src/lib.rs`
- Create: `crates/daemon/Cargo.toml`, `crates/daemon/src/main.rs`
- Move: `writeValueToDisplay.exe` -> `tools/writeValueToDisplay.exe`

**Interfaces:** none yet — every crate is an empty placeholder. Later tasks
fill in real content without changing these Cargo.toml files' dependency
lists (already final here).

- [ ] **Step 1: Remove upstream's single-crate scaffolding**

Run:
```
git rm -r src build.rs Cargo.lock
```

(Leave `LICENSE`, `README.md`, `CLAUDE.md`, `Makefile`, `rustfmt.toml`,
`rust-toolchain.toml`, `.github/`, `dev.haim.display-switch.daemon.plist`,
`.gitignore` untouched — they still describe this fork's provenance and
license.)

- [ ] **Step 2: Move the pre-validated exe into `tools/`**

Run:
```
mkdir tools
git mv writeValueToDisplay.exe tools/writeValueToDisplay.exe
```

- [ ] **Step 3: Replace the root `Cargo.toml` with a virtual workspace manifest**

Replace the entire contents of `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/kvm_core",
    "crates/trigger",
    "crates/ddc-backend",
    "crates/power-fallback",
    "crates/daemon",
]
# TODO(v2): crates/ui-tauri — optional configuration UI, talks to the daemon
# via IPC. Rejected as a foundation; not part of this milestone at all
# (see DECISIONS.md #6).
```

- [ ] **Step 4: Create `crates/trigger`**

`crates/trigger/Cargo.toml`:
```toml
[package]
name = "trigger"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
log = "0.4"
rusb = "0.9"

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["winuser", "libloaderapi"] }
```

`crates/trigger/src/lib.rs`:
```rust
//! Trigger sources that emit `TriggerEvent`s when a watched USB device
//! connects to or disconnects from this host.
```

- [ ] **Step 5: Create `crates/ddc-backend`**

`crates/ddc-backend/Cargo.toml`:
```toml
[package]
name = "ddc-backend"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
log = "0.4"
```

`crates/ddc-backend/src/lib.rs`:
```rust
//! Backends that write VCP feature values to a monitor over DDC/CI.
```

- [ ] **Step 6: Create `crates/power-fallback`**

`crates/power-fallback/Cargo.toml`:
```toml
[package]
name = "power-fallback"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["winuser"] }
```

`crates/power-fallback/src/lib.rs`:
```rust
//! Last-resort monitor power-cycling, used when a DDC switch attempt fails.
```

- [ ] **Step 7: Create `crates/kvm_core`**

`crates/kvm_core/Cargo.toml`:
```toml
[package]
name = "kvm_core"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
log = "0.4"
serde = { version = "1.0", features = ["derive"] }
config = { version = "0.15", features = ["ini"], default-features = false }
paste = "1.0"
trigger = { path = "../trigger" }
ddc-backend = { path = "../ddc-backend" }
power-fallback = { path = "../power-fallback" }
```

`crates/kvm_core/src/lib.rs`:
```rust
//! Configuration parsing and the trigger -> ddc-backend -> power-fallback
//! orchestration logic.
```

- [ ] **Step 8: Create `crates/daemon`**

`crates/daemon/Cargo.toml`:
```toml
[package]
name = "kvm-switch-daemon"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
log = "0.4"
simplelog = "0.12"
clap = { version = "4.6.1", features = ["derive"] }
kvm_core = { path = "../kvm_core" }
trigger = { path = "../trigger" }
ddc-backend = { path = "../ddc-backend" }
power-fallback = { path = "../power-fallback" }

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["wincon"] }
```

`crates/daemon/src/main.rs`:
```rust
fn main() {}
```

- [ ] **Step 9: Build the empty workspace**

Run:
```
cargo build --workspace
```

Expected: succeeds (every crate is a no-op placeholder). This is the
milestone's first `cargo build --workspace` checkpoint from the design spec,
confirming the workspace itself (member list, path dependencies, Windows
`cfg` dependency gating) is wired correctly before any real logic exists.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "Restructure into a 5-crate Cargo workspace skeleton"
```

---

## Task 4: `ddc-backend` — NVAPI shell-out backend

**Files:**
- Modify: `crates/ddc-backend/src/lib.rs`
- Create: `crates/ddc-backend/src/windows_nvapi.rs`
- Create: `crates/ddc-backend/src/windows_generic.rs`

**Interfaces:**
- Produces: `pub trait DdcBackend { fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> anyhow::Result<()>; }`
- Produces: `pub struct NvapiBackend` with `pub fn new(exe_path: std::path::PathBuf) -> Self`, implementing `DdcBackend`.
- Produces: `pub struct GenericDdcBackend` implementing `DdcBackend` (unimplemented stub).

- [ ] **Step 1: Write the failing test for argument building**

Create `crates/ddc-backend/src/windows_nvapi.rs`:
```rust
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

impl DdcBackend for NvapiBackend {
    fn set_vcp(&self, _monitor_index: u32, _code: u8, _value: u16, _source_addr: Option<u8>) -> Result<()> {
        unimplemented!()
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ddc-backend`
Expected: **compile error** — `build_args` is not defined yet.

- [ ] **Step 3: Implement `build_args` and wire it into `set_vcp`**

Replace the `NvapiBackend`/`impl DdcBackend for NvapiBackend` block in
`crates/ddc-backend/src/windows_nvapi.rs` with:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ddc-backend`
Expected: `build_args_uses_validated_default_source_addr` and
`build_args_honors_explicit_source_addr_override` both `PASS`.

- [ ] **Step 5: Add the trait and the generic fallback stub**

Replace `crates/ddc-backend/src/lib.rs`:
```rust
use anyhow::Result;

pub trait DdcBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()>;
}

// TODO(macos): macos_ioavservice.rs — IOAVServiceReadI2C/WriteI2C backend,
// blocked on Spike #2 (see DECISIONS.md #5, #7).
// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9).

pub mod windows_generic;
pub mod windows_nvapi;
```

Create `crates/ddc-backend/src/windows_generic.rs`:
```rust
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
```

- [ ] **Step 6: Build and test the whole crate**

Run: `cargo test -p ddc-backend`
Expected: all tests pass, crate builds clean.

- [ ] **Step 7: Commit**

```bash
git add crates/ddc-backend
git commit -m "Implement NvapiBackend shelling out to writeValueToDisplay.exe"
```

---

## Task 5: `trigger` — USB hotplug watcher (Windows `WM_DEVICECHANGE`)

Upstream cannot use `rusb`'s hotplug API on Windows (libusb hotplug is
unsupported there — see upstream's own comment in
`platform/pnp_detect_windows.rs`). This ports upstream's actual Windows path:
an invisible window receiving `WM_DEVICECHANGE`, diffing `rusb::devices()`
against the previous snapshot.

**Files:**
- Modify: `crates/trigger/src/lib.rs`
- Create: `crates/trigger/src/usb_hotplug.rs`

**Interfaces:**
- Produces: `pub enum TriggerEvent { HostGainedFocus, HostLostFocus }` (`Debug, Clone, Copy, PartialEq, Eq`)
- Produces: `pub trait TriggerSource { fn watch(&self) -> std::sync::mpsc::Receiver<TriggerEvent>; }`
- Produces: `pub struct UsbHotplugTrigger` with `pub fn new(usb_device: &str) -> Self`, implementing `TriggerSource`.
- Consumes: nothing from earlier tasks.

- [ ] **Step 1: Define `TriggerEvent`/`TriggerSource` in `lib.rs`**

Replace `crates/trigger/src/lib.rs`:
```rust
use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    HostGainedFocus,
    HostLostFocus,
}

pub trait TriggerSource {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent>;
}

// TODO(v2): bluetooth_hid.rs — native Bluetooth HID watchers per OS.
// TODO(v2): hidpp_receiver.rs — hidapi + HID++ 1.0/2.0 parsing, notification
// 0x41 + feature 0x1814 "Change Host" (see DECISIONS.md #6, #8).

pub mod usb_hotplug;
```

- [ ] **Step 2: Write the failing test for the pure diffing logic**

Create `crates/trigger/src/usb_hotplug.rs`:
```rust
use crate::TriggerEvent;
use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gained_focus_when_watched_device_appears() {
        let current: HashSet<String> = HashSet::new();
        let new: HashSet<String> = ["17e9:6000".to_string()].into_iter().collect();
        assert_eq!(diff_to_events(&current, &new, "17e9:6000"), vec![TriggerEvent::HostGainedFocus]);
    }

    #[test]
    fn lost_focus_when_watched_device_disappears() {
        let current: HashSet<String> = ["17e9:6000".to_string()].into_iter().collect();
        let new: HashSet<String> = HashSet::new();
        assert_eq!(diff_to_events(&current, &new, "17e9:6000"), vec![TriggerEvent::HostLostFocus]);
    }

    #[test]
    fn no_event_for_unrelated_device_changes() {
        let current: HashSet<String> = HashSet::new();
        let new: HashSet<String> = ["aaaa:bbbb".to_string()].into_iter().collect();
        assert!(diff_to_events(&current, &new, "17e9:6000").is_empty());
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p trigger`
Expected: **compile error** — `diff_to_events` is not defined yet.

- [ ] **Step 4: Implement `diff_to_events`**

Add above the `#[cfg(test)]` block in `crates/trigger/src/usb_hotplug.rs`:
```rust
fn diff_to_events(current: &HashSet<String>, new: &HashSet<String>, watched: &str) -> Vec<TriggerEvent> {
    let mut events = Vec::new();
    if new.contains(watched) && !current.contains(watched) {
        events.push(TriggerEvent::HostGainedFocus);
    }
    if current.contains(watched) && !new.contains(watched) {
        events.push(TriggerEvent::HostLostFocus);
    }
    events
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p trigger`
Expected: all three tests `PASS`.

- [ ] **Step 6: Add the real Windows message-loop watcher around the tested logic**

Add to the top of `crates/trigger/src/usb_hotplug.rs` (imports) and below the
`diff_to_events` function (before the `#[cfg(test)]` block):

```rust
use anyhow::{anyhow, Result};
use rusb::UsbContext;
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::{self, Sender};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::ntdef::LPCWSTR;
use winapi::shared::windef::{HBRUSH, HCURSOR, HICON, HWND};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW, PostQuitMessage,
    RegisterClassW, SetWindowLongPtrW, TranslateMessage, GWLP_USERDATA, MSG, WM_CREATE, WM_DESTROY,
    WM_DEVICECHANGE, WNDCLASSW,
};
```

then, still above `#[cfg(test)]`:

```rust
pub struct UsbHotplugTrigger {
    usb_device: String,
}

impl UsbHotplugTrigger {
    pub fn new(usb_device: &str) -> Self {
        Self {
            usb_device: usb_device.to_lowercase(),
        }
    }
}

impl crate::TriggerSource for UsbHotplugTrigger {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent> {
        let (tx, rx) = mpsc::channel();
        let usb_device = self.usb_device.clone();
        std::thread::spawn(move || {
            if let Err(err) = run_message_loop(usb_device, tx) {
                log::error!("USB hotplug detection failed: {:?}", err);
            }
        });
        rx
    }
}

fn device_id<T: UsbContext>(device: &rusb::Device<T>) -> Option<String> {
    device
        .device_descriptor()
        .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
        .ok()
}

fn read_device_list() -> Result<HashSet<String>> {
    Ok(rusb::devices()?.iter().filter_map(|device| device_id(&device)).collect())
}

struct WindowState {
    usb_device: String,
    sender: Sender<TriggerEvent>,
    current_devices: HashSet<String>,
}

impl WindowState {
    fn handle_hotplug_event(&mut self) {
        let new_devices = match read_device_list() {
            Ok(devices) => devices,
            Err(err) => {
                log::error!("Cannot get list of USB devices: {:?}", err);
                return;
            }
        };
        for event in diff_to_events(&self.current_devices, &new_devices, &self.usb_device) {
            let _ = self.sender.send(event);
        }
        self.current_devices = new_devices;
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = lparam as *mut winapi::um::winuser::CREATESTRUCTW;
            let state_ptr = create_struct.as_ref().unwrap().lpCreateParams;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
        }
        WM_DESTROY => PostQuitMessage(0),
        WM_DEVICECHANGE => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if state_ptr != 0 {
                let state: &mut WindowState = &mut *(state_ptr as *mut WindowState);
                state.handle_hotplug_event();
            }
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

fn run_message_loop(usb_device: String, sender: Sender<TriggerEvent>) -> Result<()> {
    let mut state = Box::new(WindowState {
        current_devices: read_device_list().unwrap_or_default(),
        usb_device,
        sender,
    });

    let class_name: Vec<u16> = OsStr::new("KvmSwitchPnPDetectWindowClass").encode_wide().chain(once(0)).collect();
    let window_name: Vec<u16> = OsStr::new("KvmSwitchPnPDetectWindow").encode_wide().chain(once(0)).collect();
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: 0 as HICON,
        hCursor: 0 as HCURSOR,
        hbrBackground: 0 as HBRUSH,
        lpszMenuName: 0 as LPCWSTR,
        lpszClassName: class_name.as_ptr(),
    };

    let hwnd = unsafe {
        if RegisterClassW(&wc) == 0 {
            return Err(anyhow!("failed to register window class"));
        }
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            state.as_mut() as *mut WindowState as *mut _,
        )
    };
    if hwnd.is_null() {
        return Err(anyhow!("failed to create window"));
    }

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let val = GetMessageW(&mut msg, hwnd, 0, 0);
            if val == 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
```

- [ ] **Step 7: Build and test the whole crate**

Run: `cargo test -p trigger`
Expected: the three `diff_to_events` tests still pass; the crate compiles
clean (the window-loop code isn't exercised by tests — it needs a live
Windows message pump and a real USB event, which is covered later by
`MANUAL_TEST.md`, per this milestone's stated test strategy).

- [ ] **Step 8: Commit**

```bash
git add crates/trigger
git commit -m "Port Windows WM_DEVICECHANGE USB hotplug watcher behind TriggerSource"
```

---

## Task 6: `power-fallback` — Windows monitor power blank/restore

**Files:**
- Modify: `crates/power-fallback/src/lib.rs`
- Create: `crates/power-fallback/src/windows_monitorpower.rs`

**Interfaces:**
- Produces: `pub trait PowerFallback { fn blank_and_restore(&self) -> anyhow::Result<()>; }`
- Produces: `pub struct WindowsMonitorPower;` implementing `PowerFallback`.

- [ ] **Step 1: Define the trait**

Replace `crates/power-fallback/src/lib.rs`:
```rust
use anyhow::Result;

pub trait PowerFallback {
    fn blank_and_restore(&self) -> Result<()>;
}

// TODO(macos): macos_pmset.rs — `pmset displaysleepnow` + wake, waits on the
// macOS DDC backend itself (see DECISIONS.md #8).

pub mod windows_monitorpower;
```

- [ ] **Step 2: Implement blank/restore**

This is direct Win32 API plumbing (`SendMessageW`, `mouse_event`) with no
pure logic worth isolating for a unit test — it's covered by
`MANUAL_TEST.md`. Create `crates/power-fallback/src/windows_monitorpower.rs`:

```rust
use crate::PowerFallback;
use anyhow::Result;
use std::{thread, time};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{mouse_event, SendMessageW, MOUSEEVENTF_MOVE, SC_MONITORPOWER, WM_SYSCOMMAND};

const HWND_BROADCAST: HWND = 0xffff as HWND;
/// Second `WM_SYSCOMMAND`/`SC_MONITORPOWER` parameter: 2 = off, 1 = low power, -1 = on.
const MONITOR_OFF: isize = 2;
const BLANK_DURATION_MS: u64 = 500;

pub struct WindowsMonitorPower;

impl PowerFallback for WindowsMonitorPower {
    fn blank_and_restore(&self) -> Result<()> {
        unsafe {
            SendMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER as usize, MONITOR_OFF);
        }
        thread::sleep(time::Duration::from_millis(BLANK_DURATION_MS));
        // Jiggle the mouse to wake the display back up.
        unsafe {
            mouse_event(MOUSEEVENTF_MOVE, 0, 1, 0, 0);
            thread::sleep(time::Duration::from_millis(50));
            mouse_event(MOUSEEVENTF_MOVE, 0, 0xffffffff, 0, 0);
        }
        Ok(())
    }
}
```

- [ ] **Step 3: Build the crate**

Run: `cargo build -p power-fallback`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git add crates/power-fallback
git commit -m "Implement Windows SC_MONITORPOWER blank/restore fallback"
```

---

## Task 7: `kvm_core::config` — configuration parsing

**Files:**
- Create: `crates/kvm_core/src/config.rs`
- Modify: `crates/kvm_core/src/lib.rs`

**Interfaces:**
- Produces: `pub struct Configuration { pub usb_device: String, pub on_usb_connect: Option<InputSource>, pub on_usb_disconnect: Option<InputSource>, pub on_usb_connect_source_addr: Option<u8>, pub nvapi_display_index: Option<u32> }`
- Produces: `impl Configuration { pub fn load(path: &std::path::Path) -> anyhow::Result<Self>; pub fn display_index(&self) -> u32 }`
- Produces: `pub enum InputSource` with `pub fn value(&self) -> u16`, `Deserialize`, `Clone`, `Copy`, `PartialEq`, `Eq`.

- [ ] **Step 1: Write the failing tests**

Create `crates/kvm_core/src/config.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use config::FileFormat::Ini;

    fn load_test_config(config_str: &str) -> Result<Configuration, config::ConfigError> {
        config::Config::builder()
            .add_source(config::File::from_str(config_str, Ini))
            .build()?
            .try_deserialize()
    }

    #[test]
    fn usb_device_is_lowercased() {
        let config = load_test_config(
            r#"
            usb_device = "17E9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.usb_device, "17e9:6000");
    }

    #[test]
    fn symbolic_input_source_resolves_to_vcp_value() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn hex_input_source_is_accepted() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "0x11"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn source_addr_defaults_to_none() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, None);
    }

    #[test]
    fn source_addr_override_is_parsed_as_hex() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_source_addr = "0x50"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, Some(0x50));
    }

    #[test]
    fn display_index_defaults_to_zero() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.display_index(), 0);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kvm_core`
Expected: **compile error** — `Configuration`, `InputSource` etc. don't
exist yet.

- [ ] **Step 3: Implement `InputSource` and `Configuration`**

Add above the `#[cfg(test)]` block in `crates/kvm_core/src/config.rs`:

```rust
use anyhow::Result;
use paste::paste;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};
use std::convert::TryFrom;
use std::fmt;

macro_rules! symbolic_input_source {
    ($($name:ident: $value:expr)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum SymbolicInputSource {
            $($name = $value,)*
        }

        impl TryFrom<u16> for SymbolicInputSource {
            type Error = ();
            fn try_from(v: u16) -> std::result::Result<Self, Self::Error> {
                match v {
                    $($value => Ok(Self::$name),)*
                    _ => Err(()),
                }
            }
        }

        impl TryFrom<&str> for SymbolicInputSource {
            type Error = ();
            fn try_from(v: &str) -> std::result::Result<Self, Self::Error> {
                paste! {
                    match v.to_lowercase().as_str() {
                        $(stringify!([< $name:lower >]) => Ok(Self::$name),)*
                        _ => Err(()),
                    }
                }
            }
        }
    }
}

symbolic_input_source! {
    DisplayPort1: 0x0f
    DisplayPort2: 0x10
    Hdmi1: 0x11
    Hdmi2: 0x12
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Raw(u16),
    Symbolic(SymbolicInputSource),
}

impl InputSource {
    pub fn value(&self) -> u16 {
        match self {
            Self::Symbolic(sym) => *sym as u16,
            Self::Raw(value) => *value,
        }
    }

    fn normalize(self) -> Self {
        match self {
            Self::Symbolic(_) => self,
            Self::Raw(value) => SymbolicInputSource::try_from(value).map(Self::Symbolic).unwrap_or(Self::Raw(value)),
        }
    }
}

impl fmt::Debug for InputSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symbolic(sym) => write!(f, "{:?}(0x{:x})", sym, *sym as u16),
            Self::Raw(value) => write!(f, "Custom(0x{:x})", value),
        }
    }
}

fn parse_int(s: &str) -> std::result::Result<u16, std::num::ParseIntError> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
    } else {
        s.parse::<u16>()
    }
}

impl<'de> Deserialize<'de> for InputSource {
    fn deserialize<D>(deserializer: D) -> std::result::Result<InputSource, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?.trim().to_lowercase();
        if let Ok(val) = parse_int(&str) {
            Ok(Self::Raw(val).normalize())
        } else {
            SymbolicInputSource::try_from(str.as_str())
                .map(Self::Symbolic)
                .map_err(|_| D::Error::custom(format!("Invalid input source: {}", str)))
        }
    }
}

fn parse_source_addr<'de, D>(deserializer: D) -> std::result::Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => {
            let s = s.trim();
            let hex = s.strip_prefix("0x").unwrap_or(s);
            u8::from_str_radix(hex, 16)
                .map(Some)
                .map_err(|_| DeError::custom(format!("Invalid source address: {}", s)))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Configuration {
    #[serde(deserialize_with = "Configuration::deserialize_usb_device")]
    pub usb_device: String,
    pub on_usb_connect: Option<InputSource>,
    pub on_usb_disconnect: Option<InputSource>,
    #[serde(default, deserialize_with = "parse_source_addr")]
    pub on_usb_connect_source_addr: Option<u8>,
    #[serde(default)]
    pub nvapi_display_index: Option<u32>,
}

impl Configuration {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let builder = config::Config::builder().add_source(config::File::from(path));
        let config: Self = builder.build()?.try_deserialize()?;
        Ok(config)
    }

    fn deserialize_usb_device<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(s.to_lowercase())
    }

    pub fn display_index(&self) -> u32 {
        self.nvapi_display_index.unwrap_or(0)
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kvm_core`
Expected: all six tests in `config::tests` `PASS`.

- [ ] **Step 5: Wire the module into `lib.rs`**

Replace `crates/kvm_core/src/lib.rs`:
```rust
pub mod config;
```

- [ ] **Step 6: Commit**

```bash
git add crates/kvm_core
git commit -m "Add simplified single-monitor Configuration parsing"
```

---

## Task 8: `kvm_core::monitor_map` — resolve config into a switch target

**Files:**
- Create: `crates/kvm_core/src/monitor_map.rs`
- Modify: `crates/kvm_core/src/lib.rs`

**Interfaces:**
- Consumes: `kvm_core::config::{Configuration, InputSource}` (Task 7).
- Produces: `pub enum SwitchDirection { Connect, Disconnect }` (`Debug, Clone, Copy, PartialEq, Eq`)
- Produces: `pub struct SwitchTarget { pub display_index: u32, pub input_source: InputSource, pub source_addr: Option<u8> }`
- Produces: `pub fn resolve(config: &Configuration, direction: SwitchDirection) -> Option<SwitchTarget>`

- [ ] **Step 1: Write the failing tests**

Create `crates/kvm_core/src/monitor_map.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Configuration;

    fn load(config_str: &str) -> Configuration {
        config::Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Ini))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap()
    }

    #[test]
    fn resolves_connect_target_from_config() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_source_addr = "0x50"
        "#,
        );
        let target = resolve(&config, SwitchDirection::Connect).unwrap();
        assert_eq!(target.display_index, 0);
        assert_eq!(target.input_source.value(), 0x11);
        assert_eq!(target.source_addr, Some(0x50));
    }

    #[test]
    fn disconnect_with_no_config_resolves_to_none() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        );
        assert!(resolve(&config, SwitchDirection::Disconnect).is_none());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kvm_core`
Expected: **compile error** — `resolve`, `SwitchDirection` not defined yet.

- [ ] **Step 3: Implement `resolve`**

Add above the `#[cfg(test)]` block in `crates/kvm_core/src/monitor_map.rs`:
```rust
use crate::config::{Configuration, InputSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchDirection {
    Connect,
    Disconnect,
}

pub struct SwitchTarget {
    pub display_index: u32,
    pub input_source: InputSource,
    pub source_addr: Option<u8>,
}

pub fn resolve(config: &Configuration, direction: SwitchDirection) -> Option<SwitchTarget> {
    let input_source = match direction {
        SwitchDirection::Connect => config.on_usb_connect,
        SwitchDirection::Disconnect => config.on_usb_disconnect,
    }?;
    Some(SwitchTarget {
        display_index: config.display_index(),
        input_source,
        source_addr: config.on_usb_connect_source_addr,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kvm_core`
Expected: both `monitor_map::tests` pass, plus all `config::tests` from
Task 7 still pass.

- [ ] **Step 5: Wire the module into `lib.rs`**

Replace `crates/kvm_core/src/lib.rs`:
```rust
pub mod config;
pub mod monitor_map;
```

- [ ] **Step 6: Commit**

```bash
git add crates/kvm_core
git commit -m "Resolve trigger direction + config into a SwitchTarget"
```

---

## Task 9: `kvm_core::orchestrator` — wire trigger, ddc-backend, power-fallback

**Files:**
- Create: `crates/kvm_core/src/orchestrator.rs`
- Modify: `crates/kvm_core/src/lib.rs`

**Interfaces:**
- Consumes: `kvm_core::config::Configuration` (Task 7), `kvm_core::monitor_map::{resolve, SwitchDirection}` (Task 8), `ddc_backend::DdcBackend` (Task 4), `power_fallback::PowerFallback` (Task 6), `trigger::TriggerEvent` (Task 5).
- Produces: `pub fn handle_event(event: trigger::TriggerEvent, config: &Configuration, ddc_backend: &dyn DdcBackend, power_fallback: &dyn PowerFallback)`

- [ ] **Step 1: Write the failing tests with fake backends**

Create `crates/kvm_core/src/orchestrator.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Configuration;
    use ddc_backend::DdcBackend;
    use power_fallback::PowerFallback;
    use std::cell::RefCell;
    use trigger::TriggerEvent;

    struct FakeDdc {
        calls: RefCell<Vec<(u32, u8, u16, Option<u8>)>>,
        fail_first_n: RefCell<u32>,
    }

    impl DdcBackend for FakeDdc {
        fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> anyhow::Result<()> {
            self.calls.borrow_mut().push((monitor_index, code, value, source_addr));
            let mut remaining = self.fail_first_n.borrow_mut();
            if *remaining > 0 {
                *remaining -= 1;
                return Err(anyhow::anyhow!("simulated failure"));
            }
            Ok(())
        }
    }

    struct FakePower {
        called: RefCell<u32>,
    }

    impl PowerFallback for FakePower {
        fn blank_and_restore(&self) -> anyhow::Result<()> {
            *self.called.borrow_mut() += 1;
            Ok(())
        }
    }

    fn load(config_str: &str) -> Configuration {
        config::Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Ini))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap()
    }

    #[test]
    fn successful_switch_calls_ddc_backend_once_with_resolved_target() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_source_addr = "0x50"
        "#,
        );
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostGainedFocus, &config, &ddc, &power);

        assert_eq!(*ddc.calls.borrow(), vec![(0, 0x60, 0x11, Some(0x50))]);
        assert_eq!(*power.called.borrow(), 0);
    }

    #[test]
    fn failed_switch_triggers_power_fallback_and_retries_once() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        );
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(1),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostGainedFocus, &config, &ddc, &power);

        assert_eq!(ddc.calls.borrow().len(), 2);
        assert_eq!(*power.called.borrow(), 1);
    }

    #[test]
    fn unconfigured_direction_does_not_touch_backend() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        );
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostLostFocus, &config, &ddc, &power);

        assert!(ddc.calls.borrow().is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kvm_core`
Expected: **compile error** — `handle_event` is not defined yet.

- [ ] **Step 3: Implement `handle_event`**

Add above the `#[cfg(test)]` block in `crates/kvm_core/src/orchestrator.rs`:
```rust
use crate::config::Configuration;
use crate::monitor_map::{self, SwitchDirection};
use ddc_backend::DdcBackend;
use power_fallback::PowerFallback;
use trigger::TriggerEvent;

/// VCP feature code for input select (DDC/CI standard) — see DECISIONS.md #4.
const INPUT_SELECT: u8 = 0x60;

pub fn handle_event(
    event: TriggerEvent,
    config: &Configuration,
    ddc_backend: &dyn DdcBackend,
    power_fallback: &dyn PowerFallback,
) {
    let direction = match event {
        TriggerEvent::HostGainedFocus => SwitchDirection::Connect,
        TriggerEvent::HostLostFocus => SwitchDirection::Disconnect,
    };
    let Some(target) = monitor_map::resolve(config, direction) else {
        log::info!("No input source configured for {:?}, skipping", direction);
        return;
    };
    let attempt = |ddc_backend: &dyn DdcBackend| {
        ddc_backend.set_vcp(target.display_index, INPUT_SELECT, target.input_source.value(), target.source_addr)
    };
    if let Err(err) = attempt(ddc_backend) {
        log::warn!("Failed to switch display input: {:?}. Retrying after power fallback.", err);
        if let Err(err) = power_fallback.blank_and_restore() {
            log::error!("Power fallback failed: {:?}", err);
        }
        if let Err(err) = attempt(ddc_backend) {
            log::error!("Retry failed, giving up: {:?}", err);
        }
    } else {
        log::info!("Display switched to {:?} for {:?}", target.input_source, direction);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kvm_core`
Expected: all three `orchestrator::tests` pass, plus every earlier
`kvm_core` test from Tasks 7-8 still passes.

- [ ] **Step 5: Wire the module into `lib.rs`**

Replace `crates/kvm_core/src/lib.rs`:
```rust
pub mod config;
pub mod monitor_map;
pub mod orchestrator;
```

- [ ] **Step 6: Commit**

```bash
git add crates/kvm_core
git commit -m "Wire trigger events to ddc-backend with power-fallback retry"
```

---

## Task 10: `daemon` — CLI wiring + example config

**Files:**
- Modify: `crates/daemon/src/main.rs`
- Create: `config/kvm-switch.example.ini`

**Interfaces:**
- Consumes: `kvm_core::config::Configuration::load` (Task 7), `kvm_core::orchestrator::handle_event` (Task 9), `trigger::{TriggerSource, usb_hotplug::UsbHotplugTrigger}` (Task 5), `ddc_backend::windows_nvapi::NvapiBackend` (Task 4), `power_fallback::windows_monitorpower::WindowsMonitorPower` (Task 6).
- Produces: the `kvm-switch-daemon` binary.

- [ ] **Step 1: Replace the placeholder `main.rs`**

Replace `crates/daemon/src/main.rs`:
```rust
use anyhow::{Context, Result};
use clap::Parser;
use ddc_backend::windows_nvapi::NvapiBackend;
use kvm_core::config::Configuration;
use kvm_core::orchestrator;
use power_fallback::windows_monitorpower::WindowsMonitorPower;
use trigger::usb_hotplug::UsbHotplugTrigger;
use trigger::TriggerSource;
use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Print debug information
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    /// Path to the configuration file
    #[arg(short = 'c', long = "config")]
    config_file_path: Option<std::path::PathBuf>,
}

/// Re-attach the console if the parent process has one, so log output shows
/// up when run from the command line.
fn attach_console() {
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn init_logging(debug: bool) -> Result<()> {
    use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
    let level = if debug { LevelFilter::Debug } else { LevelFilter::Info };
    CombinedLogger::init(vec![TermLogger::new(level, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)])
        .context("failed to initialize logging")
}

/// Resolves `tools/writeValueToDisplay.exe` relative to the daemon binary's
/// own directory (see `docs/superpowers/specs/2026-07-06-kvm-switch-fork-mvp-design.md`).
fn default_exe_path() -> Result<std::path::PathBuf> {
    let mut path = std::env::current_exe().context("failed to locate daemon executable")?;
    path.pop();
    path.push("tools");
    path.push("writeValueToDisplay.exe");
    Ok(path)
}

fn main() -> Result<()> {
    attach_console();
    let args = Args::parse();
    init_logging(args.debug)?;

    let config_path = args.config_file_path.unwrap_or_else(|| std::path::PathBuf::from("display-switch.ini"));
    let config = Configuration::load(&config_path)
        .with_context(|| format!("failed to load configuration from {:?}", config_path))?;

    let ddc_backend = NvapiBackend::new(default_exe_path()?);
    let power_fallback = WindowsMonitorPower;
    let trigger_source = UsbHotplugTrigger::new(&config.usb_device);

    log::info!("kvm-switch daemon started, watching USB device {}", config.usb_device);
    for event in trigger_source.watch() {
        orchestrator::handle_event(event, &config, &ddc_backend, &power_fallback);
    }
    Ok(())
}
```

- [ ] **Step 2: Write the example config**

Create `config/kvm-switch.example.ini`:
```ini
# Vendor:Product ID (hex) of the USB device to watch (e.g. the KVM switch, or
# a peripheral plugged into it). Find it in Device Manager.
usb_device = "17e9:6000"

# Input source to switch this monitor to when the watched USB device connects
# to this Windows host. Accepts a symbolic name (DisplayPort1, DisplayPort2,
# Hdmi1, Hdmi2) or a raw VCP value ("0x11" / decimal).
on_usb_connect = "Hdmi1"

# Optional: input source to switch to when the device disconnects. Leave
# unset to do nothing on disconnect.
# on_usb_disconnect = "DisplayPort2"

# Optional: I2C source-address override passed to writeValueToDisplay.exe.
# Defaults to 0x50, validated for the LG 34GL750 (see DECISIONS.md #4).
# Windows' standard DDC API hardcodes 0x51 and does not expose this
# override; only NVAPI raw I2C access allows it.
# on_usb_connect_source_addr = "0x50"

# Optional: NVAPI display index (0 = first screen) passed as
# writeValueToDisplay.exe's `display_index` argument. Defaults to 0.
# nvapi_display_index = 0
```

- [ ] **Step 3: Build the whole workspace**

Run: `cargo build --workspace`
Expected: succeeds cleanly (the milestone's second `cargo build --workspace`
checkpoint — this time with all real logic in place).

- [ ] **Step 4: Run the whole test suite**

Run: `cargo test --workspace`
Expected: every test from Tasks 4, 5, 7, 8, 9 passes (`ddc-backend`:
2, `trigger`: 3, `kvm_core`: 6 + 2 + 3 = 11). `daemon` and `power-fallback`
have no unit tests (integration-only, per Global Constraints).

- [ ] **Step 5: Commit**

```bash
git add crates/daemon config/kvm-switch.example.ini
git commit -m "Wire daemon CLI: trigger -> orchestrator -> ddc-backend/power-fallback"
```

---

## Task 11: Manual test documentation and final verification

**Files:**
- Create: `MANUAL_TEST.md`

**Interfaces:** none (documentation only).

- [ ] **Step 1: Write `MANUAL_TEST.md`**

Create `MANUAL_TEST.md`:
```markdown
# Manual Test: Windows -> Mac switch via USB hotplug

This milestone's success criterion cannot be automated — it depends on real
hardware (LG 34GL750, NVIDIA GPU, USB switch VID:PID 17e9:6000). Run this
after `cargo build --workspace` succeeds.

## Setup

1. Copy `config/kvm-switch.example.ini` to `display-switch.ini` in the repo
   root (or pass `--config <path>` to the daemon) and uncomment/adjust
   `usb_device` / `on_usb_connect` if your hardware differs.
2. Confirm `tools/writeValueToDisplay.exe` exists.

## Steps

1. Run the daemon with debug logging:
   ```
   cargo run -p kvm-switch-daemon -- --debug
   ```
2. With the monitor showing the Windows host (DisplayPort), physically
   toggle the USB switch so the watched device (`17e9:6000`) connects to the
   Windows host's USB bus.
3. Observe in the daemon's log output:
   - a log line noting the device was added to `current_devices`
   - `Display switched to ... for Connect`
4. Confirm the monitor switches to HDMI1 (Mac) within a few seconds.
5. Repeat steps 2-4 five times in a row to confirm reliability (per
   DECISIONS.md's milestone criterion of "reliably across repeated cycles").

## Known non-goals for this milestone

- The Mac -> Windows direction is handled separately by BetterDisplay
  running on the Mac (see DECISIONS.md #5), not by this daemon.
- No automated test exists for this end-to-end flow — it requires
  physically toggling the USB switch and observing the monitor.
```

- [ ] **Step 2: Full workspace check**

Run:
```
cargo build --workspace
cargo test --workspace
```

Expected: both succeed with zero errors — this is the milestone's final
success criterion from the design spec (`cargo build --workspace` clean),
plus every unit test from Tasks 4-9 passing.

- [ ] **Step 3: Commit**

```bash
git add MANUAL_TEST.md
git commit -m "Add manual end-to-end test instructions for the Windows->Mac switch"
```

- [ ] **Step 4: Run the actual manual test**

Follow `MANUAL_TEST.md` end to end with the real hardware. This is the
milestone's true acceptance test — do not consider the milestone done until
this passes across multiple cycles.
