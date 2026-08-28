# Release gate: end-to-end test of the installed build

This is the **blocking gate for tagging v0.1.0** (step 5 of
`docs/superpowers/plans/2026-08-27-v1-release-plan.md`).

It runs against the **installed NSIS artifact**, never against
`cargo tauri dev`. That distinction is the whole point: the dev build resolves
`tools/writeValueToDisplay.exe` relative to the repo, so it cannot detect a
packaging mistake. Only an installed build exercises the exe-relative lookup in
`default_exe_path()` (`crates/gui/src-tauri/src/paths.rs`) against the copy NSIS
actually placed on disk.

Every step is pass/fail. **A single failure blocks the tag** — fix it, rebuild,
and start again from Part 1.

## Prerequisites

- The hardware from `docs/DECISIONS.md`: LG 34GL750, active NVIDIA GPU, USB
  switch `17e9:6000`, MX Keys with Unifying receiver.
- DDC/CI enabled in the monitor's own OSD menu.
- The installer, either from `cargo tauri build` (output at
  `target/release/bundle/nsis/MonitorHop_0.1.0_x64-setup.exe`) or downloaded
  from the GitHub release.

### Back up your real configuration first

The test deliberately deletes the config to force the wizard. Save it:

```powershell
Copy-Item "$env:APPDATA\MonitorHop\config.json" "$env:USERPROFILE\Desktop\config.json.bak"
```

Restore it at the end if you want your working setup back.

## Part 1 — Install

| # | Step | Expected |
|---|---|---|
| 1.1 | Verify the download hash: `Get-FileHash .\MonitorHop_0.1.0_x64-setup.exe -Algorithm SHA256` | Matches the release's `SHA256SUMS.txt` (skip when testing a local build) |
| 1.2 | Remove any previous install: uninstall from **Settings → Apps**, then delete `%APPDATA%\MonitorHop\` and `%LOCALAPPDATA%\MonitorHop\` if they survive | Both directories gone — this is a first-install test, not an upgrade |
| 1.3 | Double-click the installer | Windows shows **"Windows protected your PC"**; **More info → Run anyway** proceeds. This confirms the README's SmartScreen workaround is accurate |
| 1.4 | Complete the installer | **No UAC / administrator prompt appears at any point** |
| 1.5 | `ls "$env:LOCALAPPDATA\MonitorHop"` | Contains `monitorhop.exe` (lowercase — the cargo binary name) |
| 1.6 | `ls "$env:LOCALAPPDATA\MonitorHop\tools"` | **Contains `writeValueToDisplay.exe`.** This is the packaging fix under test; if it is missing, switching will fail in Part 3 |
| 1.7 | Check **Settings → Apps** | "MonitorHop" is listed with publisher "Fernando Garioli" |

## Part 2 — First run and the wizard

Launch from the Start menu, not from `target/`.

1. The **wizard** appears (no config exists yet).
2. **Trigger device step:** with the USB switch and the MX Keys receiver
   already plugged in, confirm both appear **immediately** in a list with
   friendly labels ("DisplayLink Dock/Switch (17e9:6000)", "Logitech MX Keys /
   Unifying Receiver (046d:c52b)") — no "Start" click, no replugging.
   Select the switch.
3. **Fallback flow:** before moving on, click "Not sure which one? Plug it in
   now", physically unplug/replug a device, and confirm the older diff-based
   flow still identifies it correctly.
4. **MX Keys step:** select the Unifying receiver.
5. **Monitor step:** the LG 34GL750 appears by model name or EDID id. Select it.
6. **Input mapping step:** the listed inputs include DisplayPort 1 / HDMI 1 /
   HDMI 2 (`0xF`, `0x11`, `0x12` — DECISIONS.md §2) shown as **friendly names,
   not raw hex**. Set "on connect" to HDMI 1. Click Finish.
7. **Back navigation:** re-enter the wizard steps with the back arrow and
   confirm the monitor step still shows the previous selection checked.
8. Confirm `%APPDATA%\MonitorHop\config.json` now exists and contains
   `usb_device`, `mxkeys_usb_device`, `on_usb_connect`, `display_index`.
   Note the path: `%APPDATA%`, **not** the install directory.
9. **Device database:** quit the app, open
   `%APPDATA%\MonitorHop\device-database.json`, confirm the four seeded
   entries (`046d:c52b`, `046d`, `17e9:6000`, `17e9`). Add a
   `"vendor:product": "Some Name"` line for any other device you own, relaunch,
   and confirm the wizard shows that name.
10. **Corrupted database degrades gracefully:** quit, break the JSON (delete a
    closing brace), relaunch. The device list still appears with the seeded
    names, your custom entry falls back to a raw hex id, and nothing crashes or
    blocks. Restore the file afterwards.

## Part 3 — Switching (the packaging-critical part)

This is what the dev build cannot validate. Every switch here runs
`writeValueToDisplay.exe` **from the install directory**.

1. **Manual switch:** on the main screen, click Switch next to HDMI 1 — the
   monitor changes input. Click DisplayPort 1 — it changes back.
2. **Tray quick-switch without restarting:** right-click the tray icon; the
   "Switch to …" items are present even though no config existed at launch.
   Click one; the monitor switches. (First-run regression check: the
   `DaemonEvent` receiver used to be dropped when the process started
   config-less.)
3. **Hardware trigger:** physically toggle the USB switch. The monitor switches
   on its own.
4. **Active-input read:** the input matching the monitor's real current source
   shows as "Active" and highlighted. Change the input using the monitor's own
   physical buttons, then reopen the window; the highlight follows reality.
5. **MX Keys status:** unplug the receiver — the status line flips to "not
   connected" within a few seconds; replug — it flips back.

If switching fails here but worked under `cargo tauri dev`, the cause is
packaging, not logic: check step 1.6.

## Part 4 — Tray, autostart, reboot

1. Close the window with **X** — the process keeps running (Task Manager) and
   the tray icon stays.
2. Left-click the tray icon — the window restores.
3. Right-click — "Open" and "Quit" both work.
4. Confirm the autostart entry exists:
   `Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"`
   (or the user's Startup folder, depending on how `tauri-plugin-autostart`
   registers it).
