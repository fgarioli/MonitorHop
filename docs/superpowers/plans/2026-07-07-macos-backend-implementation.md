# macOS Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a working macOS backend (trigger + ddc-backend + power-fallback +
daemon wiring) to the kvm-switch-fork daemon, and parameterize the VCP code
(not just value and source-addr) across all OS backends via config.

**Architecture:** `trigger::macos_hotplug` uses `rusb`'s native hotplug API
(unlike Windows' `WM_DEVICECHANGE` hack). `ddc-backend::macos_ioavservice`
depends on the published `ddc-hi`/`ddc-macos` crates instead of writing raw
`IOAVServiceReadI2C`/`WriteI2C` FFI (trade-off: no source-addr override on
macOS, only `code`/`value` are configurable there — see Global Constraints).
`power-fallback::macos_pmset` shells out to `pmset`/`caffeinate`.
`daemon/src/main.rs` is refactored to share OS-agnostic wiring across a new
`windows_main`/`macos_main` module split.

**Tech Stack:** Rust, `ddc-hi = "0.4"`, `ddc-macos = "0.2.2"` (macOS-only),
`rusb = "0.9"` (already a dependency, used differently on macOS).

## Global Constraints

- Companion design spec (read for full rationale):
  `docs/superpowers/specs/2026-07-07-macos-backend-design.md`.
- Companion research (verified `IOAVService*` API details, **not directly used**
  in this plan since we depend on `ddc-macos` instead of writing FFI — kept for
  context/fallback if this design proves insufficient):
  `docs/superpowers/research/2026-07-07-ioavservice-macos-ddc-api.md`.
- **This development environment is Windows-only.** Tasks touching only
  cross-platform logic (`kvm_core::config`, `monitor_map`, `orchestrator`,
  `trigger`'s shared `device_id` helper, the Windows half of `daemon`) are
  verified here with real `cargo build`/`cargo test`. Tasks touching
  `#[cfg(target_os = "macos")]` code can only be verified with
  `cargo check --target aarch64-apple-darwin` (type-checks, does **not**
  link — no Apple SDK/frameworks available here). Real build/link/run
  verification of the macOS-specific code happens on the user's Mac, per
  `MANUAL_TEST_MACOS.md` (produced by this plan's last task).
- **No source-address override on macOS.** `ddc-backend::MacosIoavserviceBackend`
  accepts `source_addr: Option<u8>` (same trait signature as Windows) but
  ignores it with a `log::warn!` if `Some(_)` — the `ddc-hi`/`ddc-macos` crates'
  public API hardcodes the DDC/CI sub-address to `0x51` internally and does not
  expose an override. This is an accepted, explicit trade-off (see spec) — do
  not attempt to "fix" this by writing raw FFI as part of this plan.
  The user explicitly chose to give up this capability in exchange for
  depending on a known-working crate instead of new FFI; if it proves
  insufficient, that's a separate future decision, not something to solve in
  this plan.
- **New config field:** `on_usb_connect_vcp_code: Option<u8>` (hex string,
  e.g. `"0x60"`), default `0x60`. Exists specifically so a monitor needing an
  LG "alt" input-select code (e.g. `0xD0`, `0x90` — see `DECISIONS.md` §2)
  can be configured without a rebuild, since macOS has no source-addr lever.
- **Field rename:** `Configuration::nvapi_display_index` → `display_index`
  (Windows-flavored name generalized to an OS-neutral "ordinal index into this
  backend's display enumeration").
- Validated Windows recipe (unchanged, do not modify):
  `display_index=0`, VCP code `0x60`, value `0x11` (Hdmi1), source address
  `0x50`. The macOS recipe is **unvalidated** — this plan does not claim any
  specific macOS VCP code/value works; that's for the user to determine via
  `MANUAL_TEST_MACOS.md`.

---

## Task 1: Add the `aarch64-apple-darwin` rustup target

Host setup only; no repo files change, no commit.

- [ ] **Step 1: Add the target**

Run (with cargo on `PATH` — `export PATH="$PATH:/c/Users/nando/.cargo/bin:/c/Users/nando/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin"` first if needed):
```
rustup target add aarch64-apple-darwin
```
Expected: rustup downloads and installs the target's standard library
component. This does **not** provide a linker or Apple SDK/frameworks — it
only enables `cargo check`, not `cargo build`, for this target.

- [ ] **Step 2: Verify with a trivial check**

Run:
```
cargo check --target aarch64-apple-darwin -p kvm_core
```
Expected: succeeds (this crate has no macOS-specific code yet, so this just
confirms the target itself is usable for `cargo check`).

---

## Task 2: `kvm_core::config` — add `vcp_code`, rename `display_index`

**Files:**
- Modify: `crates/kvm_core/src/config.rs`
- Modify: `config/kvm-switch.example.ini`

**Interfaces:**
- Produces: `Configuration::on_usb_connect_vcp_code: Option<u8>` (field),
  `Configuration::vcp_code(&self) -> u8` (defaults to `0x60`).
- Produces: `Configuration::display_index: Option<u32>` (field, renamed from
  `nvapi_display_index`), `Configuration::display_index(&self) -> u32`
  (unchanged method name/behavior — Rust allows a field and a method to share
  a name, accessed via `.display_index` vs `.display_index()`).
- Consumes: nothing new.

- [ ] **Step 1: Write the failing tests**

In `crates/kvm_core/src/config.rs`, add to the `#[cfg(test)] mod tests` block
(after the existing `display_index_defaults_to_zero` test):

```rust
    #[test]
    fn vcp_code_defaults_to_0x60() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.vcp_code(), 0x60);
    }

    #[test]
    fn vcp_code_override_is_parsed_as_hex() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_vcp_code = "0xD0"
        "#,
        )
        .unwrap();
        assert_eq!(config.vcp_code(), 0xD0);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kvm_core config::tests`
Expected: **compile error** — `Configuration::vcp_code` doesn't exist yet.

- [ ] **Step 3: Rename `parse_source_addr` to `parse_hex_u8` and reuse it**

The existing `parse_source_addr` function (lines 101-116) parses an
`Option<String>` hex value into `Option<u8>` — this is exactly what
`on_usb_connect_vcp_code` also needs, so rename it to a neutral name instead
of writing a near-identical second function. Replace:

```rust
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
```

with:

```rust
fn parse_hex_u8<'de, D>(deserializer: D) -> std::result::Result<Option<u8>, D::Error>
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
                .map_err(|_| DeError::custom(format!("Invalid hex value: {}", s)))
        }
    }
}
```

- [ ] **Step 4: Update `Configuration` — new field, renamed field, both using `parse_hex_u8`**

Replace the `Configuration` struct and its `impl` block:

```rust
#[derive(Debug, Deserialize)]
pub struct Configuration {
    #[serde(deserialize_with = "Configuration::deserialize_usb_device")]
    pub usb_device: String,
    pub on_usb_connect: Option<InputSource>,
    pub on_usb_disconnect: Option<InputSource>,
    #[serde(default, deserialize_with = "parse_hex_u8")]
    pub on_usb_connect_source_addr: Option<u8>,
    #[serde(default, deserialize_with = "parse_hex_u8")]
    pub on_usb_connect_vcp_code: Option<u8>,
    #[serde(default)]
    pub display_index: Option<u32>,
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
        self.display_index.unwrap_or(0)
    }

    pub fn vcp_code(&self) -> u8 {
        self.on_usb_connect_vcp_code.unwrap_or(0x60)
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p kvm_core config::tests`
Expected: all 8 tests in `config::tests` `PASS` (the original 6 plus the 2
new ones from Step 1).

- [ ] **Step 6: Update the example config**

In `config/kvm-switch.example.ini`, replace the last block:

```ini
# Optional: NVAPI display index (0 = first screen) passed as
# writeValueToDisplay.exe's `display_index` argument. Defaults to 0.
# nvapi_display_index = 0
```

with:

```ini
# Optional: display index (0 = first screen) — on Windows, passed as
# writeValueToDisplay.exe's `display_index` argument; on macOS, the ordinal
# position in ddc-hi's display enumeration. Defaults to 0.
# display_index = 0

# Optional: VCP feature code to write for input switching. Defaults to 0x60
# (the DDC/CI standard "Input Source" code). Some monitors (e.g. this LG
# model) also respond to vendor "alt" codes (0xD0, 0x90, ...) documented in
# DECISIONS.md #2 — override this if the standard code doesn't switch the
# monitor, especially on macOS where there's no source-addr override to fall
# back on (see DECISIONS.md and the macOS backend design spec).
# on_usb_connect_vcp_code = "0x60"
```

- [ ] **Step 7: Commit**

```bash
git add crates/kvm_core/src/config.rs config/kvm-switch.example.ini
git commit -m "Add on_usb_connect_vcp_code config field, rename display_index"
```

---

## Task 3: `kvm_core::monitor_map` — carry `vcp_code` in `SwitchTarget`

**Files:**
- Modify: `crates/kvm_core/src/monitor_map.rs`

**Interfaces:**
- Consumes: `Configuration::vcp_code(&self) -> u8` (Task 2).
- Produces: `SwitchTarget.vcp_code: u8` (new field).

- [ ] **Step 1: Write the failing test**

Add to `crates/kvm_core/src/monitor_map.rs`'s `#[cfg(test)] mod tests` block
(after `resolves_connect_target_from_config`):

```rust
    #[test]
    fn resolves_vcp_code_override_from_config() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_vcp_code = "0xD0"
        "#,
        );
        let target = resolve(&config, SwitchDirection::Connect).unwrap();
        assert_eq!(target.vcp_code, 0xD0);
    }
```

Also update the existing `resolves_connect_target_from_config` test to assert
the new field's default value — add this line right after the existing
`assert_eq!(target.source_addr, Some(0x50));`:

```rust
        assert_eq!(target.vcp_code, 0x60);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kvm_core monitor_map::tests`
Expected: **compile error** — `SwitchTarget` has no field `vcp_code`.

- [ ] **Step 3: Add the field and populate it in `resolve`**

Replace the `SwitchTarget` struct and `resolve` function:

```rust
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

Run: `cargo test -p kvm_core monitor_map::tests`
Expected: all 3 tests in `monitor_map::tests` `PASS` (2 original + 1 new).

- [ ] **Step 5: Commit**

```bash
git add crates/kvm_core/src/monitor_map.rs
git commit -m "Carry vcp_code in SwitchTarget"
```

---

## Task 4: `kvm_core::orchestrator` — use `target.vcp_code` instead of a fixed constant

**Files:**
- Modify: `crates/kvm_core/src/orchestrator.rs`

**Interfaces:**
- Consumes: `SwitchTarget.vcp_code: u8` (Task 3).
- No change to `handle_event`'s own signature.

- [ ] **Step 1: Write the failing test**

Add to `crates/kvm_core/src/orchestrator.rs`'s `#[cfg(test)] mod tests` block
(after `successful_switch_calls_ddc_backend_once_with_resolved_target`):

```rust
    #[test]
    fn successful_switch_uses_configured_vcp_code_override() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_vcp_code = "0xD0"
        "#,
        );
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostGainedFocus, &config, &ddc, &power);

        assert_eq!(*ddc.calls.borrow(), vec![(0, 0xD0, 0x11, None)]);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p kvm_core orchestrator::tests`
Expected: `successful_switch_uses_configured_vcp_code_override` **FAILS** —
the assertion sees `0x60` (the still-hardcoded `INPUT_SELECT` constant)
instead of `0xD0`.

- [ ] **Step 3: Use `target.vcp_code` instead of the constant**

Replace the top of `crates/kvm_core/src/orchestrator.rs`:

```rust
use crate::config::Configuration;
use crate::monitor_map::{self, SwitchDirection};
use ddc_backend::DdcBackend;
use power_fallback::PowerFallback;
use trigger::TriggerEvent;

pub fn handle_event(
```

(i.e. delete the `/// VCP feature code for input select...` doc comment and
the `const INPUT_SELECT: u8 = 0x60;` line entirely), and inside `handle_event`,
replace:

```rust
    let attempt = |ddc_backend: &dyn DdcBackend| {
        ddc_backend.set_vcp(target.display_index, INPUT_SELECT, target.input_source.value(), target.source_addr)
    };
```

with:

```rust
    let attempt = |ddc_backend: &dyn DdcBackend| {
        ddc_backend.set_vcp(target.display_index, target.vcp_code, target.input_source.value(), target.source_addr)
    };
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kvm_core orchestrator::tests`
Expected: all 4 tests in `orchestrator::tests` `PASS` (3 original + 1 new).
The original 3 tests still pass unchanged because `config.vcp_code()`
defaults to `0x60` — the same value the deleted constant held.

- [ ] **Step 5: Run the whole `kvm_core` suite**

Run: `cargo test -p kvm_core`
Expected: all 15 tests pass (8 `config` + 3 `monitor_map` + 4
`orchestrator`).

- [ ] **Step 6: Commit**

```bash
git add crates/kvm_core/src/orchestrator.rs
git commit -m "Use configured vcp_code instead of a fixed INPUT_SELECT constant"
```

---

## Task 5: `trigger` — extract shared `device_id` helper

**Files:**
- Modify: `crates/trigger/src/lib.rs`
- Modify: `crates/trigger/src/usb_hotplug.rs`

**Interfaces:**
- Produces: `pub(crate) fn device_id<T: rusb::UsbContext>(device: &rusb::Device<T>) -> Option<String>` in `trigger::lib`.
- Consumed by: `usb_hotplug.rs` (this task) and `macos_hotplug.rs` (Task 6).

This is a pure refactor (no behavior change) preparing for Task 6, which needs
the same device-id-formatting logic. No new test — the existing 3
`usb_hotplug::tests` are the regression check.

- [ ] **Step 1: Add the shared helper to `lib.rs`**

In `crates/trigger/src/lib.rs`, add after the `pub trait TriggerSource` block
(before the `// TODO(v2): bluetooth_hid.rs` comments):

```rust
pub(crate) fn device_id<T: rusb::UsbContext>(device: &rusb::Device<T>) -> Option<String> {
    device
        .device_descriptor()
        .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
        .ok()
}
```

- [ ] **Step 2: Remove the duplicate from `usb_hotplug.rs` and use the shared one**

In `crates/trigger/src/usb_hotplug.rs`, delete this function entirely:

```rust
fn device_id<T: UsbContext>(device: &rusb::Device<T>) -> Option<String> {
    device
        .device_descriptor()
        .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
        .ok()
}
```

and change the one call site, inside `read_device_list`, from:

```rust
fn read_device_list() -> Result<HashSet<String>> {
    Ok(rusb::devices()?.iter().filter_map(|device| device_id(&device)).collect())
}
```

to:

```rust
fn read_device_list() -> Result<HashSet<String>> {
    Ok(rusb::devices()?.iter().filter_map(|device| crate::device_id(&device)).collect())
}
```

The `use rusb::UsbContext;` import at the top of `usb_hotplug.rs` is still
needed (used elsewhere in the file, e.g. the `HotplugHandler`/window-loop
code's generic bounds) — leave it as-is.

- [ ] **Step 3: Run the tests to verify nothing broke**

Run: `cargo test -p trigger`
Expected: all 3 `usb_hotplug::tests` still `PASS`, crate builds clean (this
also exercises the full window-loop unsafe code path at compile time, same as
before the refactor).

- [ ] **Step 4: Commit**

```bash
git add crates/trigger/src/lib.rs crates/trigger/src/usb_hotplug.rs
git commit -m "Extract shared device_id helper out of usb_hotplug"
```

---

## Task 6: `trigger::macos_hotplug`

**Files:**
- Modify: `crates/trigger/src/lib.rs`
- Create: `crates/trigger/src/macos_hotplug.rs`

**Interfaces:**
- Consumes: `trigger::device_id` (Task 5), `trigger::{TriggerEvent, TriggerSource}`.
- Produces: `pub struct MacosHotplugTrigger` with `pub fn new(usb_device: &str) -> Self`, implementing `TriggerSource`.

This module is `#[cfg(target_os = "macos")]`-gated and cannot be exercised by
`cargo build`/`cargo test` on this (Windows) machine at all — it is invisible
to the compiler here. Verification in this task is limited to
`cargo check --target aarch64-apple-darwin`, which type-checks without
linking. There is no way to unit-test this module's logic further than that
in this environment; `MANUAL_TEST_MACOS.md` (Task 10) covers the real
behavioral check.

- [ ] **Step 1: Create the module**

Create `crates/trigger/src/macos_hotplug.rs`:

```rust
use crate::TriggerEvent;
use anyhow::{anyhow, Result};
use rusb::{Context, Device, HotplugBuilder, Registration, UsbContext};
use std::sync::mpsc::{self, Sender};

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
        if crate::device_id(&device).as_deref() == Some(self.usb_device.as_str()) {
            let _ = self.sender.send(TriggerEvent::HostGainedFocus);
        }
    }

    fn device_left(&mut self, device: Device<T>) {
        if crate::device_id(&device).as_deref() == Some(self.usb_device.as_str()) {
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

- [ ] **Step 2: Wire the module into `lib.rs`**

In `crates/trigger/src/lib.rs`, change:

```rust
#[cfg(windows)]
pub mod usb_hotplug;
```

to:

```rust
#[cfg(windows)]
pub mod usb_hotplug;
#[cfg(target_os = "macos")]
pub mod macos_hotplug;
```

- [ ] **Step 3: Type-check against the macOS target**

Run:
```
cargo check --target aarch64-apple-darwin -p trigger
```
Expected: succeeds. This is the only verification available in this
environment for this file.

- [ ] **Step 4: Confirm the Windows build is still unaffected**

Run: `cargo test -p trigger`
Expected: still 3/3 passing, unchanged — `macos_hotplug.rs` is invisible to
this build (`cfg(target_os = "macos")` excludes it entirely on Windows), so
this just confirms Step 2's `lib.rs` edit didn't break the Windows path.

- [ ] **Step 5: Commit**

```bash
git add crates/trigger/src/lib.rs crates/trigger/src/macos_hotplug.rs
git commit -m "Port upstream's libusb hotplug watcher as trigger::macos_hotplug"
```

---

## Task 7: `ddc-backend::macos_ioavservice`

**Files:**
- Modify: `crates/ddc-backend/Cargo.toml`
- Modify: `crates/ddc-backend/src/lib.rs`
- Create: `crates/ddc-backend/src/macos_ioavservice.rs`

**Interfaces:**
- Consumes: `ddc_backend::DdcBackend` trait (already defined).
- Produces: `pub struct MacosIoavserviceBackend` (unit struct, no
  constructor needed — `ddc_hi::Display::enumerate()` is called fresh inside
  `set_vcp`), implementing `DdcBackend`.

Same environment caveat as Task 6: `#[cfg(target_os = "macos")]`-gated,
`cargo check --target aarch64-apple-darwin` only.

- [ ] **Step 1: Add the macOS dependencies**

In `crates/ddc-backend/Cargo.toml`, add at the end of the file:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
ddc-hi = "0.4"
ddc-macos = "0.2.2"
```

- [ ] **Step 2: Create the module**

Create `crates/ddc-backend/src/macos_ioavservice.rs`:

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

- [ ] **Step 3: Wire the module into `lib.rs`**

In `crates/ddc-backend/src/lib.rs`, replace:

```rust
// TODO(macos): macos_ioavservice.rs — IOAVServiceReadI2C/WriteI2C backend,
// blocked on Spike #2 (see DECISIONS.md #5, #7).
// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9).

#[cfg(windows)]
pub mod windows_generic;
#[cfg(windows)]
pub mod windows_nvapi;
```

with:

```rust
// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9).

#[cfg(target_os = "macos")]
pub mod macos_ioavservice;
#[cfg(windows)]
pub mod windows_generic;
#[cfg(windows)]
pub mod windows_nvapi;
```

(The `TODO(macos)` comment is removed since this task implements that module;
the `TODO(v2)` Linux comment stays.)

- [ ] **Step 4: Type-check against the macOS target**

Run:
```
cargo check --target aarch64-apple-darwin -p ddc-backend
```
Expected: succeeds.

- [ ] **Step 5: Confirm the Windows build is still unaffected**

Run: `cargo test -p ddc-backend`
Expected: still 2/2 passing, unchanged.

- [ ] **Step 6: Commit**

```bash
git add crates/ddc-backend/Cargo.toml crates/ddc-backend/src/lib.rs crates/ddc-backend/src/macos_ioavservice.rs
git commit -m "Add MacosIoavserviceBackend using the ddc-hi/ddc-macos crates"
```

---

## Task 8: `power-fallback::macos_pmset`

**Files:**
- Modify: `crates/power-fallback/src/lib.rs`
- Create: `crates/power-fallback/src/macos_pmset.rs`

**Interfaces:**
- Produces: `pub struct MacosPmset` (unit struct), implementing `PowerFallback`.

Same environment caveat as Tasks 6-7.

- [ ] **Step 1: Create the module**

Create `crates/power-fallback/src/macos_pmset.rs`:

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

- [ ] **Step 2: Wire the module into `lib.rs`**

In `crates/power-fallback/src/lib.rs`, change:

```rust
#[cfg(windows)]
pub mod windows_monitorpower;
```

to:

```rust
#[cfg(target_os = "macos")]
pub mod macos_pmset;
#[cfg(windows)]
pub mod windows_monitorpower;
```

- [ ] **Step 3: Type-check against the macOS target**

Run:
```
cargo check --target aarch64-apple-darwin -p power-fallback
```
Expected: succeeds.

- [ ] **Step 4: Confirm the Windows build is still unaffected**

Run: `cargo build -p power-fallback`
Expected: still succeeds, unchanged (this crate has no unit tests, per the
original MVP plan's Global Constraints).

- [ ] **Step 5: Commit**

```bash
git add crates/power-fallback/src/lib.rs crates/power-fallback/src/macos_pmset.rs
git commit -m "Add MacosPmset blank/restore fallback via pmset+caffeinate"
```

---

## Task 9: `daemon` — split `main.rs` into shared wiring + per-OS modules

**Files:**
- Modify: `crates/daemon/src/main.rs`

**Interfaces:**
- Consumes: `ddc_backend::macos_ioavservice::MacosIoavserviceBackend` (Task 7),
  `power_fallback::macos_pmset::MacosPmset` (Task 8),
  `trigger::macos_hotplug::MacosHotplugTrigger` (Task 6).
- No change to the crate's external interface (still a `[[bin]]`, same CLI
  flags).

This is the highest-risk task in this plan: it rewrites the currently-working
Windows entry point. Read carefully and verify the Windows path for real
after each step — do not just trust the macOS half compiling.

- [ ] **Step 1: Replace `main.rs` in full**

Replace the entire contents of `crates/daemon/src/main.rs`:

```rust
use anyhow::{Context, Result};
use clap::Parser;
use kvm_core::config::Configuration;
use kvm_core::orchestrator;
use trigger::TriggerSource;

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

fn init_logging(debug: bool) -> Result<()> {
    use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
    let level = if debug { LevelFilter::Debug } else { LevelFilter::Info };
    CombinedLogger::init(vec![TermLogger::new(level, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)])
        .context("failed to initialize logging")
}

fn load_config(args: &Args) -> Result<Configuration> {
    let config_path = args
        .config_file_path
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("display-switch.ini"));
    Configuration::load(&config_path).with_context(|| format!("failed to load configuration from {:?}", config_path))
}

fn run_daemon(
    config: &Configuration,
    trigger_source: &dyn TriggerSource,
    ddc_backend: &dyn ddc_backend::DdcBackend,
    power_fallback: &dyn power_fallback::PowerFallback,
) {
    log::info!("kvm-switch daemon started, watching USB device {}", config.usb_device);
    for event in trigger_source.watch() {
        orchestrator::handle_event(event, config, ddc_backend, power_fallback);
    }
}

#[cfg(windows)]
mod windows_main {
    use super::{init_logging, load_config, run_daemon, Args};
    use anyhow::{Context, Result};
    use clap::Parser;
    use ddc_backend::windows_nvapi::NvapiBackend;
    use power_fallback::windows_monitorpower::WindowsMonitorPower;
    use trigger::usb_hotplug::UsbHotplugTrigger;
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};

    /// Re-attach the console if the parent process has one, so log output
    /// shows up when run from the command line.
    fn attach_console() {
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }

    /// Resolves `tools/writeValueToDisplay.exe` relative to the current
    /// working directory, matching the same CWD-relative convention used for
    /// the config file default. This is intended to be run via `cargo run`
    /// (or otherwise) from the repo root, per `MANUAL_TEST.md`.
    fn default_exe_path() -> Result<std::path::PathBuf> {
        Ok(std::path::PathBuf::from("tools/writeValueToDisplay.exe"))
    }

    pub fn main() -> Result<()> {
        attach_console();
        let args = Args::parse();
        init_logging(args.debug)?;
        let config = load_config(&args)?;
        let ddc_backend = NvapiBackend::new(default_exe_path().context("failed to locate writeValueToDisplay.exe")?);
        let power_fallback = WindowsMonitorPower;
        let trigger_source = UsbHotplugTrigger::new(&config.usb_device);
        run_daemon(&config, &trigger_source, &ddc_backend, &power_fallback);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos_main {
    use super::{init_logging, load_config, run_daemon, Args};
    use anyhow::Result;
    use clap::Parser;
    use ddc_backend::macos_ioavservice::MacosIoavserviceBackend;
    use power_fallback::macos_pmset::MacosPmset;
    use trigger::macos_hotplug::MacosHotplugTrigger;

    pub fn main() -> Result<()> {
        let args = Args::parse();
        init_logging(args.debug)?;
        let config = load_config(&args)?;
        let ddc_backend = MacosIoavserviceBackend;
        let power_fallback = MacosPmset;
        let trigger_source = MacosHotplugTrigger::new(&config.usb_device);
        run_daemon(&config, &trigger_source, &ddc_backend, &power_fallback);
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> Result<()> {
    windows_main::main()
}

#[cfg(target_os = "macos")]
fn main() -> Result<()> {
    macos_main::main()
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn main() {
    eprintln!("kvm-switch-daemon currently only supports Windows and macOS.");
    std::process::exit(1);
}
```

- [ ] **Step 2: Build and test the Windows path for real**

Run:
```
cargo build --workspace
cargo test --workspace
```
Expected: both succeed cleanly. Test count should be 20 (2 ddc-backend + 15
kvm_core + 3 trigger — `kvm_core` grew from 11 to 15 across Tasks 2-4: 2 new
`config` tests, 1 new `monitor_map` test, 1 new `orchestrator` test), all
passing.

- [ ] **Step 3: Sanity-check the daemon binary still starts correctly on Windows**

Run:
```
cargo run -p kvm-switch-daemon -- --help
```
Expected: clap's `--help` output prints (confirms `Args`/`clap::Parser` wiring
is intact) without any panic.

- [ ] **Step 4: Type-check the macOS path**

Run:
```
cargo check --target aarch64-apple-darwin -p kvm-switch-daemon
```
Expected: succeeds — confirms `macos_main`'s wiring against
`MacosIoavserviceBackend`/`MacosPmset`/`MacosHotplugTrigger`'s actual
signatures (from Tasks 6-8) type-checks.

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/main.rs
git commit -m "Split daemon main.rs into shared wiring + windows_main/macos_main"
```

---

## Task 10: `MANUAL_TEST_MACOS.md`

**Files:**
- Create: `MANUAL_TEST_MACOS.md`

- [ ] **Step 1: Write the manual test doc**

Create `MANUAL_TEST_MACOS.md`:

```markdown
# Manual Test: macOS backend (Mac -> Windows switch via USB hotplug)

Unlike `MANUAL_TEST.md` (Windows), this backend's VCP recipe has **not** been
validated ahead of time — there was no Spike #1-equivalent for macOS. Expect
to iterate on `config/kvm-switch.example.ini` (copied to
`display-switch.ini`) before it works. Run this after confirming
`cargo build --workspace` succeeds **on the Mac** (this repo was developed on
a Windows machine with no way to build or run macOS code — this is the first
real build/run of this backend).

## Setup

1. On the Mac, from the repo root: `cargo build --workspace`. If this fails
   to *link* (as opposed to type-check, which was already verified on the dev
   machine via `cargo check --target aarch64-apple-darwin`), that's a real
   bug in this backend — report it, don't work around it silently.
2. Copy `config/kvm-switch.example.ini` to `display-switch.ini` in the repo
   root and set `usb_device` to the same VID:PID as the Windows side
   (`17e9:6000`, unless your hardware differs), and `on_usb_connect` to the
   input source this Mac's monitor cable is on (e.g. `DisplayPort2` if this
   Mac connects via DisplayPort, or `Hdmi1` if HDMI — see `DECISIONS.md` for
   this setup's actual physical topology).

## Steps

1. Run the daemon with debug logging:
   ```
   cargo run -p kvm-switch-daemon -- --debug
   ```
2. Physically toggle the USB switch so the watched device connects to this
   Mac's USB bus.
3. Observe the daemon's log output for a `Display switched to ... for
   Connect` line (or an error — see Troubleshooting below).
4. Confirm the monitor actually switches to this Mac's input.
5. Repeat steps 2-4 a few times to confirm reliability.

## Troubleshooting: the monitor doesn't switch

Since there's no source-addr override on macOS (see the design spec,
`docs/superpowers/specs/2026-07-07-macos-backend-design.md`), the only lever
available if the default VCP code (`0x60`, DDC/CI standard "Input Source") is
being silently ignored by the monitor is the code itself:

1. Uncomment `on_usb_connect_vcp_code` in `display-switch.ini` and try one of
   the LG "alt" input-select codes documented in `DECISIONS.md` §2 (e.g.
   `"0xD0"`, `"0x90"`, `"0xD1"`, `"0xD2"`) — no rebuild needed, just restart
   the daemon.
2. If a `Display switched to ...` log line appears but the monitor doesn't
   actually change, the write likely succeeded at the protocol level but the
   monitor rejected/ignored the specific code+value combination — try a
   different code from the same list.
3. If `IOAVServiceReadI2C`/`WriteI2C`-level errors show up in the log (surfaced
   as an `anyhow` error from `ddc-hi`), see
   `docs/superpowers/research/2026-07-07-ioavservice-macos-ddc-api.md` for
   what's actually happening under the hood — the "Open questions" section
   covers known gaps (no confirmed timeout behavior, buffer size limits,
   `CFRelease` semantics) that could explain intermittent failures.

## Known non-goals for this milestone

- No automated test exists for this end-to-end flow — it requires physically
  toggling the USB switch and observing the monitor, on real Apple Silicon
  hardware this project was never built or run on until now.
- If neither the standard code nor any LG "alt" code switches the monitor
  reliably, the next step is writing raw `IOAVServiceReadI2C`/`WriteI2C` FFI
  bindings for a source-addr override (see the design spec's "Deviations"
  section) — that's a follow-up decision, not something to debug further
  under this plan.
```

- [ ] **Step 2: Commit**

```bash
git add MANUAL_TEST_MACOS.md
git commit -m "Add manual test instructions for the macOS backend"
```

---

## Task 11: Final verification

**Files:** none (verification only).

- [ ] **Step 1: Full workspace build and test on Windows**

Run:
```
cargo build --workspace
cargo test --workspace
```
Expected: both succeed cleanly. 20 tests total (2 ddc-backend + 15 kvm_core +
3 trigger), `daemon`/`power-fallback` still have 0 unit tests as expected.

- [ ] **Step 2: Type-check every touched crate against macOS**

Run:
```
cargo check --target aarch64-apple-darwin -p ddc-backend -p trigger -p power-fallback -p kvm-switch-daemon
```
Expected: succeeds for all four crates — this is the strongest verification
possible in this environment for the macOS-specific code added in Tasks 6-9.

- [ ] **Step 3: Confirm nothing is left uncommitted**

Run: `git status --porcelain`
Expected: clean (or only unrelated pre-existing untracked files, e.g.
`display-switch.ini`, `.vscode/` — nothing from this plan's work should be
untracked at this point).

This plan's true acceptance test is `MANUAL_TEST_MACOS.md`, run by the human
on real Apple Silicon hardware — not something this task (or this
environment) can complete.
