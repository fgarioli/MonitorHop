# KVM Switch Fork — MVP Design

> Companion to `DECISIONS.md` (hardware investigation, validated VCP/source-addr values,
> full v1/v2 scope). This spec covers only the concrete engineering plan for standing up
> the fork and shipping the first working milestone: Windows→Mac monitor switch via USB
> hotplug, using the validated NVAPI source-addr override.

## Goal

Fork `display-switch` (https://github.com/haimgel/display-switch) into a Rust workspace
matching the module split in DECISIONS.md §9, and ship a first vertical slice: USB
hotplug trigger → NVAPI DDC backend (source-addr override) → monitor switches input.
No macOS backend, no power-fallback backend, no v2 triggers (HID++, Bluetooth) in this
pass.

## Bootstrap sequence

1. Install Rust toolchain: `winget install Rustlang.Rustup`, stable-msvc, verify
   `cargo --version`.
2. `git init` in this directory (already contains `DECISIONS.md`, `display-switch.ini`,
   `writeValueToDisplay.exe`, `.vscode`, `.claude`).
3. `git remote add upstream https://github.com/haimgel/display-switch`,
   `git fetch upstream`, `git merge upstream/main --allow-unrelated-histories` —
   preserves upstream commit history, `LICENSE`, and authorship as a real fork, merged
   alongside the existing files in this directory.
4. Restructure upstream's single-crate `src/` into the `crates/` workspace below,
   carrying upstream's hotplug-watching code into `trigger/usb_hotplug.rs` as the seed
   for the near-verbatim port.
5. Move `writeValueToDisplay.exe` into `tools/` (third-party compiled binary, not part
   of any crate's `src/`).

## Workspace layout (this milestone only)

```
Cargo.toml                        # workspace root, members = crates/*
tools/
└── writeValueToDisplay.exe       # third-party, pre-validated (kaleb422/NVapi-write-value-to-monitor)
crates/
├── core/
│   └── src/{lib.rs, config.rs, orchestrator.rs, monitor_map.rs}
├── trigger/
│   └── src/{lib.rs, usb_hotplug.rs}
├── ddc-backend/
│   └── src/{lib.rs, windows_nvapi.rs, windows_generic.rs}
├── power-fallback/
│   └── src/{lib.rs, windows_monitorpower.rs}
└── daemon/
    └── src/main.rs
config/
└── kvm-switch.example.ini
MANUAL_TEST.md
```

Explicitly out of scope for this pass (leave a `// TODO(macos): pending Spike #2` /
`// TODO(v2)` marker at the relevant trait definition instead of an empty file):
- `ddc-backend/macos_ioavservice.rs`, `ddc-backend/linux_ddcutil.rs`
- `power-fallback/macos_pmset.rs`
- `trigger/bluetooth_hid.rs`, `trigger/hidpp_receiver.rs`
- `ui-tauri/`

## Contracts

```rust
// crates/trigger/src/lib.rs
pub trait TriggerSource {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent>;
}
pub enum TriggerEvent { HostGainedFocus, HostLostFocus }

// crates/ddc-backend/src/lib.rs
pub trait DdcBackend {
    fn get_vcp(&self, monitor_id: &str, code: u8) -> Result<u16>;
    fn set_vcp(&self, monitor_id: &str, code: u8, value: u16, source_addr: Option<u8>) -> Result<()>;
}

// crates/power-fallback/src/lib.rs
pub trait PowerFallback {
    fn blank_and_restore(&self) -> Result<()>;
}
```

`core::orchestrator` wires them together: on `TriggerEvent::HostGainedFocus`, resolve
the target monitor + VCP value from config, call `ddc_backend.set_vcp(...)`; on error
or timeout, call `power_fallback.blank_and_restore()` and retry once. Retry/backoff
policy lives only in `orchestrator` — backends stay single-purpose (do the operation,
report success/failure) with no orchestration logic of their own.

## `ddc-backend/windows_nvapi.rs`

NVAPI's I2C write function requires NVIDIA's proprietary NVAPI SDK headers, which are
not redistributable. Rather than vendoring those headers and writing unsafe FFI bindings
in this MVP, `NvapiBackend` shells out to the already-validated
`tools/writeValueToDisplay.exe`:

```rust
pub struct NvapiBackend {
    exe_path: PathBuf,   // default: tools/writeValueToDisplay.exe relative to daemon binary
}

impl DdcBackend for NvapiBackend {
    fn set_vcp(&self, monitor_id: &str, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        // TODO(v2): replace with direct FFI against nvapi64.dll once we have the
        // NVIDIA SDK headers under an appropriate license, removing the exe dependency.
        let addr = source_addr.unwrap_or(0x50); // 0x50 = validated override for this monitor
        let status = Command::new(&self.exe_path)
            .args([monitor_id, &format!("0x{code:02X}"), &format!("0x{value:02X}"), &format!("0x{addr:02X}")])
            .status()?;
        if !status.success() {
            return Err(DdcError::BackendFailed(status));
        }
        Ok(())
    }
}
```

Default `source_addr` is `0x50` (the validated override), not the DDC-standard `0x51` —
config can pass `Some(0x51)` explicitly to use the standard address for monitors that
don't need the override.

## `ddc-backend/windows_generic.rs`

Documented fallback for non-NVIDIA GPUs (dxva2/`SetVCPFeature`), implemented against
the trait but with a `todo!()` body and a comment stating AMD/ADL source-addr override
support is unconfirmed — not a working path yet, per DECISIONS.md §4/§10.

## `trigger/usb_hotplug.rs`

Near-verbatim port of upstream's `rusb`-based hotplug watcher. Only structural change:
wrap it behind the `TriggerSource` trait and emit `TriggerEvent` instead of calling
orchestration logic inline (upstream doesn't separate these; this fork does).

## Config format

Extends upstream's `.ini` format (backward compatible):

```ini
usb_device = "17E9:6000"
on_usb_connect = "Hdmi1"
on_usb_connect_source_addr = "0x50"   ; optional override, defaults to 0x50 for windows_nvapi
on_usb_connect_fallback = "blank"     ; optional fallback strategy
```

## Milestone / success criteria

- `cargo build --workspace` succeeds cleanly.
- Manual integration test (not automated — depends on real hardware), documented in
  `MANUAL_TEST.md`: unplug/replug the USB switch (VID:PID `17E9:6000`), observe
  `daemon` call `NvapiBackend::set_vcp(monitor_id, 0x60, 0x11, Some(0x50))`, and confirm
  the monitor switches to HDMI1 (Mac) within a few seconds, reliably across repeated
  cycles.

## Explicitly deferred (not this milestone)

- macOS backend (`macos_ioavservice.rs`) — blocked on Spike #2 (USB-C→HDMI cable
  migration test), per DECISIONS.md §7.
- Linux backend (`linux_ddcutil.rs`).
- `power-fallback` beyond the Windows `SC_MONITORPOWER` stub — macOS `pmset` variant
  waits on the macOS backend itself.
- HID++ trigger, Bluetooth trigger, Tauri UI — all confirmed v2 in DECISIONS.md §8.
- Direct NVAPI FFI (replacing the `writeValueToDisplay.exe` shell-out) — needs a
  decision on sourcing the NVIDIA SDK headers under license.