5. **Reboot the machine.** After login, MonitorHop is running in the tray, and
   a hardware toggle of the USB switch still switches the monitor. This proves
   the config is found from an unpredictable working directory — the reason
   `config_path()` resolves against `%APPDATA%` rather than the CWD.

## Part 5 — Updater sanity

Only meaningful once a release exists at the configured endpoint.

1. With v0.1.0 installed and the latest release also v0.1.0, launch the app:
   **no update banner appears**, and the log records "no update available".
2. Confirm the log has no `updater unavailable` warning — that would mean the
   plugin failed to initialise rather than finding nothing.

Full install/restart of a newer version is exercised for real at v0.1.1; it
cannot be tested before a second release exists.

## Part 6 — Uninstall

1. Uninstall from **Settings → Apps**. No administrator prompt.
2. `%LOCALAPPDATA%\MonitorHop\` is gone, **including the `tools\` subdirectory**
   (the generated `installer.nsi` deletes the exe and then `RMDir`s `tools`).
3. `%APPDATA%\MonitorHop\config.json` may survive — that is intended, so a
   reinstall keeps the user's setup. Note whether it did.
4. Restore your backed-up config if you want your working setup back.

## Sign-off

| Part | Result | Notes |
|---|---|---|
| 1 Install | | |
| 2 Wizard | | |
| 3 Switching | | |
| 4 Tray/autostart/reboot | | |
| 5 Updater | | |
| 6 Uninstall | | |

Tag `v0.1.0` only when Parts 1–4 and 6 pass. Part 5 is advisory at v0.1.0.

## Known non-goals

- Nothing here is automated. It needs a human to toggle a physical switch and
  look at a monitor.
- macOS and Linux are not covered — they are not shipped in v0.1.0.
- Non-NVIDIA GPUs are not covered; there is no working backend for them.
