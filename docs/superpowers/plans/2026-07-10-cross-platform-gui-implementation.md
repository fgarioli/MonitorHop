# Cross-Platform GUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the headless `kvm-switch-daemon` CLI into a single-process Tauri
GUI application (`kvm-switch-gui`) that wraps the existing trigger →
orchestrator → ddc-backend/power-fallback core: a setup wizard that
auto-detects USB devices and lists monitor codes/inputs, a main screen with
manual software switching, MX Keys presence detection, and tray-minimize with
autostart — built and verified end-to-end on Windows, with the macOS backend
added as design/type-check-only code (mirrors the existing macOS backend spec's
treatment; not run on real hardware in this pass).

**Architecture:** One Tauri binary (`crates/gui/src-tauri`) replaces
`crates/daemon`. On startup it spawns two `UsbHotplugTrigger` instances (the
switch device and, independently, the MX Keys receiver) whose events are
forwarded into one `mpsc::Sender<DaemonEvent>`; a single consumer thread calls
`kvm_core::orchestrator::run`, so hardware-triggered switches and GUI-button
manual switches share one code path (and therefore one retry/fallback
behavior) and only one thing ever calls into the DDC write path. A React+TS
frontend (Vite, no Tauri backend logic) talks to this Rust core exclusively
through `#[tauri::command]`s and `listen()`-based events — it never touches
USB or DDC directly. Monitor/input discovery for the wizard uses a new,
purely-additive `MonitorReader` trait backed by `ddc-hi` (read-only,
`0x51`-source-address reads are fine per `DECISIONS.md` #4 — the source-addr
quirk only blocks *writes*); the validated NVAPI write path
(`NvapiBackend`/`writeValueToDisplay.exe`) is untouched.

