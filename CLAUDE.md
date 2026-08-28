# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**MonitorHop** turns a USB switch into a KVM: it watches for USB device connect/disconnect events and switches monitor inputs over DDC/CI, so the displays follow the keyboard between computers. It is installed on every computer sharing the monitors; each one switches the displays to its own input when it sees the trigger device arrive.

Originally a fork of [haimgel/display-switch](https://github.com/haimgel/display-switch), now rewritten as a Cargo workspace with a Tauri GUI. It shares no configuration format with the original and only the USB-hotplug detection descends closely from it.

## Scope of v0.1.0

**Windows only, NVIDIA only.** This is a deliberate scoping decision made 2026-08-27 and it supersedes any earlier statement in this file that a GUI on all three platforms is a hard requirement:

- **macOS** — code exists and is `cfg`-gated, but has never been compiled or run on a Mac. Not shipped, not validated.
- **Linux** — not implemented. Planned via the `ddcutil` CLI as a subprocess, *not* by linking libddcutil (GPL-2.0, incompatible with this MIT project). See `docs/IMPROVEMENTS.md` #9.
- **AMD/Intel GPUs** — no working backend. `windows_generic.rs` is an intentional stub that returns `Err` and is not selected by any code path. See `docs/IMPROVEMENTS.md` #4.

Cross-platform support remains the product direction, just not the v0.1.0 deliverable. The plan of record is `docs/superpowers/plans/2026-08-27-v1-release-plan.md`.

## Core Architecture

A Cargo workspace of five crates in a ports-and-adapters shape: `kvm_core` depends only on traits (`DdcBackend`, `PowerFallback`, `TriggerSource`), and the adapter crates supply one file per platform behind `#[cfg]`-gated `pub mod` declarations.

### `crates/kvm_core` — the domain

- `config.rs` — the `Configuration` struct and `InputSource`; serde (de)serialization.
- `orchestrator.rs` — `run()`, the **single consumer** of the `DaemonEvent` channel. Every DDC write in the process goes through here, whether triggered by hotplug or a manual switch. Handles the power-fallback retry.
- `monitor_map.rs` — input-code mapping helpers.

### `crates/trigger` — USB hotplug (port: `TriggerSource`)

- `usb_hotplug.rs` — Windows, via `WM_DEVICECHANGE` (winapi).
- `macos_hotplug.rs` — macOS, via `rusb` hotplug. Written, never executed.

### `crates/ddc-backend` — DDC/CI (ports: `DdcBackend` write, `MonitorReader` read)

- `windows_nvapi.rs` — the working write path. Shells out to `tools/writeValueToDisplay.exe`, which reaches NVAPI's raw I2C to override the source address to `0x50`. This override is the whole reason NVIDIA is required; see `docs/DECISIONS.md` #4.
- `windows_generic.rs` — **intentional stub.** Returns `Err`, never selected. `dxva2`/`SetVCPFeature` cannot override the source address, so it is not an equivalent fallback.
- `macos_ioavservice.rs` — macOS write path. Note the name is historical: it does not use IOAVService.
- `ddchi_reader.rs` — the read path (monitor enumeration and input codes for the wizard and tray), via `ddc-hi`.
- `lib.rs` — `ddc_io_lock()`, a process-wide mutex every read and write must hold (concurrent reads silently corrupted writes — `docs/IMPROVEMENTS.md` #3), and the shared `retry()` helper.

### `crates/power-fallback` — display wake (port: `PowerFallback`)

- `windows_monitorpower.rs` (`SC_MONITORPOWER`), `macos_pmset.rs` (`pmset displaysleepnow`). Fallback only, never the primary mechanism.

### `crates/gui/src-tauri` — the Tauri shell

- `main.rs` — entrypoint only: `init_logging`, the Tauri `Builder`, tray setup, `invoke_handler`.
- `app_state.rs` — `AppState` (event sender, tray handles, `pending_rx` for the first-run case).
- `paths.rs` — `app_support_dir`, `config_path`, `default_exe_path`. Resolved against `%APPDATA%` / `current_exe()`, never the CWD, because autostart launches with an unpredictable one.
- `platform/{mod,windows,macos}.rs` — the three per-OS thread spawners (`spawn_switch_trigger`, `spawn_consumer`, `spawn_mxkeys_trigger`). **Adding a platform means adding one file here plus one `cfg` pair in `mod.rs`** — no edits to `main.rs`, `commands.rs` or `device_database.rs`.
- `tray.rs` — `build_quick_switch_items`, shared by startup and `save_config` so the tray stays in sync.
- `commands.rs` — the eight Tauri commands the frontend invokes.
- `device_database.rs` — the seeded USB vendor/product name lookup.

### `crates/gui/frontend` — React + TypeScript + Vite

Four-step setup wizard (`src/wizard/`), `MainScreen.tsx` for manual switching, `api.ts` wrapping the Tauri commands.

### Flow

1. `main()` loads the config; if none exists, it parks the channel receiver in `AppState.pending_rx` so the wizard's first `save_config` can start the pipeline.
2. Trigger threads watch USB hotplug and send `DaemonEvent`s.
3. `orchestrator::run` — the single consumer — applies the switch through `DdcBackend`, falling back to power-cycling if needed, and emits `current-input-changed`.

## Development Commands

The Tauri crate is at `crates/gui/src-tauri`, and its `beforeBuildCommand` builds the frontend from `crates/gui/frontend`.

```bash
cargo install tauri-cli --version "^2"          # once
cd crates/gui/frontend && npm install           # once

cd crates/gui/src-tauri && cargo tauri dev      # run the GUI
cd crates/gui/src-tauri && cargo tauri build    # NSIS installer
```

### Testing

```bash
cargo test --workspace                          # 40 tests
cd crates/gui/frontend && npx tsc --noEmit      # typecheck
cd crates/gui/frontend && npx vitest run        # 52 tests
```

Both suites must stay green. `MANUAL_TEST_GUI.md` is the hardware roteiro — it must be run against the **installed NSIS artifact**, not `cargo tauri dev`, because that is the only thing that exercises resource bundling.

## Configuration

A JSON file at `%APPDATA%\MonitorHop\config.json` (see `config_path()` in `crates/gui/src-tauri/src/paths.rs`; the directory is created if missing). Resolved against `%APPDATA%`, not the CWD, so autostart can still find it. Written by the wizard (`crates/gui/frontend/src/wizard/Wizard.tsx`'s final `saveConfig(config)`), not hand-edited.

The schema (`Configuration` in `crates/kvm_core/src/config.rs`) is a single flat object:
- `usb_device` — the trigger USB device ID (vendor:product)
- `mxkeys_usb_device` — optional Logitech MX Keys / Unifying receiver device ID
- `on_usb_connect` / `on_usb_disconnect` — input source to switch to on each event
- `on_usb_connect_source_addr` — optional DDC source-address override
- `on_usb_connect_vcp_code` — optional VCP feature code override (defaults to `0x60`)
- `display_index` — which detected display to control (defaults to `0`)

There is no per-monitor `[monitor1]..[monitor6]` support and no external command execution — both were cut in the CLI/daemon era and have not been reintroduced. Changing the config requires restarting the app; there is no hot-reload (deliberate, documented in `commands.rs`).

## Key Dependencies

- **ddc-hi 0.4** — DDC/CI *reads* only (monitor and input enumeration), `cfg(any(windows, target_os = "macos"))`. Writes on Windows do **not** go through it; they go through the NVAPI subprocess.
- **ddc-macos 0.2.2** — macOS only.
- **rusb 0.9** — USB hotplug in `trigger` (macOS path).
- **winapi 0.3** — Windows hotplug (`WM_DEVICECHANGE`) and `SC_MONITORPOWER`.
- **Tauri 2** — GUI shell, with the tray-icon feature and the single-instance and autostart plugins.
- **serde / serde_json**, **anyhow**, **simplelog**, **paste**.

There is no `nvapi` crate dependency — the NVAPI path is a bundled subprocess, not a linked library.

## Documentation map

- `docs/DECISIONS.md` — why things are the way they are; #4 is the DDC/CI source-address finding that the whole Windows write path rests on.
- `docs/IMPROVEMENTS.md` — findings #1-#9 from the 2026-07-17 review. #4 (no non-NVIDIA backend) and #6 (multi-monitor index) are still open.
- `docs/superpowers/plans/2026-08-27-v1-release-plan.md` — the plan of record for v0.1.0.
- `docs/superpowers/specs/` and the other `plans/` entries are **historical records**. They describe what was true when written; do not rewrite names or facts inside them.

## Build System

`Makefile` is a leftover from the CLI era (it still carries macOS `lipo` universal-binary targets). It is not used by the release pipeline. Releases are built by `.github/workflows/release.yml`: a `v*` tag on a Windows runner runs the tests, `cargo tauri build`, assembles `latest.json` for the updater, and publishes the installer plus `SHA256SUMS.txt`.
