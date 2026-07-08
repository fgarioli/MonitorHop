# macOS Backend — Design

> Companion to `DECISIONS.md` (hardware investigation) and
> `docs/superpowers/plans/2026-07-06-kvm-switch-fork-mvp-implementation.md` (the
> Windows-only MVP this extends). Companion research:
> `docs/superpowers/research/2026-07-07-ioavservice-macos-ddc-api.md`.

## Goal

Add a working macOS backend to the kvm-switch-fork daemon, so the Mac side of the
KVM setup can also run this daemon (mirroring the Windows vertical slice): USB
hotplug trigger → DDC backend → monitor switches input, with a power-fallback
retry path. Full vertical slice (trigger + ddc-backend + power-fallback + daemon
wiring for macOS), not just the DDC-write piece.

## Development environment constraint

This repo is developed on a Windows machine with no macOS SDK/toolchain — the
code below can be **written and type-checked** (`cargo check --target
aarch64-apple-darwin`, no linking possible without Apple's frameworks) but
**cannot be built, linked, or run** in this environment. The user will build and
run it on their own Mac once implemented. `MANUAL_TEST_MACOS.md` (see below) is
the real acceptance test, and — unlike the Windows backend, where the DDC recipe
was empirically validated via a spike before any code was written — the macOS
VCP recipe is **unvalidated** going in. This is an accepted, explicit trade-off
(see "Deviations" below).

## Config changes (`kvm_core::config`)

- New field `on_usb_connect_vcp_code: Option<u8>` on `Configuration`, parsed the
  same way as the existing `on_usb_connect_source_addr` (hex string like
  `"0x60"`), defaulting to `0x60` (`INPUT_SELECT`, the DDC/CI standard code —
  matches the constant currently hardcoded in `orchestrator.rs`).
- `monitor_map::SwitchTarget` gains a `vcp_code: u8` field, resolved from
  `Configuration` the same way `source_addr`/`input_source` already are.
- `orchestrator::handle_event` uses `target.vcp_code` instead of the hardcoded
  `INPUT_SELECT` constant.
- Rename `Configuration::nvapi_display_index` → `display_index` (and
  `nvapi_display_index()` accessor → `display_index()`, already named this at
  the accessor level). The field was Windows/NVAPI-flavored naming for what is
  actually an OS-neutral "ordinal position in this backend's display
  enumeration" — both the NVAPI exe (`display_index` arg) and macOS's
  `ddc_hi::Display::enumerate()` (Vec index) need the same kind of value.

This is why the field exists at all: without a per-OS way to override the VCP
code, a monitor that needs an LG "alt" input-select code (`0xD0`, `0x90`, etc.
— see `DECISIONS.md` §2) instead of the DDC/CI standard `0x60` would need a
code change instead of a config change.

## `ddc-backend::macos_ioavservice`

Depends on the published `ddc-hi`/`ddc-macos` crates (matching upstream
`haimgel/display-switch`'s original `Cargo.toml` pins,
`ddc-macos = "0.2.2"`/`ddc-hi = "0.4"`) rather than writing raw
`IOAVServiceReadI2C`/`WriteI2C` FFI bindings directly.

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

`crates/ddc-backend/Cargo.toml` gains:
```toml
[target.'cfg(target_os = "macos")'.dependencies]
ddc-hi = "0.4"
ddc-macos = "0.2.2"
```
and `crates/ddc-backend/src/lib.rs`'s `// TODO(macos)` marker is replaced with
`#[cfg(target_os = "macos")] pub mod macos_ioavservice;`.

## `trigger::macos_hotplug`

Ports upstream's `platform/pnp_detect_libusb.rs` (present in this repo's git
history from the upstream merge, removed from the working tree in Task 3 of the
MVP plan) to this fork's `TriggerSource`/`TriggerEvent` model. Unlike Windows,
`rusb`'s native hotplug API works on macOS, so no custom OS message-loop is
needed.

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
        Self { usb_device: usb_device.to_lowercase() }
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

DRY note: `device_id(&Device<T>) -> Option<String>` (formats `"vvvv:pppp"`) is
identical logic to what already exists inside `trigger::usb_hotplug` (Windows).
Factor it into a shared `pub(crate) fn device_id` in `trigger/src/lib.rs`, used
by both platform modules (they never compile together, but the text shouldn't
be duplicated).

`crates/trigger/Cargo.toml` needs no new dependency — `MacosHotplugTrigger` only
uses `rusb`/`anyhow`/`log`, all already unconditional deps. `lib.rs` gains
`#[cfg(target_os = "macos")] pub mod macos_hotplug;` alongside the existing
`#[cfg(v2)]` TODO markers.

## `power-fallback::macos_pmset`

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

Same wake primitive (`caffeinate -u -t N`) as upstream's original
`platform::wake_displays` macOS branch, repurposed here as the "restore" half of
the blank/restore fallback pair (mirroring Windows' `SC_MONITORPOWER` +
mouse-jiggle). No new dependency; `lib.rs` gains
`#[cfg(target_os = "macos")] pub mod macos_pmset;`.

## `daemon` wiring

`crates/daemon/src/main.rs` is restructured to share the OS-agnostic pieces
(`Args`, `init_logging`, config loading, the trigger-watch loop) across three
platform-specific wiring modules instead of one `#[cfg(windows)]` monolith plus
a bare stub:

