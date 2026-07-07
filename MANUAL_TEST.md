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

1. Run the daemon with debug logging **from the repo root** (both
   `display-switch.ini` and `tools/writeValueToDisplay.exe` are resolved
   relative to the current working directory, not the built binary):
   ```
   cargo run -p kvm-switch-daemon -- --debug
   ```
2. With the monitor showing the Windows host (DisplayPort), physically
   toggle the USB switch so the watched device (`17e9:6000`) connects to the
   Windows host's USB bus.
3. Observe in the daemon's log output:
   - `USB device state changed, emitting HostGainedFocus for device ...`
   - `Display switched to ... for Connect`
4. Confirm the monitor switches to HDMI1 (Mac) within a few seconds.
5. Repeat steps 2-4 five times in a row to confirm reliability (per
   DECISIONS.md's milestone criterion of "reliably across repeated cycles").

## Known non-goals for this milestone

- The Mac -> Windows direction is handled separately by BetterDisplay
  running on the Mac (see DECISIONS.md #5), not by this daemon.
- No automated test exists for this end-to-end flow — it requires
  physically toggling the USB switch and observing the monitor.
