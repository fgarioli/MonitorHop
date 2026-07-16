# Immediate Device Detection — Design Spec

## Problem

The GUI's setup wizard (`crates/gui/frontend/src/wizard/DeviceStep.tsx`, used for both the
KVM-switch and MX Keys device-selection steps) only detects a device via a
before/after snapshot diff: click "Start", physically plug in (or unplug and
replug) the device, click "I plugged it in", and whatever USB id is new gets
offered as a candidate to select.

This breaks down for hardware that is **already connected and stays
connected day-to-day** — exactly this user's real setup (KVM switch and MX
Keys/Unifying receiver are permanent fixtures, never unplugged). Onboarding
currently forces a physical unplug/replug ritual just to get through the
wizard.

**This is a non-negotiable requirement** (user's own words, 2026-07-15): the
app must detect already-connected devices immediately, without requiring
unplug/replug. See memory `project_devicestep_immediate_detection_gap` for
the full history of how this was raised (discovered mid-manual-test of the
2026-07-15 GUI aesthetics/usability plan, which never touched this
detection logic).

## Key finding from exploration

`list_usb_devices()` (Tauri command, backed by `crates/trigger`'s
`read_device_list()` via `rusb`) **already returns every currently-connected
USB device** as `"vendor:product"` hex id strings — it is not itself the
gap. The gap is entirely in `DeviceStep.tsx`'s UX: it deliberately discards
that list and only ever shows a before/after diff. No changes to the
existing device-enumeration logic are needed.

## Decisions made during brainstorming (in order)

1. **Identification is by hex id + a name, not by reading live USB string
   descriptors from the hardware.** Reading `iManufacturer`/`iProduct`
   strings via `rusb` requires opening a device handle per device, which can
   fail for permission reasons (especially HID/composite devices on
   Windows) and needs new, more fragile Rust code. Rejected in favor of a
   maintained lookup table (see below).
2. **No filtering of the connected-device list** (e.g. by USB class code to
   hide hubs) — out of scope, not requested.
3. **Individual failures degrade gracefully, never block.** If a specific
   piece of data can't be resolved, fall back to the next-best
   representation (id-only) rather than hiding the device or blocking the
   whole list.
4. **A maintained "known devices" database, keyed by `vendor:product`
   first, falling back to `vendor` alone, falling back to the raw hex id.**
   Specific-product entries (e.g. "Logitech MX Keys / Unifying Receiver")
   are more useful than vendor-only ("Logitech") when multiple products
   from the same vendor could appear.
5. **The database lives in a JSON file, loaded at runtime, not imported at
   build time.** The explicit reason for choosing JSON over extending the
   existing `usbVendorLabels.ts` TypeScript table was to let new devices be
   added without rebuilding/reinstalling the app.
6. **No in-app management UI for the database.** Direct text-editing of the
   JSON file is sufficient — this is personal-use software, a CRUD screen
   for a handful of entries isn't justified.
7. **Both the new direct-selection list AND the old plug-in/diff flow
   coexist.** The direct list becomes the primary view; the diff flow
   becomes a secondary, collapsed/linked fallback for a genuinely new,
   not-yet-cataloged device the user can't recognize by id alone.
8. **Rust stays a thin file reader; TypeScript does the id→name
   resolution.** A new Tauri command returns the device database's raw
   JSON content (after validating it parses, falling back to seed content
   for a corrupted file without touching it on disk); the frontend's
   already-tested `usbDeviceLabel()` utility (from the earlier GUI
   aesthetics/usability plan) is extended to take the loaded database as a
   parameter, keeping the "smart" fallback logic in the layer that already
   has Vitest coverage for it, consistent with this codebase's existing
   thin-Rust/logic-in-TS split.

## Architecture

### Data flow

1. `DeviceStep` mounts → fires `listUsbDevices()` and `loadDeviceDatabase()`
   in parallel.
2. Once both resolve, the primary view renders: every connected device id,
   labeled via `usbDeviceLabel(id, database)`, each with a "Use this"
   button — selectable immediately, no snapshot/diff required.
3. A secondary, de-emphasized control ("Not sure which one? Plug it in
   now") reveals the pre-existing Start → snapshot → replug → diff flow,
   unchanged, for a device not recognizable from the direct list.

### File format — `%APPDATA%\kvm-switch-gui\device-database.json`

Flat map, same directory convention as `kvm-switch-config.json`
(`config_path()`'s existing pattern). Keys are either a full `vendor:product`
pair or a bare `vendor` prefix; values are display names:

```json
{
  "046d:c52b": "Logitech MX Keys / Unifying Receiver",
  "046d": "Logitech",
  "17e9:6000": "DisplayLink Dock/Switch",
  "17e9": "DisplayLink"
}
```

Seed content is exactly these 4 entries — the same two vendors already
hardcoded in `usbVendorLabels.ts` today (`046d`→Logitech, `17e9`→DisplayLink,
per `docs/DECISIONS.md` §2), just restructured to also carry
product-specific names for the two ids this project has already confirmed
(`046d:c52b`, `17e9:6000`).

### New Tauri command: `load_device_database`

- **File doesn't exist (first run):** write it with the seed content above,
  then return that content.
- **File exists and parses as valid JSON:** return its raw content
  unmodified — Rust never merges/interprets it, no cross-device-id logic on
  the Rust side (decided explicitly: TypeScript owns resolution logic).
- **File exists but fails to parse as JSON:** log a warning, return the seed
  content for this call only — **never overwrite the on-disk file**, so a
  manual edit-in-progress isn't destroyed. The app degrades to seed-only
  names for that session; fixing the file and relaunching (or reopening the
  wizard) picks up the correction.
- This mirrors `config_path()`/`Configuration::load`'s existing
  `%APPDATA%` convention, but unlike `Configuration` (which is only ever
  written by an explicit user action — finishing the wizard), this file is
  auto-seeded on first read since its content is meant to have a sensible
  starting point, not user-authored data entered through a form.

### Frontend changes

`usbDeviceLabel` (in `crates/gui/frontend/src/usbVendorLabels.ts`) gains a
second, optional parameter:

```ts
export function usbDeviceLabel(id: string, database: Record<string, string> = {}): string {
  const vendor = id.split(":")[0]?.toLowerCase();
  const key = id.toLowerCase();
  const name = database[key] ?? (vendor ? database[vendor] : undefined) ?? (vendor ? FALLBACK_NAMES[vendor] : undefined);
  return name ? `${name} (${id})` : id;
}
```

Resolution order: exact `vendor:product` match in the loaded database →
`vendor`-only match in the loaded database → the existing 2-entry hardcoded
`FALLBACK_NAMES` (kept only as a last-resort safety net if the Rust command
fails outright, e.g. an I/O error) → raw hex id.

`DeviceStep.tsx` is restructured: the current Start/snapshot/diff view moves
behind a secondary, de-emphasized entry point; the new primary view is the
full connected-device list (rendered once both `listUsbDevices()` and
`loadDeviceDatabase()` resolve), each entry directly selectable.

`api.ts` gains `loadDeviceDatabase(): Promise<Record<string, string>>`
wrapping the new command (parses the returned JSON string).

## Error handling

- `listUsbDevices()` failure → blocks the view via the existing
  `InlineError` component (unchanged from today).
- `loadDeviceDatabase()` failure → does **not** block the device list; it
  renders with raw hex ids only (as if the database were empty). No
  `InlineError` for this specific failure — it's a degraded-but-usable
  state, not a blocking one.
- Malformed on-disk JSON is caught entirely on the Rust side (see above);
  the frontend never receives unparseable JSON to choke on.

## Testing plan

**Rust:** the seed-or-validate decision is factored into a pure function
(`content: &str -> String`, deciding between the real content and the seed
fallback) that gets unit tests — valid JSON passes through unchanged,
invalid JSON falls back to the seed — following the same pattern as
`capabilities.rs`'s `parse_input_codes` (pure logic tested in isolation from
file I/O). The file I/O wrapper itself (create-if-missing, read) is not
unit-tested, matching the existing untested precedent of
`config_path()`/`Configuration::load`.

**TypeScript:** `usbDeviceLabel` gets new test cases: exact
`vendor:product` match, vendor-only match, hardcoded-fallback match, empty
database (raw id). `DeviceStep.test.tsx` is substantially rewritten to
cover: the new primary flow (connected list renders immediately, direct
selection works), the secondary flow still works (the "not sure which one"
link still reveals and completes the old diff flow), and the
database-load-failure case doesn't block the id list.

## Out of scope

- Reading live USB string descriptors from hardware (rejected, see
  Decision 1).
- Filtering the connected-device list by USB class code.
- An in-app UI for managing the device database.
- Any change to `list_usb_devices()`/`read_device_list()`'s existing
  enumeration logic — it already does everything this feature needs.
