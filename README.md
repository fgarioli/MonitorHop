# MonitorHop

Your keyboard hops between computers. Your monitors should follow.

MonitorHop watches for a USB device connecting or disconnecting — a USB switch,
or a Logitech Unifying receiver moving between machines — and switches your
monitors to the matching input over DDC/CI. Press the button on a $30 USB
switch, and the screens come with you.

Install it on every computer sharing the monitors. Each one switches the
displays to *its own* input when it sees the keyboard arrive.

## Requirements

Read these before downloading — v0.1.0 is deliberately narrow.

| | |
|---|---|
| **OS** | Windows 10 or 11. macOS and Linux are not supported yet (see [Status](#status)). |
| **GPU** | **NVIDIA, and it must be the active GPU.** See [Why NVIDIA](#why-nvidia). |
| **Monitor** | Must support DDC/CI, and it must be enabled in the monitor's own OSD menu. Many monitors ship with it off. |
| **Trigger** | Any USB device that moves between computers: a USB switch, or an MX Keys / Unifying receiver. |

## Install

Download the installer from the [latest release][releases], run it, and follow
the setup wizard.

It installs for the current user only — no administrator prompt, and nothing is
written outside your own profile.

Once installed, the app checks for a newer release on startup and offers it in
a banner. Updates are signed, and nothing installs without you clicking the
button — it will not restart itself out from under you.

### About the SmartScreen warning

The installer is not code-signed, so Windows will show **"Windows protected your
PC"**. To proceed: click **More info**, then **Run anyway**.

A code-signing certificate costs a few hundred dollars a year, which is hard to
justify for a project like this one. Instead, every release publishes a
`SHA256SUMS.txt` next to the installer so you can verify the download yourself:

```powershell
Get-FileHash .\MonitorHop_0.1.0_x64-setup.exe -Algorithm SHA256
```

Compare the output against the release's `SHA256SUMS.txt`. If they match, the
file is exactly what the build produced.

## Setup

On first launch, a wizard walks you through four steps:

1. **Trigger device** — pick the USB device that moves between computers.
   Devices already plugged in show up immediately; unplug and replug to
   identify one you're unsure about.
2. **MX Keys** — optionally pick a Logitech MX Keys or Unifying receiver, so
   the app can show you whether the keyboard is currently on this machine.
3. **Monitor** — pick which detected display to control.
4. **Inputs** — pick which input to switch to when the device connects, and
   which when it disconnects.

Step 4 has an **Advanced** section for monitors that need a non-standard DDC
recipe: a source-address override and a VCP feature code override. Most
monitors don't need either. If switching silently does nothing, that section is
where the fix lives — see [DECISIONS.md](docs/DECISIONS.md) #4 for the
reasoning and a worked example (LG 34GL750).

## Using it

- **Automatic** — plug the trigger device in and the monitors switch. That's
  the whole point.
- **Manual** — the main window lists the monitor's inputs; click one to switch.
  The tray menu has the same list.
- **Tray** — closing the window minimises to the system tray; the app keeps
  running so hotplug switching still works. Quit from the tray menu.
- **Autostart** — the app registers itself to start at login.

Configuration lives at `%APPDATA%\MonitorHop\config.json`. It's written by the
wizard; there's no need to edit it by hand.

## Why NVIDIA

The hard part of DDC/CI on Windows isn't sending the command — it's the **I2C
source address**. Windows' own `SetVCPFeature` API forces source address `0x51`
and gives you no way to change it. Some monitors ignore input-switch commands
that arrive that way.

Overriding it to `0x50` is what makes switching work, and reaching that override
means going around the Windows API. MonitorHop does it through NVIDIA's NVAPI,
via a small helper tool. AMD's ADL exposes raw I2C on *discrete* GPUs, but that
path isn't implemented and isn't validated on integrated graphics at all.

So: no active NVIDIA GPU, no switching. On a laptop in iGPU/Eco mode, switching
to the NVIDIA GPU is currently the only known fix. The app detects this case and
says so rather than failing silently.

## Status

v0.1.0 is a real, working release within a deliberately narrow scope. What isn't
there yet:

- **macOS** — the code exists and is cfg-gated, but has never been compiled or
  run on a Mac. Not shipped.
- **Linux** — not implemented at all. Planned via the `ddcutil` CLI rather than
  linking libddcutil, which is GPL-2.0 and would be incompatible with this
  project's MIT license.
- **AMD / Intel graphics** — no working backend. See [Why NVIDIA](#why-nvidia).
- **Multi-monitor** — one display is controlled. The display-index handling in a
  multi-monitor NVIDIA setup is a known open risk
  ([IMPROVEMENTS.md](docs/IMPROVEMENTS.md) #6).
- **Changing your configuration** requires restarting the app; there's no
  hot-reload.

The version number reflects this. 1.0.0 is reserved for when that matrix fills
in. [IMPROVEMENTS.md](docs/IMPROVEMENTS.md) tracks the open items and
[DECISIONS.md](docs/DECISIONS.md) records why things are the way they are.

## Building from source

```bash
cargo install tauri-cli --version "^2"    # once

cd crates/gui/frontend && npm install && cd ../../..
cd crates/gui/src-tauri && cargo tauri dev     # run
cd crates/gui/src-tauri && cargo tauri build   # release installer
```

Tests:

```bash
cargo test --workspace                             # Rust
cd crates/gui/frontend && npx vitest run           # frontend
```

## Credits

MonitorHop is a fork of **[display-switch][upstream]** by Haim Gelfenbeyn, which
originated the idea of driving DDC/CI input switching from USB hotplug events.
The USB hotplug detection in particular descends closely from that project. It
has since been rewritten around a Tauri GUI, a setup wizard and an NVAPI-based
write path, and no longer shares the original's configuration format.

`tools/writeValueToDisplay.exe` is **[NVapi-write-value-to-monitor][nvapitool]**
by kaleb422, which does the actual NVAPI I2C write. Bundled unmodified, with
thanks. It carries no license statement of its own; if you are the author and
would like it removed or relicensed, please open an issue.

## License

MIT — see [LICENSE](LICENSE). Copyright (c) 2020 Haim Gelfenbeyn (original
work), copyright (c) 2026 Fernando Garioli (this fork).

[releases]: https://github.com/fgarioli/MonitorHop/releases/latest
[upstream]: https://github.com/haimgel/display-switch
[nvapitool]: https://github.com/kaleb422/NVapi-write-value-to-monitor