```rust
// unconditional (shared)
#[derive(Parser, Debug)]
struct Args { /* --debug, --config — unchanged from today */ }

fn init_logging(debug: bool) -> Result<()> { /* unchanged from today */ }

fn load_config(args: &Args) -> Result<Configuration> {
    let config_path = args.config_file_path.clone().unwrap_or_else(|| std::path::PathBuf::from("display-switch.ini"));
    Configuration::load(&config_path).with_context(|| format!("failed to load configuration from {:?}", config_path))
}

fn run_daemon(config: &Configuration, trigger_source: &dyn TriggerSource, ddc_backend: &dyn DdcBackend, power_fallback: &dyn PowerFallback) {
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

    fn attach_console() {
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }

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
fn main() -> Result<()> { windows_main::main() }

#[cfg(target_os = "macos")]
fn main() -> Result<()> { macos_main::main() }

#[cfg(all(not(windows), not(target_os = "macos")))]
fn main() {
    eprintln!("kvm-switch-daemon currently only supports Windows and macOS.");
    std::process::exit(1);
}
```

No `daemon/Cargo.toml` changes needed — `kvm_core`/`trigger`/`ddc-backend`/
`power-fallback` are already unconditional path dependencies; only `winapi`
is `cfg(windows)`-gated today, and macOS's wiring module needs nothing extra
from `daemon`'s own `Cargo.toml`.

## Testing / validation plan

**Verifiable in this (Windows) environment, with real `cargo test`:**
- `config.rs` (`on_usb_connect_vcp_code` field, `display_index` rename),
  `monitor_map.rs` (`SwitchTarget.vcp_code`), and `orchestrator.rs` (uses
  `target.vcp_code` instead of the `INPUT_SELECT` constant) are all
  OS-agnostic pure logic — TDD'd the same way Tasks 7-9 of the MVP plan were,
  with tests that actually run here. This is the majority of what could
  silently break.
- After adding the `aarch64-apple-darwin` rustup target,
  `cargo check --target aarch64-apple-darwin` on `ddc-backend`, `trigger`,
  `power-fallback`, `daemon` — type-checks (no linking, no Apple SDK/frameworks
  available here) against the real `ddc-hi`/`ddc-macos`/`rusb` crate APIs.
  Because this design uses published, typed crates rather than hand-written
  FFI, the risk `cargo check` can't catch (linker-level symbol/signature
  mismatches) is much smaller than it would have been with raw
  `IOAVServiceReadI2C`/`WriteI2C` bindings.

**Only verifiable on real macOS hardware (the user's Mac):**
- Whether `rusb` hotplug actually fires for the watched device on macOS.
- Whether `/usr/bin/pmset`/`/usr/bin/caffeinate` behave as expected at those
  paths.
- **The actual working VCP code/value for this monitor.** Unlike Windows,
  there is no source-addr lever left to pull (see Deviations below) — if the
  default `0x60`/`Hdmi1` doesn't switch the monitor, the only thing to try is
  a different `on_usb_connect_vcp_code` (e.g. one of the LG "alt" codes
  `0xD0`/`0x90` from `DECISIONS.md` §2), via config, no rebuild required.

Deliverable: `MANUAL_TEST_MACOS.md` (mirrors `MANUAL_TEST.md`), including this
troubleshooting path explicitly.

## Deviations from the Windows backend (accepted trade-offs)

- **No source-address override on macOS.** The Windows backend bypasses the
  standard `ddc-winapi`/`ddc-hi` stack specifically to get raw NVAPI I2C access
  with a configurable source address (`0x50` override, since Windows' standard
  DDC API hardcodes `0x51`). The macOS equivalent would require writing our own
  `IOAVServiceReadI2C`/`WriteI2C` FFI bindings (verified feasible — see the
  research doc — `CoreDisplay.framework`, `#[link(...)]`, no `dlopen` needed)
  instead of depending on the `ddc-macos` crate, whose public API only exposes
  the standard `ddc`/`ddc-hi` interface and hardcodes the DDC/CI sub-address to
  `0x51` internally. The user explicitly chose to give up this override
  capability in exchange for depending on a known-working published crate
  instead of hand-rolled FFI. If this turns out to be insufficient (the
  monitor needs a `0x50`-style override on macOS too, the way it does on
  Windows), the fallback plan is to revisit this decision and write the raw
  FFI bindings the research doc already prepared the ground for.
- **No empirically-validated recipe going in.** Windows had Spike #1 (manual
  testing via `writeValueToDisplay.exe`) before any daemon code was written.
  macOS does not have an equivalent spike — `on_usb_connect_vcp_code`/
  `on_usb_connect` exist specifically so the user can find the right values by
  editing config on their Mac, without needing new code or a rebuild, playing
  the role Spike #1's manual CLI testing played for Windows.

## Explicitly out of scope for this pass

- Writing raw `IOAVServiceReadI2C`/`WriteI2C` FFI bindings (only needed if the
  no-source-addr-override trade-off above proves insufficient).
- Linux backend, HID++/Bluetooth triggers, Tauri UI — unchanged from the MVP
  plan's v2/TODO scope.
- Cross-platform monitor-capability auto-detection (raised during brainstorming
  as a separate, larger, cross-cutting future project — not part of this
  design).
