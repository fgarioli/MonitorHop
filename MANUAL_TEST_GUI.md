# Manual Test: GUI wizard, manual switching, tray, MX Keys status

Run this after `cargo tauri build --debug` succeeds, on the real hardware
described in `DECISIONS.md` (LG 34GL750, NVIDIA GPU, USB switch
17e9:6000, MX Keys with Unifying receiver).

## Setup

1. Delete any existing `%APPDATA%\kvm-switch-gui\kvm-switch-config.json` to
   force the wizard on first launch.
2. Confirm `tools/writeValueToDisplay.exe` exists (relative to the repo root
   for `cargo tauri dev`; see `default_exe_path()` in
   `crates/gui/src-tauri/src/main.rs` for the exe-relative resolution a real
   installed build uses instead).

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
6. Confirm `%APPDATA%\kvm-switch-gui\kvm-switch-config.json` now exists and
   contains the selected `usb_device`, `mxkeys_usb_device`,
   `on_usb_connect`, `display_index`.
7. **Back navigation:** on the input-mapping step, click the new back arrow
   (top-left). Confirm it returns to the monitor step with the previously
   selected monitor showing a checkmark. Click back again to the MX Keys
   step, then again to the switch-device step; confirm the switch-device and
   MX Keys steps restart their plug-detection flow from scratch (this is
   expected — see this plan's Task 8 back-navigation design).
8. **Inline errors:** temporarily unplug the monitor's DDC connection (or
   otherwise make a DDC call fail) and confirm the wizard shows a red inline
   error message anchored under the relevant step, not a floating toast, and
   that it clears on the next successful action.
9. **Friendly labels:** confirm the switch-device/MX-Keys candidate list
   shows "Logitech (046d:c52b)" or "DisplayLink (17e9:6000)" style labels
   instead of raw hex vendor:product ids, and the input-mapping step's
   dropdowns show "DisplayPort 1"/"HDMI 1" instead of `0xf`/`0x11`.
10. **Immediate device detection:** with the KVM switch and MX Keys receiver
    already plugged in (do NOT unplug them first), open the switch-device
    step. Confirm both devices appear immediately in a list, each labeled
    with a friendly name and id (e.g. "DisplayLink Dock/Switch
    (17e9:6000)"), with no need to click "Start" or replug anything.
    Confirm clicking "Use this" on one selects it and advances the wizard.
11. **Diff-flow fallback still works:** on the same step, click "Not sure
    which one? Plug it in now"; confirm the old plug-in-now flow still
    appears and completes correctly for a device you physically
    unplug/replug.
12. **Device database editing:** close the app, open
    `%APPDATA%\kvm-switch-gui\device-database.json` in a text editor,
    confirm it contains the 4 seeded entries (046d:c52b, 046d, 17e9:6000,
    17e9). Add a new `"vendor:product": "Some Name"` entry for any other
    device you have, save, relaunch the app, and confirm that device now
    shows the new name in the wizard's connected-device list.
13. **Corrupted database degrades gracefully:** with the app closed, edit
    `device-database.json` to contain invalid JSON (e.g. delete a closing
    brace) and save. Relaunch the app and open the switch-device step;
    confirm the connected-device list still appears with the seeded devices
    (DisplayLink Dock/Switch and Logitech MX Keys / Unifying Receiver) still
    showing their friendly names, the custom device added in item 12 now
    shows as a raw hex id (since the corrupted file's custom entry is lost
    and the fallback uses only seeded names), and nothing crashes or shows a
    blocking error. Restore the file afterward.
14. **Without restarting the app**, right-click the tray icon; confirm the
    "Switch to 0x..." quick-switch items are now present (they weren't there
    before the wizard finished, since no config existed at startup). Click
    one; confirm the monitor switches. Then physically toggle the USB switch;
    confirm the monitor switches via the hardware trigger path too. This is
    the first-run regression check: before the final-review fix, the switch
    pipeline never started until the next restart because the `DaemonEvent`
    receiver was silently dropped when no config existed at process launch.

## Main screen

1. Confirm the main screen loads (not the wizard) on a second launch.
2. Confirm the MX Keys status line reflects reality: unplug the receiver,
   confirm it flips to "not connected" within a few seconds; replug it,
   confirm it flips back.
3. Confirm the input that matches the monitor's actual current source shows
   an "Active" (disabled) button and a highlighted border, without clicking
   anything — this comes from the new `current_input` DDC read, not from
   memory of the last button clicked. Manually switch the monitor's input
   using the monitor's own physical buttons/remote (bypassing this app
   entirely), then reopen or refresh the main screen; confirm the
   highlighted "Active" input updates to match reality.
4. Click "Switch" next to `0x11`; confirm the monitor switches to HDMI1.
5. Click "Switch" next to `0xF`; confirm the monitor switches back to
   DisplayPort1.
6. Physically toggle the USB switch; confirm the monitor still switches via
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
