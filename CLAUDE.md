# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust application that converts a simple USB switch into a KVM (Keyboard, Video, Mouse) solution by watching for USB device connect/disconnect events and automatically switching monitor inputs via DDC/CI commands. The app runs on all computers connected to shared monitors and coordinates input switching.

## Core Architecture

### Main Components

- **`main.rs`** - Entry point with CLI argument parsing using clap
- **`app.rs`** - Main application logic implementing `UsbCallback` trait for USB events
- **`configuration.rs`** - INI file configuration parsing with per-monitor support using serde
- **`display_control.rs`** - DDC/CI monitor control using `ddc-hi` crate
- **`usb.rs`** - USB device monitoring abstraction
- **`input_source.rs`** - Monitor input source definitions (HDMI, DisplayPort, etc.)
- **`platform/`** - Platform-specific implementations:
  - `pnp_detect_libusb.rs` - USB detection for macOS/Linux using libusb
  - `pnp_detect_windows.rs` - USB detection for Windows using WinAPI
  - `wake_displays.rs` - Platform-specific display wake functionality

### Flow

1. App loads configuration from platform-specific INI file location
2. Starts USB device monitoring using platform-specific PnP detection
3. On USB connect/disconnect events matching configured device ID:
   - Enumerates DDC-compatible displays
   - Switches each display to configured input source
   - Optionally executes configured external commands

### Platform Support

Cross-platform with platform-specific dependencies:
- **macOS**: Uses `ddc-macos` for display control
- **Linux**: Uses `ddc-i2c` and requires i2c device permissions
- **Windows**: Uses `ddc-winapi` and `nvapi` for display control

## GUI Requirement (All Platforms)

A GUI application is a **hard requirement for every supported OS** (Windows, macOS, Linux) — not an optional or Windows-only add-on. This supersedes the earlier "Tauri UI is v2/optional, not coupled to the daemon" scoping decision recorded in `DECISIONS.md` #6/#8. The GUI wraps the existing headless daemon; it does not replace the USB-hotplug trigger path.

Required capabilities:
- **Configuration step**: a setup flow that lists detected DDC-compatible monitors together with their monitor codes/IDs, and lists each monitor's available input sources (VCP input-select values), so the connect/disconnect mapping can be built without hand-editing the `.ini` file.
- **MX Keys detection**: the app detects whether a Logitech MX Keys (or its Unifying receiver) is currently connected and surfaces it as a recognized trigger device.
- **Software switching**: the main screen lets the user manually trigger a switch between a monitor's available inputs, in addition to the passive USB-hotplug-triggered switching.
- **Tray/menu-bar minimization**: the app minimizes to the system tray (Windows/Linux) or menu bar (macOS) instead of quitting, keeping the daemon logic running in the background.

## Development Commands

### Building
```bash
# Install the Tauri CLI once
cargo install tauri-cli --version "^2"

# Debug build/run (opens the GUI)
cargo tauri dev

# Release build
cargo tauri build
```

### Testing
```bash
make test
# or
cargo test
```

### Running
```bash
# Launch the GUI directly (after cargo tauri build)
./target/release/monitorhop
```

## Configuration

Configuration is a JSON file at `%APPDATA%\MonitorHop\config.json` (see `config_path()` in `crates/gui/src-tauri/src/main.rs`; the directory is created if missing). It is resolved against `%APPDATA%`, not the process's working directory, so it can still be found when `tauri-plugin-autostart` launches the app at login with an unpredictable CWD. It is written by the GUI's setup wizard (`crates/gui/frontend/src/wizard/Wizard.tsx`'s final `saveConfig(config)` call) rather than hand-edited by the user.

The schema (`Configuration` in `crates/kvm_core/src/config.rs`) is a single flat object:
- `usb_device` - the trigger USB device ID (vendor:product)
- `mxkeys_usb_device` - optional Logitech MX Keys / Unifying receiver device ID
- `on_usb_connect` / `on_usb_disconnect` - input source to switch to on each event
- `on_usb_connect_source_addr` - optional DDC source-address override
- `on_usb_connect_vcp_code` - optional VCP feature code override (defaults to `0x60`)
- `display_index` - which detected display to control (defaults to `0`)

There is no per-monitor `[monitor1]..[monitor6]` support and no external command execution on connect/disconnect — both were cut during the earlier CLI/daemon-era design and have not been reintroduced.

## Key Dependencies

- **ddc/ddc-hi** - Cross-platform DDC/CI monitor control
- **rusb** - USB device monitoring
- **serde** / **serde_json** - Configuration (de)serialization
- **anyhow** - Error handling
- **simplelog** - Logging to platform-specific log files
- **Tauri** — cross-platform GUI shell (system tray, single-instance, autostart plugins); frontend is React + TypeScript, built via Vite (`crates/gui/frontend`)

## Testing Strategy

The project uses standard Rust unit tests with `cargo test`. Tests cover:
- Configuration parsing and deserialization
- Per-monitor configuration matching
- Input source value conversion

## Build System

Uses a Makefile wrapper around Cargo that:
- On macOS: Creates universal binaries supporting both Intel and ARM architectures
- On other platforms: Uses standard cargo commands
- Includes packaging targets for release distribution