**Tech Stack:** Rust workspace (unchanged: 1.89.0,
`x86_64-pc-windows-gnu`), Tauri 2 (`tray-icon` feature,
`tauri-plugin-single-instance`, `tauri-plugin-autostart`), `ddc-hi` 0.4
(read-only on Windows/macOS), React 18 + TypeScript + Vite (frontend, no
Tauri-side JS logic beyond `invoke`/`listen`), `serde_json` (config format,
replacing the `config` crate's `.ini` parsing).

## Global Constraints

- This plan **supersedes** the "Tauri UI is v2/optional, not coupled to the
  daemon" scoping decision in `DECISIONS.md` #6/#8, and supersedes Task 7 of
  `2026-07-06-kvm-switch-fork-mvp-implementation.md`'s "Tauri rejected as
  *foundation*" call — that rejection was about the daemon's headless core;
  here Tauri is the single process that *owns* a background thread running
  that same core, which does not reopen the original objection.
- **Platform scope for this plan:** Windows is built, run, and manually
  verified on real hardware (same as the existing MVP). macOS gets real code
  (mirroring `docs/superpowers/specs/2026-07-07-macos-backend-design.md`) that
  compiles via `cargo check --target aarch64-apple-darwin` only — this repo's
  dev environment has no macOS SDK, so macOS code is never linked, run, or
  hardware-tested here. Linux is explicitly **out of scope** — no
  `ddc-backend`/`trigger` Linux module exists yet, and CLAUDE.md's "hard
  requirement for every OS" wording is aspirational until a Linux backend
  exists; that is a separate future plan, not silently done here.
- **Known risk — GNU toolchain + Tauri.** This repo intentionally uses
  `x86_64-pc-windows-gnu` (`rust-toolchain.toml`, chosen in the original MVP
  plan specifically to avoid installing MSVC Build Tools). Tauri's Windows
  build/documentation/CI story is most commonly exercised against the MSVC
  toolchain; GNU is supported but less trodden. If `cargo tauri build`/`cargo
  tauri dev` fails to link on GNU in Task 6, the fallback is installing MSVC
  Build Tools and switching this one workspace member's toolchain — do not
  silently switch the *whole* workspace to MSVC without checking with the user
  first, since GNU-vs-MSVC was an explicit prior decision.
- **Config format changes from `.ini` to JSON** (this plan, Task 3) because
  the wizard — not a human — now owns writing it (decided during grilling);
  `.ini`'s upstream-compatibility rationale (`DECISIONS.md` §9) no longer
  applies once nothing hand-edits the file. `on_usb_connect_source_addr` and
  the renamed `display_index` become plain JSON numbers (no more hex-string
  parsing helpers — JSON has native numbers, `.ini` didn't).
- **Field rename bundled from the macOS spec:** `nvapi_display_index` →
  `display_index` (accessor already named this), and a new
  `on_usb_connect_vcp_code: Option<u8>` field (defaults to `0x60`,
  `INPUT_SELECT`) — both were already decided in
  `docs/superpowers/specs/2026-07-07-macos-backend-design.md` and are folded
  into this plan's Task 3 instead of being done twice.
- **MX Keys is a second, independent `TriggerSource`** (an unmodified second
  instance of `UsbHotplugTrigger`, pointed at the receiver's own VID:PID,
  reusing 100% of the existing `usb_hotplug` diff logic — no HID++ protocol
  work, which stays deferred per `DECISIONS.md` §6). **Its events update a
  `mxkeys_connected: bool` status shown in the GUI only — they do not feed
  `orchestrator::run`'s switch channel.** This avoids a duplicate/racing
  switch attempt when the receiver and the switch device change state at
  effectively the same moment (today's topology: the receiver lives on the
  switch, so they move together).
- **`writeValueToDisplay.exe` is unaffected.** Every new capability (monitor
  listing, input listing, manual switch) either reads via `ddc-hi` (new,
  additive `MonitorReader` trait) or writes via the existing, validated
  `DdcBackend::set_vcp`/`NvapiBackend` path. Nothing changes about how the
  actual hardware-validated switch is performed.

---

## Task 1: `ddc-backend` — capabilities-string parser (pure, TDD'd)

Standalone, OS-agnostic logic: parsing a DDC/CI capabilities reply string
(VESA MCCS format, e.g. `vcp(... 60(0F 11 12) ...)`) down to the list of VCP
input-select values a monitor advertises. This has no hardware dependency and
is fully unit-testable today.

**Files:**
- Create: `crates/ddc-backend/src/capabilities.rs`
- Modify: `crates/ddc-backend/src/lib.rs`

**Interfaces:**
- Produces: `pub fn parse_input_codes(capabilities: &str) -> Vec<u8>` (parses
  VCP feature `0x60`'s enumerated values out of a raw capabilities string).
- Produces: `pub struct MonitorInfo { pub display_index: u32, pub id: String, pub model_name: Option<String> }` (`Debug, Clone`)
- Produces: `pub trait MonitorReader { fn enumerate(&self) -> anyhow::Result<Vec<MonitorInfo>>; fn input_codes(&self, display_index: u32) -> anyhow::Result<Vec<u8>>; }`

- [ ] **Step 1: Write the failing tests**

Create `crates/ddc-backend/src/capabilities.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic fixture in VESA MCCS capabilities-string format. Not a real
    /// captured string from the LG 34GL750 (DECISIONS.md doesn't record one) —
    /// exercises the general `code(v1 v2 ...)` grammar this parser handles.
    const FIXTURE: &str = "(prot(monitor)type(lcd)model(34GL750)cmds(01 02 03 0C E3 F3)vcp(02 04 05 08 10 12 14(05 08 0B 0C) 16 18 1A 52 60(0F 11 12) AC AE B2 B6 C6 C8 C9 D6(01 04) DF)mswhql(1)mccs_ver(2.1))";

    #[test]
    fn extracts_enumerated_values_for_feature_0x60() {
        assert_eq!(parse_input_codes(FIXTURE), vec![0x0F, 0x11, 0x12]);
    }

    #[test]
    fn returns_empty_when_feature_0x60_is_not_enumerated() {
        let no_input_select = "(prot(monitor)type(lcd)vcp(02 04 05 08 10 12))";
        assert!(parse_input_codes(no_input_select).is_empty());
    }

    #[test]
    fn returns_empty_when_there_is_no_vcp_group() {
        assert!(parse_input_codes("(prot(monitor)type(lcd))").is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ddc-backend`
Expected: **compile error** — `parse_input_codes` is not defined yet.

- [ ] **Step 3: Implement `parse_input_codes`**

Add above the `#[cfg(test)]` block in `crates/ddc-backend/src/capabilities.rs`:
```rust
/// Parses VCP feature `0x60` (input select)'s enumerated allowed values out of
/// a raw DDC/CI capabilities reply string (VESA MCCS format:
/// `vcp(code1 code2 code3(v1 v2) code4 ...)`, where a code immediately
/// followed by `(...)` is an enumerated feature listing its allowed values).
/// Returns an empty `Vec` if the string has no `vcp(...)` group or feature
/// `0x60` isn't listed as enumerated.
pub fn parse_input_codes(capabilities: &str) -> Vec<u8> {
    const INPUT_SELECT: u8 = 0x60;
    let Some(vcp_start) = capabilities.find("vcp(") else {
        return Vec::new();
    };
    let rest = &capabilities[vcp_start + "vcp(".len()..];

    let mut depth: u32 = 1;
    let mut current_code: Option<u8> = None;
    let mut token = String::new();
    let mut result = Vec::new();

    let flush_value = |token: &str, current_code: Option<u8>, result: &mut Vec<u8>| {
        if current_code == Some(INPUT_SELECT) {
            if let Ok(v) = u8::from_str_radix(token.trim(), 16) {
                result.push(v);
            }
        }
    };

    for c in rest.chars() {
        match c {
            '(' => {
                depth += 1;
                current_code = u8::from_str_radix(token.trim(), 16).ok();
                token.clear();
            }
            ')' => {
                depth -= 1;
                flush_value(&token, current_code, &mut result);
                token.clear();
                if depth == 1 {
                    current_code = None;
                }
                if depth == 0 {
                    break;
                }
            }
            c if c.is_whitespace() => {
                flush_value(&token, current_code, &mut result);
                token.clear();
            }
            c => token.push(c),
        }
    }
    result
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ddc-backend`
Expected: all three tests in `capabilities::tests` `PASS`.

- [ ] **Step 5: Add the `MonitorReader` trait and `MonitorInfo`, wire the module**

Replace `crates/ddc-backend/src/lib.rs`:
```rust
//! Backends that write VCP feature values to a monitor over DDC/CI, plus a
//! separate read-only capability for enumerating monitors and their
//! supported inputs (used by the GUI's configuration wizard, never by the
//! orchestrator's write path).

use anyhow::Result;

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
}

pub mod capabilities;
pub use capabilities::parse_input_codes;

// TODO(macos): macos_ioavservice.rs — IOAVServiceReadI2C/WriteI2C backend,
// blocked on Spike #2 (see DECISIONS.md #5, #7). Implemented design/type-check
// only in this plan's Task 9.
// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9). Out of scope.

#[cfg(windows)]
pub mod windows_generic;
#[cfg(windows)]
pub mod windows_nvapi;
#[cfg(any(windows, target_os = "macos"))]
pub mod ddchi_reader;
```

- [ ] **Step 6: Build and test the whole crate**

Run: `cargo test -p ddc-backend`
Expected: builds clean (the new `#[cfg(any(windows, target_os = "macos"))] pub mod ddchi_reader;` line will fail until Task 2 creates that file — for this step only, comment that one line out, run the tests, then restore it immediately before committing so the crate is left buildable at every commit boundary is not required mid-task, but the final state before commit must build. If `cargo test -p ddc-backend` fails purely because `ddchi_reader` doesn't exist yet, that's expected and resolved in Task 2 — do not leave this crate broken across a commit boundary, so temporarily remove that one `pub mod` line for this commit and Task 2 re-adds it.)

- [ ] **Step 7: Commit**

```bash
git add crates/ddc-backend/src/capabilities.rs crates/ddc-backend/src/lib.rs
git commit -m "Add pure DDC capabilities-string parser and MonitorReader trait"
```

---

## Task 2: `ddc-backend` — `ddc-hi`-backed `MonitorReader` (Windows + macOS)

**Files:**
- Create: `crates/ddc-backend/src/ddchi_reader.rs`
- Modify: `crates/ddc-backend/Cargo.toml`

**Interfaces:**
- Consumes: `MonitorReader`/`MonitorInfo` (Task 1), `capabilities::parse_input_codes` (Task 1).
- Produces: `pub struct DdcHiMonitorReader;` implementing `MonitorReader`.

- [ ] **Step 1: Add the `ddc-hi` dependency**

Modify `crates/ddc-backend/Cargo.toml`, add below the existing `[dependencies]`:
```toml
[target.'cfg(any(windows, target_os = "macos"))'.dependencies]
ddc-hi = "0.4"
```

- [ ] **Step 2: Implement `DdcHiMonitorReader`**

There is no pure logic left to isolate here (it's a thin wrapper over
`ddc-hi`'s enumeration/read calls) — this is verified by `cargo build` here on
Windows, and later by `MANUAL_TEST_GUI.md` against real hardware, same pattern
as `crates/power-fallback`'s Windows implementation.

Create `crates/ddc-backend/src/ddchi_reader.rs`:
```rust
//! Read-only monitor/capability discovery via the published `ddc-hi` crate.
//! Works on both Windows and macOS (unlike the write path, which needs
//! OS-specific backends). `ddc-hi` hardcodes DDC/CI source address `0x51`,
//! which is fine for reads — the source-addr override quirk documented in
//! DECISIONS.md #4 only affects writes on this monitor.

use crate::capabilities::parse_input_codes;
use crate::{MonitorInfo, MonitorReader};
use anyhow::{anyhow, Result};
use ddc_hi::{Ddc, Display};

pub struct DdcHiMonitorReader;

impl MonitorReader for DdcHiMonitorReader {
    fn enumerate(&self) -> Result<Vec<MonitorInfo>> {
        Ok(Display::enumerate()
            .into_iter()
            .enumerate()
            .map(|(index, display)| MonitorInfo {
                display_index: index as u32,
                id: display.info.id.clone(),
                model_name: display.info.model_name.clone(),
            })
            .collect())
    }

    /// **Documented risk (grilled and accepted):** this index is `ddc-hi`'s
    /// own enumeration order, which is not guaranteed to match the NVAPI
    /// ordinal `NvapiBackend::set_vcp`'s `monitor_index` expects on a
    /// multi-GPU machine. No cross-verification step was built for this —
    /// the first real run against hardware (`MANUAL_TEST_GUI.md`) is the
    /// check. If they disagree, `Configuration::display_index` needs a
    /// manual override (already supported — it's a plain optional field).
    fn input_codes(&self, display_index: u32) -> Result<Vec<u8>> {
        let mut displays = Display::enumerate();
        let display = displays
            .get_mut(display_index as usize)
            .ok_or_else(|| anyhow!("no display at index {}", display_index))?;
        let raw = display
            .handle
            .capabilities_string()
            .map_err(|err| anyhow!("failed to read capabilities for display {}: {:?}", display_index, err))?;
        Ok(parse_input_codes(&String::from_utf8_lossy(&raw)))
    }
}
```

- [ ] **Step 3: Restore the `ddchi_reader` module line and build**

Confirm `crates/ddc-backend/src/lib.rs` still has (from Task 1, Step 5):
```rust
#[cfg(any(windows, target_os = "macos"))]
pub mod ddchi_reader;
```

Run: `cargo build -p ddc-backend`
Expected: builds clean on this Windows host (pulls in `ddc-hi` + its
`ddc-winapi` backend).

- [ ] **Step 4: Commit**

```bash
git add crates/ddc-backend/src/ddchi_reader.rs crates/ddc-backend/Cargo.toml crates/ddc-backend/src/lib.rs
git commit -m "Add ddc-hi-backed MonitorReader for the GUI wizard's read path"
```

---

## Task 3: `kvm_core::config` — migrate to JSON, add new fields

**Files:**
- Modify: `crates/kvm_core/src/config.rs`
- Modify: `crates/kvm_core/Cargo.toml`

**Interfaces:**
- Produces: `pub struct Configuration { pub usb_device: String, pub mxkeys_usb_device: Option<String>, pub on_usb_connect: Option<InputSource>, pub on_usb_disconnect: Option<InputSource>, pub on_usb_connect_source_addr: Option<u8>, pub on_usb_connect_vcp_code: Option<u8>, pub display_index: Option<u32> }` (`Debug, Clone, Serialize, Deserialize`)
- Produces: `impl Configuration { pub fn load(path: &Path) -> anyhow::Result<Self>; pub fn save(&self, path: &Path) -> anyhow::Result<()>; pub fn display_index(&self) -> u32; pub fn vcp_code(&self) -> u8 }`
- Produces: `pub enum InputSource` unchanged in shape (`Raw(u16)`/`Symbolic(SymbolicInputSource)`), now also `Serialize`.

This drops the `config` crate (env-overlay/multi-format abstraction is no
longer needed — the wizard is the only writer) in favor of `serde_json`
directly.

- [ ] **Step 1: Swap the `config` dependency for `serde_json`**

Modify `crates/kvm_core/Cargo.toml`, replace:
```toml
config = { version = "0.15", features = ["ini"], default-features = false }
```
with:
```toml
serde_json = "1.0"
```

- [ ] **Step 2: Rewrite the failing tests for the JSON schema**

Replace the `#[cfg(test)]` block at the bottom of `crates/kvm_core/src/config.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_device_is_lowercased() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17E9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.usb_device, "17e9:6000");
    }

    #[test]
    fn symbolic_input_source_resolves_to_vcp_value() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn hex_input_source_is_accepted() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "0x11"}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn source_addr_defaults_to_none() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, None);
    }

    #[test]
    fn source_addr_override_is_a_plain_number() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1", "on_usb_connect_source_addr": 80}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, Some(0x50));
    }

    #[test]
    fn display_index_defaults_to_zero() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.display_index(), 0);
    }

    #[test]
    fn vcp_code_defaults_to_input_select() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.vcp_code(), 0x60);
    }

    #[test]
    fn mxkeys_usb_device_defaults_to_none() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.mxkeys_usb_device, None);
    }

    #[test]
    fn round_trips_through_save_and_load() {
        let config = Configuration {
            usb_device: "17e9:6000".to_string(),
            mxkeys_usb_device: Some("046d:c52b".to_string()),
            on_usb_connect: Some(InputSource::Symbolic(SymbolicInputSource::Hdmi1)),
            on_usb_disconnect: None,
            on_usb_connect_source_addr: Some(0x50),
            on_usb_connect_vcp_code: None,
            display_index: Some(0),
        };
        let dir = std::env::temp_dir().join(format!("kvm-switch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        config.save(&path).unwrap();
        let loaded = Configuration::load(&path).unwrap();

        assert_eq!(loaded.usb_device, config.usb_device);
        assert_eq!(loaded.mxkeys_usb_device, config.mxkeys_usb_device);
        assert_eq!(loaded.on_usb_connect.unwrap().value(), 0x11);
        assert_eq!(loaded.on_usb_connect_source_addr, Some(0x50));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p kvm_core`
Expected: **compile error** — `Configuration`/`SymbolicInputSource` don't yet
have the new fields/derives, `save` doesn't exist, `Configuration::load` still
expects `.ini`.

- [ ] **Step 4: Rewrite `config.rs`'s implementation**

Replace everything above the `#[cfg(test)]` block in
`crates/kvm_core/src/config.rs`:
```rust
use anyhow::{Context, Result};
use paste::paste;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

        impl SymbolicInputSource {
            fn label(&self) -> &'static str {
                paste! {
                    match self {
                        $(Self::$name => stringify!([< $name:lower >]),)*
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
                .map_err(|_| serde::de::Error::custom(format!("Invalid input source: {}", str)))
        }
    }
}

impl Serialize for InputSource {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Symbolic(sym) => serializer.serialize_str(sym.label()),
            Self::Raw(value) => serializer.serialize_str(&format!("0x{value:x}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(deserialize_with = "Configuration::deserialize_usb_device")]
    pub usb_device: String,
    #[serde(default)]
    pub mxkeys_usb_device: Option<String>,
    pub on_usb_connect: Option<InputSource>,
    pub on_usb_disconnect: Option<InputSource>,
    #[serde(default)]
    pub on_usb_connect_source_addr: Option<u8>,
    /// VCP feature code for input select. Defaults to the DDC/CI standard
    /// `0x60` — see `vcp_code()`. Only macOS is expected to need an override
    /// (see `docs/superpowers/specs/2026-07-07-macos-backend-design.md`);
    /// Windows' validated recipe (DECISIONS.md #4) always uses `0x60`.
    #[serde(default)]
    pub on_usb_connect_vcp_code: Option<u8>,
    #[serde(default)]
    pub display_index: Option<u32>,
}

impl Configuration {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).with_context(|| format!("failed to read {:?}", path))?;
        serde_json::from_str(&contents).with_context(|| format!("failed to parse {:?} as JSON", path))
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let contents = serde_json::to_string_pretty(self).context("failed to serialize configuration")?;
        std::fs::write(path, contents).with_context(|| format!("failed to write {:?}", path))
    }

    fn deserialize_usb_device<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(s.to_lowercase())
    }

    pub fn display_index(&self) -> u32 {
        self.display_index.unwrap_or(0)
    }

    /// VCP feature code for input select — `0x60` (DDC/CI standard,
    /// `orchestrator::INPUT_SELECT`) unless overridden.
    pub fn vcp_code(&self) -> u8 {
        self.on_usb_connect_vcp_code.unwrap_or(0x60)
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p kvm_core`
Expected: all nine tests in `config::tests` `PASS`.

- [ ] **Step 6: Commit**

```bash
git add crates/kvm_core/src/config.rs crates/kvm_core/Cargo.toml
git commit -m "Migrate Configuration from INI to JSON; add mxkeys/vcp_code fields"
```

---

## Task 4: `kvm_core` — `vcp_code` in `SwitchTarget`, shared switch path, `DaemonEvent`

**Files:**
- Modify: `crates/kvm_core/src/monitor_map.rs`
- Modify: `crates/kvm_core/src/orchestrator.rs`

**Interfaces:**
- Consumes: `Configuration` (Task 3), `ddc_backend::DdcBackend`, `power_fallback::PowerFallback`, `trigger::TriggerEvent`.
- Produces: `pub struct SwitchTarget { pub display_index: u32, pub input_source: InputSource, pub source_addr: Option<u8>, pub vcp_code: u8 }`
- Produces: `pub fn handle_event(event: trigger::TriggerEvent, config: &Configuration, ddc_backend: &dyn DdcBackend, power_fallback: &dyn PowerFallback)` (unchanged signature)
- Produces: `pub fn handle_manual_switch(input: InputSource, config: &Configuration, ddc_backend: &dyn DdcBackend, power_fallback: &dyn PowerFallback)`
- Produces: `pub enum DaemonEvent { Trigger(trigger::TriggerEvent), ManualSwitch(config::InputSource) }` (`Debug, Clone`)
- Produces: `pub fn run(events: std::sync::mpsc::Receiver<DaemonEvent>, config: &Configuration, ddc_backend: &dyn DdcBackend, power_fallback: &dyn PowerFallback)`

- [ ] **Step 1: Update `monitor_map.rs`'s failing test for `vcp_code`**

Modify `crates/kvm_core/src/monitor_map.rs`'s test module — replace
`resolves_connect_target_from_config`:
```rust
    #[test]
    fn resolves_connect_target_from_config() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1", "on_usb_connect_source_addr": 80}"#,
        )
        .unwrap();
        let target = resolve(&config, SwitchDirection::Connect).unwrap();
        assert_eq!(target.display_index, 0);
        assert_eq!(target.input_source.value(), 0x11);
        assert_eq!(target.source_addr, Some(0x50));
        assert_eq!(target.vcp_code, 0x60);
    }
```
and `disconnect_with_no_config_resolves_to_none`:
```rust
    #[test]
    fn disconnect_with_no_config_resolves_to_none() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert!(resolve(&config, SwitchDirection::Disconnect).is_none());
    }
```
Remove the now-unused `fn load(...)` helper and its `use crate::config::Configuration;` stays (still needed for the type annotation above).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kvm_core`
Expected: **compile error** — `SwitchTarget` has no `vcp_code` field yet.

- [ ] **Step 3: Add `vcp_code` to `SwitchTarget` and `resolve`**

Replace the non-test portion of `crates/kvm_core/src/monitor_map.rs`:
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
    pub vcp_code: u8,
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
        vcp_code: config.vcp_code(),
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kvm_core`
Expected: both `monitor_map::tests` pass.

- [ ] **Step 5: Write the failing tests for `handle_manual_switch` and `run`**

Modify `crates/kvm_core/src/orchestrator.rs`'s test module — add after the
existing three tests (keep `FakeDdc`/`FakePower`/`load` as-is except `load`
now uses `serde_json`):
```rust
    fn load(config_str: &str) -> Configuration {
        serde_json::from_str(config_str).unwrap()
    }
```
(replaces the old `config::Config::builder()...` version), then add:
```rust
    #[test]
    fn manual_switch_calls_ddc_backend_with_given_input_and_configured_display() {
        let config = load(r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1", "on_usb_connect_source_addr": 80}"#);
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_manual_switch(InputSource::Symbolic(crate::config::SymbolicInputSource::DisplayPort1), &config, &ddc, &power);

        assert_eq!(*ddc.calls.borrow(), vec![(0, 0x60, 0x0f, Some(0x50))]);
    }

    #[test]
    fn run_processes_trigger_and_manual_events_through_the_same_handlers() {
        let config = load(r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#);
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(DaemonEvent::Trigger(TriggerEvent::HostGainedFocus)).unwrap();
        tx.send(DaemonEvent::ManualSwitch(InputSource::Symbolic(crate::config::SymbolicInputSource::Hdmi2))).unwrap();
        drop(tx);

        run(rx, &config, &ddc, &power);

        assert_eq!(
            *ddc.calls.borrow(),
            vec![(0, 0x60, 0x11, None), (0, 0x60, 0x12, None)]
        );
    }
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `cargo test -p kvm_core`
Expected: **compile error** — `handle_manual_switch`, `DaemonEvent`, `run` are
not defined yet.

- [ ] **Step 7: Implement, factoring out the shared switch-with-retry logic**

Replace the non-test portion of `crates/kvm_core/src/orchestrator.rs`:
```rust
use crate::config::{Configuration, InputSource};
use crate::monitor_map::{self, SwitchDirection, SwitchTarget};
use ddc_backend::DdcBackend;
use power_fallback::PowerFallback;
use trigger::TriggerEvent;

/// VCP feature code for input select (DDC/CI standard) — see DECISIONS.md #4.
pub const INPUT_SELECT: u8 = 0x60;

/// Events consumed by `run`'s single consumer loop. `Trigger` comes from a
/// background `TriggerSource` watcher; `ManualSwitch` comes from the GUI's
/// "switch now" button. Both funnel through the same handlers below, so only
/// one thing ever calls into the DDC write path.
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    Trigger(TriggerEvent),
    ManualSwitch(InputSource),
}

fn perform_switch(target: &SwitchTarget, ddc_backend: &dyn DdcBackend, power_fallback: &dyn PowerFallback) {
    let attempt = |ddc_backend: &dyn DdcBackend| {
        ddc_backend.set_vcp(target.display_index, target.vcp_code, target.input_source.value(), target.source_addr)
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
        log::info!("Display switched to {:?}", target.input_source);
    }
}

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
    perform_switch(&target, ddc_backend, power_fallback);
}

/// Switches directly to `input`, using the monitor's configured
/// `display_index`/`source_addr`/`vcp_code` but ignoring the
/// connect/disconnect mapping — used by the GUI's manual "switch now" button.
pub fn handle_manual_switch(
    input: InputSource,
    config: &Configuration,
    ddc_backend: &dyn DdcBackend,
    power_fallback: &dyn PowerFallback,
) {
    let target = SwitchTarget {
        display_index: config.display_index(),
        input_source: input,
        source_addr: config.on_usb_connect_source_addr,
        vcp_code: config.vcp_code(),
    };
    perform_switch(&target, ddc_backend, power_fallback);
}

/// The single consumer of `DaemonEvent`s. Runs until `events`'s sender is
/// dropped. Intended to run on its own background thread in the GUI binary
/// (Task 7) — everything that can trigger a switch sends into the channel
/// this reads from, so exactly one thread ever calls `DdcBackend::set_vcp`.
pub fn run(
    events: std::sync::mpsc::Receiver<DaemonEvent>,
    config: &Configuration,
    ddc_backend: &dyn DdcBackend,
    power_fallback: &dyn PowerFallback,
) {
    for event in events {
        match event {
            DaemonEvent::Trigger(trigger_event) => handle_event(trigger_event, config, ddc_backend, power_fallback),
            DaemonEvent::ManualSwitch(input) => handle_manual_switch(input, config, ddc_backend, power_fallback),
        }
    }
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p kvm_core`
Expected: all `orchestrator::tests` pass (5 total), plus every earlier
`kvm_core` test.

- [ ] **Step 9: Commit**

```bash
git add crates/kvm_core
git commit -m "Add vcp_code to SwitchTarget; unify trigger/manual switches behind orchestrator::run"
```

---

## Task 5: `trigger` — expose device listing for the wizard's plug-and-pick flow

**Files:**
- Modify: `crates/trigger/src/lib.rs`
- Modify: `crates/trigger/src/usb_hotplug.rs`

**Interfaces:**
- Produces: `pub fn list_usb_devices() -> anyhow::Result<Vec<String>>` (VID:PID strings, e.g. `"17e9:6000"`).

- [ ] **Step 1: Write the failing test**

The existing `read_device_list()` in `crates/trigger/src/usb_hotplug.rs` is
already the pure-ish primitive (it calls into `rusb`, so it can't be
hardware-mocked in a unit test — same category as `crates/power-fallback`'s
Windows code: no unit test, verified by build + manual test). What *is*
testable is that the new public wrapper returns a `Vec<String>` shape and
compiles against the existing private helper. Skip a unit test for this step
(consistent with the project's established pattern for `rusb`-touching code)
and go straight to the build-verified implementation.

- [ ] **Step 2: Expose `list_usb_devices`**

Modify `crates/trigger/src/usb_hotplug.rs` — change `fn read_device_list()`
to `pub fn read_device_list()`:
```rust
pub fn read_device_list() -> Result<HashSet<String>> {
    Ok(rusb::devices()?.iter().filter_map(|device| device_id(&device)).collect())
}
```

Modify `crates/trigger/src/lib.rs`, add after the `pub trait TriggerSource`
block:
```rust
/// Lists currently-connected USB devices as `"vvvv:pppp"` VID:PID strings, for
/// the GUI wizard's plug-and-pick device selection (Task 8's `list_usb_devices`
/// Tauri command). Reuses the exact same enumeration `UsbHotplugTrigger` polls
/// internally.
#[cfg(windows)]
pub fn list_usb_devices() -> anyhow::Result<Vec<String>> {
    Ok(usb_hotplug::read_device_list()?.into_iter().collect())
}
```

- [ ] **Step 3: Build the crate**

Run: `cargo build -p trigger`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git add crates/trigger
git commit -m "Expose list_usb_devices for the GUI wizard's plug-and-pick flow"
```

---

## Task 6: Retire `crates/daemon`, scaffold `crates/gui/src-tauri`

**Files:**
- Delete: `crates/daemon/`
- Create: `crates/gui/src-tauri/Cargo.toml`, `crates/gui/src-tauri/build.rs`, `crates/gui/src-tauri/tauri.conf.json`, `crates/gui/src-tauri/src/main.rs`
- Modify: `Cargo.toml` (workspace root)

**Interfaces:** none yet — `main.rs` is a placeholder Tauri app with no
window content (frontend arrives in Task 11); this task's job is only to get
`cargo tauri build`/`cargo tauri dev` running at all.

- [ ] **Step 1: Install the Tauri CLI**

Run:
```
cargo install tauri-cli --version "^2"
```
Expected: installs `cargo-tauri`, adding a `cargo tauri` subcommand.

- [ ] **Step 2: Remove the old headless daemon crate**

Run:
```bash
git rm -r crates/daemon
```

- [ ] **Step 3: Update the workspace manifest**

Replace `Cargo.toml` (root):
```toml
[workspace]
resolver = "2"
members = [
    "crates/kvm_core",
    "crates/trigger",
    "crates/ddc-backend",
    "crates/power-fallback",
    "crates/gui/src-tauri",
]
```

- [ ] **Step 4: Create `crates/gui/src-tauri/Cargo.toml`**

```toml
[package]
name = "kvm-switch-gui"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
anyhow = "1.0"
log = "0.4"
simplelog = "0.12"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-single-instance = "2"
tauri-plugin-autostart = "2"
kvm_core = { path = "../../kvm_core" }
trigger = { path = "../../trigger" }
ddc-backend = { path = "../../ddc-backend" }
power-fallback = { path = "../../power-fallback" }

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["wincon"] }
```

- [ ] **Step 5: Create `crates/gui/src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 6: Create `crates/gui/src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "kvm-switch-gui",
  "version": "0.1.0",
  "identifier": "dev.display-switch.kvm-switch-gui",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../frontend/dist"
  },
  "app": {
    "windows": [
      {
        "title": "KVM Switch",
        "width": 480,
        "height": 640,
        "visible": true
      }
    ]
  },
  "bundle": {
    "active": true,
    "icon": ["icons/icon.ico", "icons/icon.png"]
  }
}
```

Note: `icons/icon.ico`/`icons/icon.png` must exist under
`crates/gui/src-tauri/` before `cargo tauri build` succeeds — `cargo tauri
icon <path-to-a-source-png>` generates the full icon set from one source
image. This plan does not include producing that source image; add one
manually before running this task's build step (a plain placeholder square
PNG is enough to unblock the build).

- [ ] **Step 7: Create a placeholder `main.rs`**

```rust
fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 8: Build**

Run: `cargo tauri build --debug` (or `cargo tauri dev` for a faster inner
loop)
Expected: succeeds, produces an empty window. **If linking fails** on the
`x86_64-pc-windows-gnu` toolchain, see this plan's Global Constraints "Known
risk — GNU toolchain + Tauri" note before changing anything toolchain-related.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "Replace headless daemon crate with a Tauri app skeleton"
```

---

## Task 7: Wire the single-consumer background architecture

**Files:**
- Modify: `crates/gui/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `kvm_core::orchestrator::{run, DaemonEvent}` (Task 4), `kvm_core::config::Configuration` (Task 3), `trigger::{TriggerSource, usb_hotplug::UsbHotplugTrigger}` (existing + Task 5), `ddc_backend::windows_nvapi::NvapiBackend` (existing), `power_fallback::windows_monitorpower::WindowsMonitorPower` (existing).
- Produces: Tauri-managed state `AppState { events: std::sync::Mutex<std::sync::mpsc::Sender<kvm_core::orchestrator::DaemonEvent>>, mxkeys_status_item: std::sync::Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>> }`, registered via `app.manage(...)`. The `mxkeys_status_item` slot is filled in by Task 9's tray-menu setup and updated by this task's MX Keys forwarder thread — declared here so Task 9 doesn't need to redefine `AppState`.
- Produces: emits a Tauri event `"mxkeys-status"` (payload: `bool`) whenever MX Keys presence changes, for the frontend's main screen (Task 13); also updates the tray's status menu item text (Task 9) via the same state.

- [ ] **Step 1: Replace `main.rs` with the wiring**

```rust
use anyhow::{Context, Result};
use ddc_backend::windows_nvapi::NvapiBackend;
use kvm_core::config::Configuration;
use kvm_core::orchestrator::{self, DaemonEvent};
use power_fallback::windows_monitorpower::WindowsMonitorPower;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use trigger::usb_hotplug::UsbHotplugTrigger;
use trigger::{TriggerEvent, TriggerSource};

pub struct AppState {
    pub events: Mutex<Sender<DaemonEvent>>,
    /// Filled in by Task 9's tray setup once the menu is built; `None` until
    /// then. The MX Keys forwarder thread below updates its text whenever
    /// presence changes.
    pub mxkeys_status_item: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
}

fn config_path() -> std::path::PathBuf {
    std::path::PathBuf::from("kvm-switch-config.json")
}

fn init_logging() -> Result<()> {
    use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .context("failed to initialize logging")
}

fn default_exe_path() -> std::path::PathBuf {
    std::path::PathBuf::from("tools/writeValueToDisplay.exe")
}

fn main() {
    init_logging().expect("failed to initialize logging");

    let config = Configuration::load(&config_path()).ok();

    let (tx, rx) = std::sync::mpsc::channel::<DaemonEvent>();

    if let Some(config) = config.clone() {
        // Forward the switch device's hotplug events into the shared channel.
        let switch_tx = tx.clone();
        let switch_device = config.usb_device.clone();
        std::thread::spawn(move || {
            let trigger = UsbHotplugTrigger::new(&switch_device);
            for event in trigger.watch() {
                let _ = switch_tx.send(DaemonEvent::Trigger(event));
            }
        });

        // The single consumer: the only thing that ever calls into the DDC
        // write path. Runs for the life of the process.
        std::thread::spawn(move || {
            let ddc_backend = NvapiBackend::new(default_exe_path());
            let power_fallback = WindowsMonitorPower;
            orchestrator::run(rx, &config, &ddc_backend, &power_fallback);
        });
    } else {
        log::warn!("No configuration found at {:?} yet; switching is disabled until the setup wizard runs.", config_path());
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState {
            events: Mutex::new(tx),
            mxkeys_status_item: Mutex::new(None),
        })
        .setup(move |app| {
            if let Some(config) = config.clone() {
                if let Some(mxkeys_device) = config.mxkeys_usb_device.clone() {
                    let handle = app.handle().clone();
                    std::thread::spawn(move || {
                        let trigger = UsbHotplugTrigger::new(&mxkeys_device);
                        for event in trigger.watch() {
                            let connected = matches!(event, TriggerEvent::HostGainedFocus);
                            let _ = handle.emit("mxkeys-status", connected);
                            let state = handle.state::<AppState>();
                            if let Some(item) = state.mxkeys_status_item.lock().unwrap().as_ref() {
                                let text = if connected { "MX Keys: connected" } else { "MX Keys: not connected" };
                                let _ = item.set_text(text);
                            }
                        }
                    });
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p kvm-switch-gui`
Expected: builds clean.

- [ ] **Step 3: Commit**

```bash
git add crates/gui/src-tauri/src/main.rs
git commit -m "Wire single-consumer DaemonEvent channel: trigger threads -> orchestrator::run"
```

---

## Task 8: Tauri commands for the wizard and main screen

**Files:**
- Create: `crates/gui/src-tauri/src/commands.rs`
- Modify: `crates/gui/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `ddc_backend::{MonitorReader, MonitorInfo, ddchi_reader::DdcHiMonitorReader}` (Task 2), `trigger::list_usb_devices` (Task 5), `kvm_core::config::Configuration` (Task 3), `AppState` (Task 7).
- Produces (Tauri commands, invoked from the frontend as `invoke("name", {...})`):
  - `list_usb_devices() -> Result<Vec<String>, String>`
  - `list_monitors() -> Result<Vec<MonitorInfoDto>, String>`
  - `list_inputs(display_index: u32) -> Result<Vec<u8>, String>`
  - `save_config(config: Configuration) -> Result<(), String>`
  - `load_config() -> Result<Option<Configuration>, String>`
  - `switch_input(input_value: u16) -> Result<(), String>`

- [ ] **Step 1: Create `commands.rs`**

```rust
use ddc_backend::ddchi_reader::DdcHiMonitorReader;
use ddc_backend::{MonitorInfo, MonitorReader};
use kvm_core::config::{Configuration, InputSource};
use kvm_core::orchestrator::DaemonEvent;
use serde::Serialize;

use crate::{config_path, AppState};

/// `MonitorInfo` isn't `Serialize` (it lives in `ddc-backend`, which has no
/// reason to depend on `serde`) — this DTO is the frontend-facing shape.
#[derive(Serialize)]
pub struct MonitorInfoDto {
    pub display_index: u32,
    pub id: String,
    pub model_name: Option<String>,
}

impl From<MonitorInfo> for MonitorInfoDto {
    fn from(info: MonitorInfo) -> Self {
        Self {
            display_index: info.display_index,
            id: info.id,
            model_name: info.model_name,
        }
    }
}

#[tauri::command]
pub fn list_usb_devices() -> Result<Vec<String>, String> {
    trigger::list_usb_devices().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_monitors() -> Result<Vec<MonitorInfoDto>, String> {
    DdcHiMonitorReader
        .enumerate()
        .map(|monitors| monitors.into_iter().map(MonitorInfoDto::from).collect())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_inputs(display_index: u32) -> Result<Vec<u8>, String> {
    DdcHiMonitorReader.input_codes(display_index).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_config(config: Configuration) -> Result<(), String> {
    config.save(&config_path()).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn load_config() -> Result<Option<Configuration>, String> {
    match Configuration::load(&config_path()) {
        Ok(config) => Ok(Some(config)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub fn switch_input(input_value: u16, state: tauri::State<AppState>) -> Result<(), String> {
    let events = state.events.lock().map_err(|err| err.to_string())?;
    events
        .send(DaemonEvent::ManualSwitch(InputSource::Raw(input_value)))
        .map_err(|err| err.to_string())
}
```

- [ ] **Step 2: Wire the commands into `main.rs`**

Modify `crates/gui/src-tauri/src/main.rs`: add `mod commands;` near the top,
make `config_path` `pub(crate)` (so `commands.rs` can call it), and register
the handler on the `tauri::Builder`:
```rust
mod commands;

// change `fn config_path()` to:
pub(crate) fn config_path() -> std::path::PathBuf {
    std::path::PathBuf::from("kvm-switch-config.json")
}
```
and add `.invoke_handler(...)` to the builder chain, immediately before
`.run(tauri::generate_context!())`:
```rust
        .invoke_handler(tauri::generate_handler![
            commands::list_usb_devices,
            commands::list_monitors,
            commands::list_inputs,
            commands::save_config,
            commands::load_config,
            commands::switch_input,
        ])
```

- [ ] **Step 3: Build**

Run: `cargo build -p kvm-switch-gui`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git add crates/gui/src-tauri/src/commands.rs crates/gui/src-tauri/src/main.rs
git commit -m "Add Tauri commands for device/monitor listing, config, and manual switch"
```

---

## Task 9: Tray icon, close-to-tray, menu

Per the grilled tray requirements: **Open** (restore window), one
**quick-switch item per available input** (fires the same
`DaemonEvent::ManualSwitch` path as the main screen's buttons — Task 4's
`orchestrator::run` doesn't care which UI surface sent it), a **status line**
showing MX Keys presence, and **Quit**.

**Files:**
- Modify: `crates/gui/src-tauri/src/main.rs`

**Interfaces:** none new beyond `AppState.mxkeys_status_item` (already added
to `AppState`'s definition in Task 7).

- [ ] **Step 1: Add the tray to `main.rs`'s `.setup(...)` closure**

Modify the `.setup(move |app| { ... })` closure from Task 7 — add before its
final `Ok(())`, and note it now needs `tx` (the same `Sender<DaemonEvent>`
already stored in `AppState`, fetched back out via `app.state::<AppState>()`
rather than captured again, since `tx` was moved into `.manage(...)` earlier
in the chain):
```rust
            {
                use kvm_core::config::InputSource;
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;

                let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
                let status_i = MenuItem::with_id(app, "mxkeys-status", "MX Keys: unknown", false, None::<&str>)?;
                *app.state::<AppState>().mxkeys_status_item.lock().unwrap() = Some(status_i.clone());

                // All menu items are the same concrete `MenuItem<Wry>` type, so a
                // plain `Vec` (no trait objects) works for `Menu::with_items`.
                let mut items: Vec<MenuItem<tauri::Wry>> = vec![open_i, status_i];

                if let Some(config) = config.clone() {
                    if let Ok(codes) = ddc_backend::ddchi_reader::DdcHiMonitorReader.input_codes(config.display_index()) {
                        for code in codes {
                            let id = format!("switch:{code:#04x}");
                            let label = format!("Switch to {code:#04x}");
                            items.push(MenuItem::with_id(app, id, label, true, None::<&str>)?);
                        }
                    }
                }

                items.push(MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?);

                let item_refs: Vec<&MenuItem<tauri::Wry>> = items.iter().collect();
                let menu = Menu::with_items(app, &item_refs)?;

                TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, event| {
                        let id = event.id.as_ref();
                        match id {
                            "open" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.unminimize();
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "quit" => app.exit(0),
                            id if id.starts_with("switch:") => {
                                if let Ok(value) = u16::from_str_radix(id.trim_start_matches("switch:0x"), 16) {
                                    let state = app.state::<AppState>();
                                    let events = state.events.lock().unwrap();
                                    let _ = events.send(kvm_core::orchestrator::DaemonEvent::ManualSwitch(InputSource::Raw(value)));
                                }
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;
            }
```

- [ ] **Step 2: Enable the `tray-icon` feature and close-to-tray behavior**

Confirm `crates/gui/src-tauri/Cargo.toml`'s `tauri` dependency already has
`features = ["tray-icon"]` (added in Task 6, Step 4).

Add an `.on_window_event(...)` handler to the `tauri::Builder` chain in
`main.rs`, before `.run(...)`:
```rust
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
```

- [ ] **Step 3: Build**

Run: `cargo build -p kvm-switch-gui`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git add crates/gui/src-tauri/src/main.rs
git commit -m "Add tray icon (Open/Quit) and close-to-tray window behavior"
```

---

## Task 10: macOS backend (design/type-check only)

Per this plan's Global Constraints: written and `cargo check`'d against the
real crate APIs, never linked/run/hardware-tested in this environment. Follows
`docs/superpowers/specs/2026-07-07-macos-backend-design.md` almost verbatim,
adapted only where the single-consumer-channel architecture (Task 4/7)
replaced that spec's older CLI-loop wiring.

**Files:**
- Create: `crates/ddc-backend/src/macos_ioavservice.rs`
- Create: `crates/trigger/src/macos_hotplug.rs`
- Create: `crates/power-fallback/src/macos_pmset.rs`
- Modify: `crates/ddc-backend/src/lib.rs`, `crates/trigger/src/lib.rs`, `crates/power-fallback/src/lib.rs`, `crates/gui/src-tauri/src/main.rs`, `crates/ddc-backend/Cargo.toml`

**Interfaces:**
- Produces: `pub struct MacosIoavserviceBackend;` implementing `DdcBackend`.
- Produces: `pub struct MacosHotplugTrigger;` implementing `TriggerSource`.
- Produces: `pub struct MacosPmset;` implementing `PowerFallback`.

- [ ] **Step 1: Add macOS DDC dependencies**

Modify `crates/ddc-backend/Cargo.toml`:
```toml
[target.'cfg(target_os = "macos")'.dependencies]
ddc-macos = "0.2.2"
```
(`ddc-hi` for macOS is already present from Task 2's `cfg(any(windows,
target_os = "macos"))` line.)

- [ ] **Step 2: Create `crates/ddc-backend/src/macos_ioavservice.rs`**

```rust
use crate::DdcBackend;
use anyhow::{anyhow, Result};
use ddc_hi::{Ddc, Display};

pub struct MacosIoavserviceBackend;

impl DdcBackend for MacosIoavserviceBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        if source_addr.is_some() {
            log::warn!(
                "source_addr override ({:?}) requested but unsupported on macOS (ddc-hi hardcodes 0x51); ignoring",
                source_addr
            );
        }
        let mut displays = Display::enumerate();
        let display = displays
            .get_mut(monitor_index as usize)
            .ok_or_else(|| anyhow!("no display at index {}", monitor_index))?;
        display
            .handle
            .set_vcp_feature(code, value)
            .map_err(|err| anyhow!("failed to set VCP {:#04x}={:#06x}: {:?}", code, value, err))
    }
}
```

- [ ] **Step 3: Wire it into `ddc-backend/src/lib.rs`**

Replace the `// TODO(macos): ...` line from Task 1 with:
```rust
#[cfg(target_os = "macos")]
pub mod macos_ioavservice;
```

- [ ] **Step 4: Create `crates/trigger/src/macos_hotplug.rs`**

```rust
use crate::TriggerEvent;
use anyhow::{anyhow, Result};
use rusb::{Context, Device, HotplugBuilder, Registration, UsbContext};
use std::sync::mpsc::{self, Sender};

fn device_id<T: UsbContext>(device: &Device<T>) -> Option<String> {
    device
        .device_descriptor()
        .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
        .ok()
}

pub struct MacosHotplugTrigger {
    usb_device: String,
}

impl MacosHotplugTrigger {
    pub fn new(usb_device: &str) -> Self {
        Self {
            usb_device: usb_device.to_lowercase(),
        }
    }
}

impl crate::TriggerSource for MacosHotplugTrigger {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent> {
        let (tx, rx) = mpsc::channel();
        let usb_device = self.usb_device.clone();
        std::thread::spawn(move || {
            if let Err(err) = run_hotplug_loop(usb_device, tx) {
                log::error!("USB hotplug detection failed: {:?}", err);
            }
        });
        rx
    }
}

struct HotplugHandler {
    usb_device: String,
    sender: Sender<TriggerEvent>,
}

impl<T: UsbContext> rusb::Hotplug<T> for HotplugHandler {
    fn device_arrived(&mut self, device: Device<T>) {
        if device_id(&device).as_deref() == Some(self.usb_device.as_str()) {
            let _ = self.sender.send(TriggerEvent::HostGainedFocus);
        }
    }
    fn device_left(&mut self, device: Device<T>) {
        if device_id(&device).as_deref() == Some(self.usb_device.as_str()) {
            let _ = self.sender.send(TriggerEvent::HostLostFocus);
        }
    }
}

fn run_hotplug_loop(usb_device: String, sender: Sender<TriggerEvent>) -> Result<()> {
    if !rusb::has_hotplug() {
        return Err(anyhow!("libusb hotplug api unsupported on this platform"));
    }
    let context = Context::new()?;
    let handler = HotplugHandler { usb_device, sender };
    let _registration: Registration<Context> =
        HotplugBuilder::new().enumerate(true).register(&context, Box::new(handler))?;
    loop {
        context.handle_events(None)?;
    }
}
```

- [ ] **Step 5: Wire it into `trigger/src/lib.rs`**

Add below the existing `#[cfg(windows)] pub mod usb_hotplug;` line:
```rust
#[cfg(target_os = "macos")]
pub mod macos_hotplug;
```
Also add a macOS-gated `list_usb_devices` (mirrors Task 5's Windows version,
reusing `rusb::devices()` directly since macOS has no `usb_hotplug` module to
borrow `read_device_list` from):
```rust
#[cfg(target_os = "macos")]
pub fn list_usb_devices() -> anyhow::Result<Vec<String>> {
    Ok(rusb::devices()?
        .iter()
        .filter_map(|device| {
            device
                .device_descriptor()
                .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
                .ok()
        })
        .collect())
}
```

- [ ] **Step 6: Create `crates/power-fallback/src/macos_pmset.rs`**

```rust
use crate::PowerFallback;
use anyhow::{anyhow, Result};
use std::process::Command;
use std::{thread, time};

pub struct MacosPmset;

impl PowerFallback for MacosPmset {
    fn blank_and_restore(&self) -> Result<()> {
        let status = Command::new("/usr/bin/pmset").args(["displaysleepnow"]).status()?;
        if !status.success() {
            return Err(anyhow!("pmset displaysleepnow exited with {:?}", status.code()));
        }
        thread::sleep(time::Duration::from_millis(500));
        let status = Command::new("/usr/bin/caffeinate").args(["-u", "-t", "1"]).status()?;
        if !status.success() {
            return Err(anyhow!("caffeinate wake exited with {:?}", status.code()));
        }
        Ok(())
    }
}
```

- [ ] **Step 7: Wire it into `power-fallback/src/lib.rs`**

Replace the `// TODO(macos): ...` line with:
```rust
#[cfg(target_os = "macos")]
pub mod macos_pmset;
```

- [ ] **Step 8: Add macOS wiring to the GUI's `main.rs`**

Modify `crates/gui/src-tauri/src/main.rs`: gate the existing switch-device
forwarder thread and the consumer thread (both currently hardcoded to
`NvapiBackend`/`WindowsMonitorPower`/`UsbHotplugTrigger`) behind
`#[cfg(windows)]`/`#[cfg(target_os = "macos")]` twins. Extract the
platform-specific pieces into two small functions:
```rust
#[cfg(windows)]
fn spawn_switch_trigger(usb_device: String, tx: Sender<DaemonEvent>) {
    std::thread::spawn(move || {
        let trigger = UsbHotplugTrigger::new(&usb_device);
        for event in trigger.watch() {
            let _ = tx.send(DaemonEvent::Trigger(event));
        }
    });
}

#[cfg(target_os = "macos")]
fn spawn_switch_trigger(usb_device: String, tx: Sender<DaemonEvent>) {
    use trigger::macos_hotplug::MacosHotplugTrigger;
    std::thread::spawn(move || {
        let trigger = MacosHotplugTrigger::new(&usb_device);
        for event in trigger.watch() {
            let _ = tx.send(DaemonEvent::Trigger(event));
        }
    });
}

#[cfg(windows)]
fn spawn_consumer(rx: std::sync::mpsc::Receiver<DaemonEvent>, config: Configuration) {
    std::thread::spawn(move || {
        let ddc_backend = NvapiBackend::new(default_exe_path());
        let power_fallback = WindowsMonitorPower;
        orchestrator::run(rx, &config, &ddc_backend, &power_fallback);
    });
}

#[cfg(target_os = "macos")]
fn spawn_consumer(rx: std::sync::mpsc::Receiver<DaemonEvent>, config: Configuration) {
    use ddc_backend::macos_ioavservice::MacosIoavserviceBackend;
    use power_fallback::macos_pmset::MacosPmset;
    std::thread::spawn(move || {
        let ddc_backend = MacosIoavserviceBackend;
        let power_fallback = MacosPmset;
        orchestrator::run(rx, &config, &ddc_backend, &power_fallback);
    });
}
```
and replace the corresponding inline blocks in `main()`'s `if let Some(config)
= config.clone() { ... }` with calls to `spawn_switch_trigger(config.usb_device.clone(),
switch_tx)` / `spawn_consumer(rx, config)`. Likewise, the MX Keys forwarder
thread in `.setup(...)` should use `UsbHotplugTrigger`/`MacosHotplugTrigger`
behind the same two `#[cfg(...)]` gates (extract a third
`spawn_mxkeys_trigger` function following the identical pattern).

- [ ] **Step 9: Type-check for macOS**

Run:
```
rustup target add aarch64-apple-darwin
cargo check --target aarch64-apple-darwin -p ddc-backend -p trigger -p power-fallback -p kvm-switch-gui
```
Expected: type-checks clean (no linking — this only proves the code compiles
against the real `ddc-hi`/`ddc-macos`/`rusb` APIs, not that it runs). If this
fails on a macOS-only symbol this repo's toolchain can't resolve without
Apple's SDK, that is an accepted limitation of developing macOS code from
Windows — note it and move on; it does not block this task's commit as long
as the failure is specifically an SDK/link-time issue, not a type error.

- [ ] **Step 10: Commit**

```bash
git add crates/ddc-backend crates/trigger crates/power-fallback crates/gui/src-tauri/src/main.rs
git commit -m "Add macOS backend (design/type-check only, not run in this environment)"
```

---

## Task 11: React + TypeScript + Vite frontend scaffold

**Files:**
- Create: `crates/gui/frontend/package.json`, `crates/gui/frontend/vite.config.ts`, `crates/gui/frontend/tsconfig.json`, `crates/gui/frontend/index.html`, `crates/gui/frontend/src/main.tsx`, `crates/gui/frontend/src/App.tsx`, `crates/gui/frontend/src/api.ts`

**Interfaces:**
- Produces: `api.ts` wraps every Tauri command from Task 8 in a typed
  function (`listUsbDevices`, `listMonitors`, `listInputs`, `saveConfig`,
  `loadConfig`, `switchInput`), so wizard/main-screen components (Tasks 12-13)
  never call `invoke` directly.

- [ ] **Step 1: `package.json`**

```json
{
  "name": "kvm-switch-gui-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "@tauri-apps/api": "^2.0.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.3",
    "vite": "^5.4.0"
  }
}
```

- [ ] **Step 2: `vite.config.ts`**

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
```

- [ ] **Step 3: `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true
  },
  "include": ["src"]
}
```

- [ ] **Step 4: `index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>KVM Switch</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: `src/api.ts` — typed wrappers for every Task 8 command**

```typescript
import { invoke } from "@tauri-apps/api/core";

export interface MonitorInfo {
  display_index: number;
  id: string;
  model_name: string | null;
}

export interface Configuration {
  usb_device: string;
  mxkeys_usb_device: string | null;
  on_usb_connect: string | null;
  on_usb_disconnect: string | null;
  on_usb_connect_source_addr: number | null;
  on_usb_connect_vcp_code: number | null;
  display_index: number | null;
}

export const listUsbDevices = () => invoke<string[]>("list_usb_devices");
export const listMonitors = () => invoke<MonitorInfo[]>("list_monitors");
export const listInputs = (displayIndex: number) => invoke<number[]>("list_inputs", { displayIndex });
export const saveConfig = (config: Configuration) => invoke<void>("save_config", { config });
export const loadConfig = () => invoke<Configuration | null>("load_config");
export const switchInput = (inputValue: number) => invoke<void>("switch_input", { inputValue });
```

- [ ] **Step 6: `src/main.tsx`**

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 7: `src/App.tsx` — routes between the wizard and the main screen**

```typescript
import { useEffect, useState } from "react";
import { loadConfig, Configuration } from "./api";
import { Wizard } from "./wizard/Wizard";
import { MainScreen } from "./MainScreen";

export default function App() {
  const [config, setConfig] = useState<Configuration | null | "loading">("loading");

  useEffect(() => {
    loadConfig().then(setConfig);
  }, []);

  if (config === "loading") {
    return <p>Loading…</p>;
  }
  if (config === null) {
    return <Wizard onComplete={setConfig} />;
  }
  return <MainScreen config={config} onReconfigure={() => setConfig(null)} />;
}
```

- [ ] **Step 8: Install and build**

Run:
```
cd crates/gui/frontend
npm install
npm run build
```
Expected: `npm run build` succeeds (this will fail until Tasks 12-13 create
`./wizard/Wizard.tsx` and `./MainScreen.tsx` — for this step only, temporarily
stub both as `export function Wizard() { return null; }` /
`export function MainScreen() { return null; }` so this task's build step is
verifiable in isolation; Tasks 12-13 replace the stubs).

- [ ] **Step 9: Commit**

```bash
git add crates/gui/frontend
git commit -m "Scaffold React+TypeScript+Vite frontend with typed Tauri command wrappers"
```

---

## Task 12: Configuration wizard

**Files:**
- Create: `crates/gui/frontend/src/wizard/Wizard.tsx`, `crates/gui/frontend/src/wizard/DeviceStep.tsx`, `crates/gui/frontend/src/wizard/MonitorStep.tsx`, `crates/gui/frontend/src/wizard/InputMappingStep.tsx`

**Interfaces:**
- Consumes: `api.ts` (Task 11).
- Produces: `Wizard({ onComplete: (config: Configuration) => void })` — three
  linear steps (device picker → monitor picker → input mapping), calling
  `saveConfig` on completion.

- [ ] **Step 1: `DeviceStep.tsx` — plug-and-pick for the switch and MX Keys devices**

```typescript
import { useState } from "react";
import { listUsbDevices } from "../api";

interface Props {
  label: string;
  onSelected: (deviceId: string) => void;
}

/** "Plug it in, click the one that appeared": snapshots the USB device list,
 * asks the user to plug in the device, re-snapshots, and highlights whatever
 * is new. */
export function DeviceStep({ label, onSelected }: Props) {
  const [before, setBefore] = useState<string[] | null>(null);
  const [candidates, setCandidates] = useState<string[]>([]);

  const snapshotBefore = async () => setBefore(await listUsbDevices());

  const detectNew = async () => {
    const after = await listUsbDevices();
    const beforeSet = new Set(before ?? []);
    setCandidates(after.filter((id) => !beforeSet.has(id)));
  };

  return (
    <div>
      <h2>{label}</h2>
      {before === null && <button onClick={snapshotBefore}>Start</button>}
      {before !== null && candidates.length === 0 && (
        <div>
          <p>Now plug in the device (or unplug/replug it).</p>
          <button onClick={detectNew}>I plugged it in</button>
        </div>
      )}
      {candidates.length > 0 && (
        <ul>
          {candidates.map((id) => (
            <li key={id}>
              {id} <button onClick={() => onSelected(id)}>Use this</button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

- [ ] **Step 2: `MonitorStep.tsx` — list detected monitors, pick one**

```typescript
import { useEffect, useState } from "react";
import { listMonitors, MonitorInfo } from "../api";

interface Props {
  onSelected: (monitor: MonitorInfo) => void;
}

export function MonitorStep({ onSelected }: Props) {
  const [monitors, setMonitors] = useState<MonitorInfo[] | null>(null);

  useEffect(() => {
    listMonitors().then(setMonitors);
  }, []);

  if (monitors === null) return <p>Detecting monitors…</p>;
  if (monitors.length === 0) return <p>No DDC-compatible monitors detected.</p>;

  return (
    <div>
      <h2>Select the monitor this KVM setup controls</h2>
      <ul>
        {monitors.map((m) => (
          <li key={m.display_index}>
            {m.model_name ?? m.id} (display index {m.display_index})
            <button onClick={() => onSelected(m)}>Use this monitor</button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

- [ ] **Step 3: `InputMappingStep.tsx` — list live inputs, map connect/disconnect**

```typescript
import { useEffect, useState } from "react";
import { listInputs } from "../api";

interface Props {
  displayIndex: number;
  onComplete: (mapping: { onConnect: number; onDisconnect: number | null }) => void;
}

export function InputMappingStep({ displayIndex, onComplete }: Props) {
  const [inputs, setInputs] = useState<number[] | null>(null);
  const [onConnect, setOnConnect] = useState<number | null>(null);
  const [onDisconnect, setOnDisconnect] = useState<number | null>(null);

  useEffect(() => {
    listInputs(displayIndex).then(setInputs);
  }, [displayIndex]);

  if (inputs === null) return <p>Reading supported inputs…</p>;

  const hex = (v: number) => `0x${v.toString(16).toUpperCase()}`;

  return (
    <div>
      <h2>Map inputs</h2>
      <label>
        Switch to this input when the KVM switch connects to this host:
        <select onChange={(e) => setOnConnect(Number(e.target.value))}>
          <option value="">Select…</option>
          {inputs.map((v) => (
            <option key={v} value={v}>
              {hex(v)}
            </option>
          ))}
        </select>
      </label>
      <label>
        Switch to this input on disconnect (optional):
        <select onChange={(e) => setOnDisconnect(e.target.value ? Number(e.target.value) : null)}>
          <option value="">None</option>
          {inputs.map((v) => (
            <option key={v} value={v}>
              {hex(v)}
            </option>
          ))}
        </select>
      </label>
      <button disabled={onConnect === null} onClick={() => onComplete({ onConnect: onConnect!, onDisconnect })}>
        Finish
      </button>
    </div>
  );
}
```

- [ ] **Step 4: `Wizard.tsx` — ties the three steps together and saves**

```typescript
import { useState } from "react";
import { Configuration, MonitorInfo, saveConfig } from "../api";
import { DeviceStep } from "./DeviceStep";
import { MonitorStep } from "./MonitorStep";
import { InputMappingStep } from "./InputMappingStep";

type Step =
  | { name: "switch-device" }
  | { name: "mxkeys-device"; switchDevice: string }
  | { name: "monitor"; switchDevice: string; mxkeysDevice: string }
  | { name: "inputs"; switchDevice: string; mxkeysDevice: string; monitor: MonitorInfo };

export function Wizard({ onComplete }: { onComplete: (config: Configuration) => void }) {
  const [step, setStep] = useState<Step>({ name: "switch-device" });

  if (step.name === "switch-device") {
    return <DeviceStep label="Select the KVM switch USB device" onSelected={(id) => setStep({ name: "mxkeys-device", switchDevice: id })} />;
  }
  if (step.name === "mxkeys-device") {
    return (
      <DeviceStep
        label="Select the MX Keys receiver (optional — plug it in, or skip)"
        onSelected={(id) => setStep({ name: "monitor", switchDevice: step.switchDevice, mxkeysDevice: id })}
      />
    );
  }
  if (step.name === "monitor") {
    return (
      <MonitorStep
        onSelected={(monitor) =>
          setStep({ name: "inputs", switchDevice: step.switchDevice, mxkeysDevice: step.mxkeysDevice, monitor })
        }
      />
    );
  }

  return (
    <InputMappingStep
      displayIndex={step.monitor.display_index}
      onComplete={async ({ onConnect, onDisconnect }) => {
        const config: Configuration = {
          usb_device: step.switchDevice,
          mxkeys_usb_device: step.mxkeysDevice || null,
          on_usb_connect: `0x${onConnect.toString(16)}`,
          on_usb_disconnect: onDisconnect !== null ? `0x${onDisconnect.toString(16)}` : null,
          on_usb_connect_source_addr: null,
          on_usb_connect_vcp_code: null,
          display_index: step.monitor.display_index,
        };
        await saveConfig(config);
        onComplete(config);
      }}
    />
  );
}
```

- [ ] **Step 5: Remove the Task 11 stub, build**

Delete the temporary `export function Wizard() { return null; }` stub from
Task 11, Step 8 (it's now the real file above).

Run:
```
cd crates/gui/frontend
npm run build
```
Expected: succeeds (assuming `MainScreen.tsx`'s stub from Task 11 is still in
place — Task 13 replaces it).

- [ ] **Step 6: Commit**

```bash
git add crates/gui/frontend/src/wizard
git commit -m "Add configuration wizard: device pick, monitor pick, input mapping"
```

---

## Task 13: Main screen — status, manual switch, MX Keys indicator

**Files:**
- Create: `crates/gui/frontend/src/MainScreen.tsx`

**Interfaces:**
- Consumes: `api.ts` (Task 11), Tauri event `"mxkeys-status"` (Task 7).
- Produces: `MainScreen({ config: Configuration, onReconfigure: () => void })`.

- [ ] **Step 1: `MainScreen.tsx`**

```typescript
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Configuration, listInputs, switchInput } from "./api";

export function MainScreen({ config, onReconfigure }: { config: Configuration; onReconfigure: () => void }) {
  const [inputs, setInputs] = useState<number[]>([]);
  const [mxkeysConnected, setMxkeysConnected] = useState<boolean | null>(null);

  useEffect(() => {
    listInputs(config.display_index ?? 0).then(setInputs);
  }, [config.display_index]);

  useEffect(() => {
    const unlisten = listen<boolean>("mxkeys-status", (event) => setMxkeysConnected(event.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const hex = (v: number) => `0x${v.toString(16).toUpperCase()}`;

  return (
    <div>
      <h1>KVM Switch</h1>
      <p>
        MX Keys receiver:{" "}
        {mxkeysConnected === null ? "unknown" : mxkeysConnected ? "connected on this host" : "not connected"}
      </p>
      <h2>Switch input</h2>
      <ul>
        {inputs.map((v) => (
          <li key={v}>
            {hex(v)} <button onClick={() => switchInput(v)}>Switch</button>
          </li>
        ))}
      </ul>
      <button onClick={onReconfigure}>Reconfigure</button>
    </div>
  );
}
```

- [ ] **Step 2: Remove the Task 11 stub, build**

Delete the temporary `export function MainScreen() { return null; }` stub
from Task 11, Step 8.

Run:
```
cd crates/gui/frontend
npm run build
```
Expected: succeeds cleanly — this is the frontend's first full build with
both real screens in place.

- [ ] **Step 3: Full workspace build**

Run:
```
cargo tauri build --debug
```
Expected: succeeds — Rust workspace + bundled frontend, end to end.

- [ ] **Step 4: Commit**

```bash
git add crates/gui/frontend/src/MainScreen.tsx
git commit -m "Add main screen: MX Keys status, manual input switching"
```

---

## Task 14: Docs — Makefile, CLAUDE.md, manual test instructions

**Files:**
- Modify: `Makefile`, `CLAUDE.md`
- Create: `MANUAL_TEST_GUI.md`

**Interfaces:** none (documentation only).

- [ ] **Step 1: Update `Makefile`'s build targets**

Read the current `Makefile` first (its exact non-macOS branch wraps `cargo
build`/`cargo build --release`) and replace those two commands with `cargo
tauri build --debug` / `cargo tauri build`, keeping the file's existing
macOS-universal-binary branch structure untouched (this task only changes
which command builds the non-macOS/default path, since Windows is this plan's
only real target).

- [ ] **Step 2: Update `CLAUDE.md`'s "Development Commands" and "Key Dependencies" sections**

Modify the `### Building` code block to:
```bash
# Install the Tauri CLI once
cargo install tauri-cli --version "^2"

# Debug build/run (opens the GUI)
cargo tauri dev

# Release build
cargo tauri build
```
Modify `### Running` to:
```bash
# Launch the GUI directly (after cargo tauri build)
./target/release/kvm-switch-gui
```
Add a line to `## Key Dependencies`: `- **Tauri** — cross-platform GUI shell
(system tray, single-instance, autostart plugins); frontend is React +
TypeScript, built via Vite (`crates/gui/frontend`)`.

- [ ] **Step 3: Write `MANUAL_TEST_GUI.md`**

```markdown
# Manual Test: GUI wizard, manual switching, tray, MX Keys status

Run this after `cargo tauri build --debug` succeeds, on the real hardware
described in `DECISIONS.md` (LG 34GL750, NVIDIA GPU, USB switch
17e9:6000, MX Keys with Unifying receiver).

## Setup

1. Delete any existing `kvm-switch-config.json` in the repo root to force the
   wizard on first launch.
2. Confirm `tools/writeValueToDisplay.exe` exists.

## Wizard flow

1. Launch `cargo tauri dev` (or the built binary).
2. **Switch device step:** click Start, then physically toggle the USB
   switch (or unplug/replug it) so it disappears and reappears; click "I
   plugged it in"; confirm exactly one new device ID appears and select it.
3. **MX Keys step:** repeat with the Unifying receiver plugged into any USB
   port on this host.
4. **Monitor step:** confirm the LG 34GL750 appears in the list (by model
   name or EDID id); select it.
5. **Input mapping step:** confirm the listed inputs include `0xF`, `0x11`,
   `0x12` (DisplayPort1/HDMI1/HDMI2 — see DECISIONS.md §2); set "on connect"
   to `0x11` (HDMI1, the Mac's input); leave disconnect unset; click Finish.
6. Confirm `kvm-switch-config.json` now exists and contains the selected
   `usb_device`, `mxkeys_usb_device`, `on_usb_connect`, `display_index`.

## Main screen

1. Confirm the main screen loads (not the wizard) on a second launch.
2. Confirm the MX Keys status line reflects reality: unplug the receiver,
   confirm it flips to "not connected" within a few seconds; replug it,
   confirm it flips back.
3. Click "Switch" next to `0x11`; confirm the monitor switches to HDMI1.
4. Click "Switch" next to `0xF`; confirm the monitor switches back to
   DisplayPort1.
5. Physically toggle the USB switch; confirm the monitor still switches via
   the hardware trigger path (not just the manual button) — this is the
   regression check that Task 4's shared `perform_switch`/`orchestrator::run`
   didn't break the original MVP behavior.

## Tray and autostart

1. Close the window (X button); confirm the process keeps running (check
   Task Manager) and the tray icon remains.
2. Left-click the tray icon; confirm the window restores.
3. Right-click the tray icon; confirm "Open"/"Quit" appear and both work.
4. Confirm an autostart entry was created (Windows: `Startup` folder or
   `HKCU\...\Run`, depending on how `tauri-plugin-autostart` implements it on
   this OS) after first launch.
5. Quit via the tray menu; confirm the process actually exits (not just
   hidden).

## Known non-goals for this milestone

- No automated test covers this end-to-end flow — it requires physically
  toggling the USB switch/receiver and observing the monitor and tray, same
  limitation as `MANUAL_TEST.md`.
- macOS is not tested here at all (see this plan's Global Constraints).
```

- [ ] **Step 4: Commit**

```bash
git add Makefile CLAUDE.md MANUAL_TEST_GUI.md
git commit -m "Update build docs for the Tauri GUI; add MANUAL_TEST_GUI.md"
```

- [ ] **Step 5: Run the actual manual test**

Follow `MANUAL_TEST_GUI.md` end to end with the real hardware. This is the
milestone's true acceptance test — do not consider the milestone done until
this passes.
