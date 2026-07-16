# Immediate Device Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the setup wizard's device-selection step (`DeviceStep.tsx`) show every currently-connected USB device as directly selectable immediately, with a friendly name from a runtime-editable "known devices" database — without requiring the user to unplug/replug already-connected hardware.

**Architecture:** A new thin Rust file-reader (`crates/gui/src-tauri/src/device_database.rs`) seeds and serves a JSON lookup file at `%APPDATA%\kvm-switch-gui\device-database.json` via a new `load_device_database` Tauri command, mirroring `config_path()`'s existing convention. All id→name resolution logic (exact `vendor:product` match → `vendor`-only match → hardcoded safety net → raw id) stays in TypeScript's already-tested `usbDeviceLabel`, extended to accept the loaded database. `DeviceStep.tsx` is restructured so the connected-device list is the primary view; the original plug-in/diff flow becomes a secondary, collapsed fallback.

**Tech Stack:** Rust (`serde_json`, already a dependency of `kvm-switch-gui` — no new dependency), React + TypeScript (existing `usbDeviceLabel`/`InlineError`/Tailwind conventions from the 2026-07-15 GUI aesthetics/usability plan).

## Global Constraints

- Full design rationale lives in `docs/superpowers/specs/2026-07-15-immediate-device-detection-design.md` — read it if any task here seems to lack context.
- No live USB string-descriptor reading from hardware (rejected during brainstorming — device-handle permission risk). Identification is id + a maintained name lookup only.
- No filtering of the connected-device list by USB class code — show everything `list_usb_devices()` returns, unfiltered.
- The device database is a JSON file loaded at **runtime** (not imported at build time) so it can be edited without rebuilding the app.
- No in-app UI for managing the database — direct text-editing of the JSON file is the only supported workflow.
- The old plug-in/diff flow must remain fully functional, as a secondary/collapsed fallback — do not remove it.
- Rust stays a thin file reader: it validates the file parses as JSON (falling back to seed content, without touching the file on disk, if it doesn't) and hands back raw content. All id→name matching/fallback logic lives in TypeScript's `usbDeviceLabel`.
- A failure loading the device database must **never** block the connected-device list — it degrades to raw hex ids, no `InlineError`. A failure listing connected devices (`list_usb_devices`) still blocks via the existing `InlineError`, unchanged from today.
- `cargo build --workspace`/`cargo test --workspace` and `npm run build`/`npm run test` (in `crates/gui/frontend`) must stay green throughout.
- Windows dev environment note: `export PATH="$PATH:/c/Users/nando/.cargo/bin:/c/Users/nando/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin"` is required before any `cargo` command in a fresh shell.

---

## File Structure

New files:
- `crates/gui/src-tauri/src/device_database.rs` — seed content, path resolution, pure validate-or-fallback logic (unit tested), file I/O wrapper (not unit tested, matches `Configuration::load` precedent)

Modified files:
- `crates/gui/src-tauri/src/main.rs` — add `mod device_database;`, register the new command in `invoke_handler!`
- `crates/gui/src-tauri/src/commands.rs` — add `load_device_database` Tauri command
- `crates/gui/frontend/src/usbVendorLabels.ts` — `usbDeviceLabel` gains a `database` parameter; `VENDOR_NAMES` renamed `FALLBACK_NAMES`
- `crates/gui/frontend/src/usbVendorLabels.test.ts` — new test cases for database-aware resolution
- `crates/gui/frontend/src/api.ts` — add `loadDeviceDatabase()`
- `crates/gui/frontend/src/wizard/DeviceStep.tsx` — restructured: connected-device list is primary, old plug-in/diff flow becomes a secondary collapsed `DiffDetectionFlow` component in the same file
- `crates/gui/frontend/src/wizard/DeviceStep.test.tsx` — substantially rewritten for the new structure
- `MANUAL_TEST_GUI.md` — new scenario

---

### Task 1: Rust — `device_database` module (seed, path, pure validation logic)

**Files:**
- Create: `crates/gui/src-tauri/src/device_database.rs`

**Interfaces:**
- Produces: `pub(crate) fn device_database_path() -> std::path::PathBuf`, `pub(crate) fn validate_or_fallback(content: &str) -> String`, `pub(crate) fn load_or_seed(path: &std::path::Path) -> anyhow::Result<String>`, `pub(crate) const SEED_DEVICE_DATABASE: &str` — all consumed by Task 2's Tauri command.

- [ ] **Step 1: Write the failing tests for the pure validation function**

Create `crates/gui/src-tauri/src/device_database.rs` with just the test module and a stub (so the tests compile against a real, if incomplete, function signature):

```rust
pub(crate) fn validate_or_fallback(_content: &str) -> String {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_valid_json_unchanged() {
        let valid = r#"{"046d": "Logitech"}"#;
        assert_eq!(validate_or_fallback(valid), valid);
    }

    #[test]
    fn falls_back_to_seed_for_invalid_json() {
        assert_eq!(validate_or_fallback("not json at all"), SEED_DEVICE_DATABASE);
    }

    #[test]
    fn falls_back_to_seed_for_empty_content() {
        assert_eq!(validate_or_fallback(""), SEED_DEVICE_DATABASE);
    }
}
```

This won't compile yet (`SEED_DEVICE_DATABASE` doesn't exist) — that's expected for this step.

- [ ] **Step 2: Run the tests to confirm they fail**

Run (with the PATH export from Global Constraints first): `cargo test -p kvm-switch-gui device_database`
Expected: FAIL to compile — `cannot find value SEED_DEVICE_DATABASE in this scope` (and `unimplemented!()` would panic if it got that far).

- [ ] **Step 3: Implement the full module**

Replace `crates/gui/src-tauri/src/device_database.rs` with:

```rust
//! Reads (and seeds, on first run) the user-maintained "known USB devices"
//! lookup the setup wizard uses to show friendly names for
//! already-connected devices immediately, without requiring an
//! unplug/replug diff. See
//! docs/superpowers/specs/2026-07-15-immediate-device-detection-design.md.
//!
//! Deliberately a thin Rust reader: this module never interprets the
//! JSON's keys/values (vendor:product vs vendor-only, which name wins) —
//! that resolution logic lives in the frontend's `usbDeviceLabel`, which
//! already has Vitest coverage for it. Rust's only jobs are: find the
//! file, seed it with sensible defaults if missing, and never hand back
//! unparseable JSON.

use std::path::{Path, PathBuf};

/// Seed content written on first run — the same two vendors already known
/// to this project (docs/DECISIONS.md §2: 046d = Logitech, 17e9 =
/// DisplayLink, the dongle used as "the USB switch"), plus product-specific
/// names for the two ids already confirmed elsewhere in this codebase's
/// tests (046d:c52b, 17e9:6000).
pub(crate) const SEED_DEVICE_DATABASE: &str = r#"{
  "046d:c52b": "Logitech MX Keys / Unifying Receiver",
  "046d": "Logitech",
  "17e9:6000": "DisplayLink Dock/Switch",
  "17e9": "DisplayLink"
}
"#;

/// Same `%APPDATA%\kvm-switch-gui\` directory as `config_path()` in
/// `main.rs` — falls back to a CWD-relative path if `APPDATA` isn't set,
/// matching that function's existing defensive behavior.
pub(crate) fn device_database_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("kvm-switch-gui");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir.join("device-database.json");
        }
    }
    PathBuf::from("device-database.json")
}

/// Returns `content` unchanged if it parses as JSON, otherwise logs a
/// warning and returns the seed content instead — never lets unparseable
/// JSON reach the frontend. Pure and file-system-free so it's unit
/// testable in isolation, mirroring `ddc-backend`'s `parse_input_codes`.
pub(crate) fn validate_or_fallback(content: &str) -> String {
    if serde_json::from_str::<serde_json::Value>(content).is_ok() {
        content.to_string()
    } else {
        log::warn!(
            "device-database.json exists but isn't valid JSON; using built-in defaults for this \
             session without touching the file on disk."
        );
        SEED_DEVICE_DATABASE.to_string()
    }
}

/// Creates the file with the seed content if it doesn't exist yet, then
/// reads and validates whatever is on disk. A corrupted file is never
/// overwritten — only a missing one is seeded.
pub(crate) fn load_or_seed(path: &Path) -> anyhow::Result<String> {
    if !path.exists() {
        std::fs::write(path, SEED_DEVICE_DATABASE)?;
    }
    let content = std::fs::read_to_string(path)?;
    Ok(validate_or_fallback(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_valid_json_unchanged() {
        let valid = r#"{"046d": "Logitech"}"#;
        assert_eq!(validate_or_fallback(valid), valid);
    }

    #[test]
    fn falls_back_to_seed_for_invalid_json() {
        assert_eq!(validate_or_fallback("not json at all"), SEED_DEVICE_DATABASE);
    }

    #[test]
    fn falls_back_to_seed_for_empty_content() {
        assert_eq!(validate_or_fallback(""), SEED_DEVICE_DATABASE);
    }
}
```

This module isn't wired into `main.rs` yet (no `mod device_database;` declaration) — that's Task 2. It won't compile as part of the crate until then, which is fine: this task's own test run (next step) targets the module directly.

Note: `main.rs` needs `mod device_database;` for `cargo test -p kvm-switch-gui device_database` to even find this file. Add that one line now (just the `mod` declaration, nothing else — Task 2 does the actual command wiring):

Edit `crates/gui/src-tauri/src/main.rs`, changing:

```rust
mod commands;
```

to:

```rust
mod commands;
mod device_database;
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cargo test -p kvm-switch-gui device_database`
Expected: `3 passed; 0 failed` (plus a `dead_code` warning for now, since nothing calls `device_database_path()`/`load_or_seed` yet — expected until Task 2 wires them in; do not silence it with `#[allow(dead_code)]`, Task 2 removes the warning by using them).

- [ ] **Step 5: Commit**

```bash
git add crates/gui/src-tauri/src/device_database.rs crates/gui/src-tauri/src/main.rs
git commit -m "feat: add device-database module with seed content and JSON validation"
```

---

### Task 2: Rust — wire the `load_device_database` Tauri command

**Files:**
- Modify: `crates/gui/src-tauri/src/commands.rs`
- Modify: `crates/gui/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `device_database::device_database_path()`, `device_database::load_or_seed()` (Task 1).
- Produces: Tauri command `load_device_database() -> Result<String, String>` — consumed by Task 5's `api.ts` addition.

- [ ] **Step 1: Add the Tauri command**

Edit `crates/gui/src-tauri/src/commands.rs`, adding after `current_input`:

```rust
#[tauri::command]
pub fn load_device_database() -> Result<String, String> {
    crate::device_database::load_or_seed(&crate::device_database::device_database_path())
        .map_err(|err| err.to_string())
}
```

- [ ] **Step 2: Register the command**

Edit `crates/gui/src-tauri/src/main.rs`, in the `invoke_handler!` list:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::list_usb_devices,
            commands::list_monitors,
            commands::list_inputs,
            commands::save_config,
            commands::load_config,
            commands::switch_input,
            commands::current_input,
            commands::load_device_database,
        ])
```

- [ ] **Step 3: Verify the whole workspace builds and tests pass**

Run: `cargo build --workspace && cargo test --workspace`
Expected: success; the `dead_code` warning from Task 1 is gone (the functions are now used); test count is the prior baseline (24) plus this module's 3 new tests = 27, all passing.

- [ ] **Step 4: Manually sanity-check the seed behavior**

Run (PowerShell or Bash — this reads whatever `%APPDATA%` resolves to on this machine, does not require the GUI to be running):

```bash
rm -f "$APPDATA/kvm-switch-gui/device-database.json"
cargo run -p kvm-switch-gui --bin kvm-switch-gui &
sleep 3
cat "$APPDATA/kvm-switch-gui/device-database.json"
kill %1
```

Expected: the file now exists and contains the 4-entry seed JSON from Task 1's `SEED_DEVICE_DATABASE`. (If a `kvm-switch-config.json` already exists on this machine from earlier manual testing, the app will open the main screen instead of the wizard — that's fine, the seed file is written the moment the process's Tauri command layer is available, not only when the wizard's `DeviceStep` actually calls it... note: actually the command only runs when *invoked* by the frontend, so if the wizard never opens `DeviceStep` in this quick smoke run, the file won't be created yet. If so, skip this step's assertion and instead trust Task 5's own manual verification, which does open `DeviceStep` — don't block on this step if the app doesn't reach the wizard.)

- [ ] **Step 5: Commit**

```bash
git add crates/gui/src-tauri/src/commands.rs crates/gui/src-tauri/src/main.rs
git commit -m "feat: add load_device_database Tauri command"
```

---

### Task 3: TypeScript — `usbDeviceLabel` gains a runtime database parameter

**Files:**
- Modify: `crates/gui/frontend/src/usbVendorLabels.ts`
- Modify: `crates/gui/frontend/src/usbVendorLabels.test.ts`

**Interfaces:**
- Produces: `usbDeviceLabel(id: string, database: Record<string, string> = {}): string` — consumed by Task 5's `DeviceStep.tsx`.

- [ ] **Step 1: Write the failing tests**

Replace `crates/gui/frontend/src/usbVendorLabels.test.ts` with (keeps the 4 existing cases — they must keep passing against the renamed `FALLBACK_NAMES` — and adds a new `describe` block for database-aware resolution):

```ts
import { describe, it, expect } from "vitest";
import { usbDeviceLabel } from "./usbVendorLabels";

describe("usbDeviceLabel", () => {
  it("labels the Logitech vendor id used by MX Keys/Unifying", () => {
    expect(usbDeviceLabel("046d:c52b")).toBe("Logitech (046d:c52b)");
  });

  it("labels the DisplayLink vendor id used by this project's USB switch", () => {
    expect(usbDeviceLabel("17e9:6000")).toBe("DisplayLink (17e9:6000)");
  });

  it("is case-insensitive on the vendor id", () => {
    expect(usbDeviceLabel("046D:C52B")).toBe("Logitech (046D:C52B)");
  });

  it("falls back to the raw id for unknown vendors", () => {
    expect(usbDeviceLabel("ffff:0001")).toBe("ffff:0001");
  });
});

describe("usbDeviceLabel with a runtime device database", () => {
  const database: Record<string, string> = {
    "046d:c52b": "Logitech MX Keys / Unifying Receiver",
    "046d": "Logitech",
    "aaaa": "Acme Corp",
  };

  it("prefers an exact vendor:product match in the database", () => {
    expect(usbDeviceLabel("046d:c52b", database)).toBe("Logitech MX Keys / Unifying Receiver (046d:c52b)");
  });

  it("falls back to a vendor-only match in the database", () => {
    expect(usbDeviceLabel("046d:1234", database)).toBe("Logitech (046d:1234)");
  });

  it("falls back to the hardcoded safety net when the database has no match", () => {
    expect(usbDeviceLabel("17e9:6000", database)).toBe("DisplayLink (17e9:6000)");
  });

  it("falls back to the raw id when nothing matches anywhere", () => {
    expect(usbDeviceLabel("ffff:0001", database)).toBe("ffff:0001");
  });

  it("is case-insensitive when matching the incoming id against the database", () => {
    expect(usbDeviceLabel("046D:C52B", database)).toBe("Logitech MX Keys / Unifying Receiver (046D:C52B)");
  });
});
```

- [ ] **Step 2: Run the tests to confirm the new ones fail**

Run: `cd crates/gui/frontend && npm run test -- usbVendorLabels`
Expected: the 4 original tests still pass (current implementation already handles them); the 5 new tests in the second `describe` block FAIL — `usbDeviceLabel` doesn't accept a second argument yet, so `database`-dependent assertions won't match (TypeScript won't even compile the extra argument against the current signature once `vitest` type-checks via `vite-node`, but the test run itself will fail at the assertion level regardless).

- [ ] **Step 3: Implement the database-aware resolution**

Replace `crates/gui/frontend/src/usbVendorLabels.ts`:

```ts
/** Curated, non-exhaustive vendor-id → name lookup, limited to devices this
 * app expects to see: Logitech (MX Keys / Unifying receivers) and
 * DisplayLink (the dongle validated as this project's "USB switch" in
 * docs/DECISIONS.md §2 — vendor 17e9, not a dedicated KVM switch chip).
 * Kept only as a last-resort safety net if the runtime device database
 * (device-database.json, loaded via `loadDeviceDatabase()` in api.ts)
 * can't be read at all — normal operation resolves names from that file
 * instead, which is seeded with these same two vendors on first run. */
const FALLBACK_NAMES: Record<string, string> = {
  "046d": "Logitech",
  "17e9": "DisplayLink",
};

/** Resolves a friendly name for a USB `vendor:product` id. Checks, in
 * order: an exact `vendor:product` entry in `database`, a `vendor`-only
 * entry in `database`, the hardcoded `FALLBACK_NAMES` safety net, then
 * falls back to the raw id. `database` is expected to already have
 * lowercase keys (see `loadDeviceDatabase` in api.ts, which normalizes
 * this at the boundary) — defaults to `{}` so existing callers that don't
 * pass one still work exactly as before. */
export function usbDeviceLabel(id: string, database: Record<string, string> = {}): string {
  const key = id.toLowerCase();
  const vendor = key.split(":")[0];
  const name = database[key] ?? (vendor ? database[vendor] : undefined) ?? (vendor ? FALLBACK_NAMES[vendor] : undefined);
  return name ? `${name} (${id})` : id;
}
```

- [ ] **Step 4: Run the tests to confirm they all pass**

Run: `cd crates/gui/frontend && npm run test -- usbVendorLabels`
Expected: `9 passed` (4 original + 5 new).

- [ ] **Step 5: Run the full frontend suite and build to confirm no regressions**

Run: `cd crates/gui/frontend && npm run test && npm run build`
Expected: all tests pass (note: `DeviceStep.test.tsx` still calls `usbDeviceLabel` indirectly through the old `DeviceStep.tsx`, which is untouched until Task 5 — it only ever calls `usbDeviceLabel(id)` with one argument, which the new default-parameter signature still supports, so no other file should break); build succeeds.

- [ ] **Step 6: Commit**

```bash
git add crates/gui/frontend/src/usbVendorLabels.ts crates/gui/frontend/src/usbVendorLabels.test.ts
git commit -m "feat: extend usbDeviceLabel to resolve names from a runtime device database"
```

---

### Task 4: `MANUAL_TEST_GUI.md` — new scenario

**Files:**
- Modify: `MANUAL_TEST_GUI.md`

**Interfaces:** none (docs only).

- [ ] **Step 1: Add the new scenario**

Edit `MANUAL_TEST_GUI.md`, inserting after the existing item 9 (the "Friendly labels" scenario added by the 2026-07-15 GUI aesthetics/usability plan) in the wizard flow section:

```markdown
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
    confirm the connected-device list still appears (with raw hex ids
    instead of names, since the corrupted file can't be used) and nothing
    crashes or shows a blocking error. Restore the file afterward.
```

Renumber the former item 10 (tray quick-switch check, from the earlier plan's renumbering) to item 14, and continue renumbering subsequent items accordingly.

- [ ] **Step 2: Commit**

```bash
git add MANUAL_TEST_GUI.md
git commit -m "docs: add manual-test scenarios for immediate device detection"
```

---

### Task 5: `DeviceStep.tsx` — restructure to show connected devices immediately

**Files:**
- Modify: `crates/gui/frontend/src/api.ts`
- Modify: `crates/gui/frontend/src/wizard/DeviceStep.tsx`
- Modify: `crates/gui/frontend/src/wizard/DeviceStep.test.tsx`

**Interfaces:**
- Consumes: `usbDeviceLabel(id, database)` (Task 3), `load_device_database` Tauri command (Task 2).
- Produces: `loadDeviceDatabase(): Promise<Record<string, string>>` in `api.ts`. `DeviceStep`'s external props (`label`, `onSelected`, `onSkip?`, `onBack?`) are unchanged — this is an internal restructure, not a prop-contract change, so `Wizard.tsx` needs no changes.

- [ ] **Step 1: Add the `api.ts` wrapper**

Edit `crates/gui/frontend/src/api.ts`, adding after `currentInput`:

```ts
/** Loads the runtime "known USB devices" lookup (device-database.json),
 * normalizing all keys to lowercase so `usbDeviceLabel`'s lookups don't
 * need to worry about casing a human might use when hand-editing the
 * file. */
export const loadDeviceDatabase = () =>
  invoke<string>("load_device_database").then((raw) => {
    const parsed = JSON.parse(raw) as Record<string, string>;
    return Object.fromEntries(Object.entries(parsed).map(([key, value]) => [key.toLowerCase(), value]));
  });
```

- [ ] **Step 2: Write the failing tests for the restructured component**

Replace `crates/gui/frontend/src/wizard/DeviceStep.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { DeviceStep } from "./DeviceStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

/** Default mock: no connected devices, empty database — used by tests that
 * don't care about the fetched content, just need the component to mount
 * without throwing. */
function mockEmptyInvoke() {
  vi.mocked(invoke).mockImplementation((cmd: string) =>
    cmd === "list_usb_devices" ? Promise.resolve([]) : Promise.resolve("{}"),
  );
}

describe("DeviceStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows already-connected devices immediately, with friendly names from the database", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_usb_devices") return Promise.resolve(["17e9:6000", "ffff:0001"]);
      if (cmd === "load_device_database") return Promise.resolve(JSON.stringify({ "17e9:6000": "DisplayLink Dock/Switch" }));
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);

    expect(await screen.findByText("DisplayLink Dock/Switch (17e9:6000)")).toBeInTheDocument();
    expect(screen.getByText("ffff:0001")).toBeInTheDocument();
  });

  it("calls onSelected when a connected device's Use this button is clicked", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) =>
      cmd === "list_usb_devices" ? Promise.resolve(["046d:c52b"]) : Promise.resolve("{}"),
    );
    const onSelected = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={onSelected} />);

    fireEvent.click(await screen.findByText("Use this"));
    expect(onSelected).toHaveBeenCalledWith("046d:c52b");
  });

  it("shows an inline error when listing connected devices fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) =>
      cmd === "list_usb_devices" ? Promise.reject("USB enumeration failed") : Promise.resolve("{}"),
    );
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("USB enumeration failed");
  });

  it("still shows the connected-device list with raw ids when the database fails to load", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) =>
      cmd === "list_usb_devices" ? Promise.resolve(["17e9:6000"]) : Promise.reject("no database"),
    );
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(await screen.findByText("17e9:6000")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("reveals the plug-in/diff flow and completes it, for a device not in the connected list", async () => {
    let listCallCount = 0;
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_usb_devices") {
        listCallCount += 1;
        return Promise.resolve(listCallCount === 1 ? ["17e9:6000"] : ["17e9:6000", "aaaa:bbbb"]);
      }
      if (cmd === "load_device_database") return Promise.resolve("{}");
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    const onSelected = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={onSelected} />);

    await screen.findByText("17e9:6000");
    fireEvent.click(screen.getByText("Not sure which one? Plug it in now"));
    fireEvent.click(screen.getByText("I plugged it in"));

    fireEvent.click(await screen.findByText("aaaa:bbbb"));
    expect(onSelected).toHaveBeenCalledWith("aaaa:bbbb");
  });

  it("calls onBack when the back button is clicked", async () => {
    mockEmptyInvoke();
    const onBack = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });

  it("renders no back button when onBack is not provided", async () => {
    mockEmptyInvoke();
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(screen.queryByLabelText("Back")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run the tests to confirm they fail**

Run: `cd crates/gui/frontend && npm run test -- DeviceStep`
Expected: FAIL — the current `DeviceStep.tsx` still requires clicking "Start" before anything renders, has no "Not sure which one?" link, and `loadDeviceDatabase` doesn't exist in `api.ts` yet (Step 1 already added it, so this part compiles — the failures are behavioral, from the component itself).

- [ ] **Step 4: Implement the restructured component**

Replace `crates/gui/frontend/src/wizard/DeviceStep.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ChevronLeft, Usb } from "lucide-react";
import { listUsbDevices, loadDeviceDatabase } from "../api";
import { usbDeviceLabel } from "../usbVendorLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  label: string;
  onSelected: (deviceId: string) => void;
  onSkip?: () => void;
  onBack?: () => void;
}

/** Primary view: every USB device connected right now, labeled via the
 * runtime device database, directly selectable — no unplug/replug
 * required (see
 * docs/superpowers/specs/2026-07-15-immediate-device-detection-design.md).
 * `DiffDetectionFlow` below stays available as a secondary, collapsed
 * fallback for a genuinely new, uncataloged device. */
export function DeviceStep({ label, onSelected, onSkip, onBack }: Props) {
  const [connected, setConnected] = useState<string[] | null>(null);
  const [database, setDatabase] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [showDiffFlow, setShowDiffFlow] = useState(false);

  useEffect(() => {
    listUsbDevices()
      .then(setConnected)
      .catch((err) => setError(String(err)));
    loadDeviceDatabase()
      .then(setDatabase)
      .catch(() => {
        // A failed name lookup degrades to raw ids, not a blocking error —
        // the device list itself comes from listUsbDevices() above, which
        // has its own independent error handling.
      });
  }, []);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        {onBack && (
          <button
            onClick={onBack}
            aria-label="Back"
            className="rounded-md p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          >
            <ChevronLeft size={18} />
          </button>
        )}
        <h2 className="flex items-center gap-2 text-base font-semibold text-neutral-900 dark:text-neutral-100">
          <Usb size={18} className="text-emerald-600" />
          {label}
        </h2>
      </div>

      {connected === null && !error && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">Detecting connected devices…</p>
      )}

      {connected !== null && (
        <ul className="flex flex-col gap-2">
          {connected.map((id) => (
            <li
              key={id}
              className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
            >
              <span className="text-sm">{usbDeviceLabel(id, database)}</span>
              <button
                onClick={() => onSelected(id)}
                className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700"
              >
                Use this
              </button>
            </li>
          ))}
        </ul>
      )}

      {onSkip && (
        <button
          onClick={onSkip}
          className="self-start rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
        >
          Skip
        </button>
      )}

      <InlineError message={error} />

      {!showDiffFlow && (
        <button
          onClick={() => setShowDiffFlow(true)}
          className="self-start text-sm text-neutral-500 underline hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200"
        >
          Not sure which one? Plug it in now
        </button>
      )}

      {showDiffFlow && <DiffDetectionFlow existingIds={connected ?? []} onSelected={onSelected} />}
    </div>
  );
}

/** The original "plug it in, click the one that appeared" flow, kept as a
 * fallback for a device not recognizable from the direct connected-device
 * list above. Snapshots against `existingIds` (the list `DeviceStep`
 * already fetched on mount) instead of taking its own fresh "before"
 * snapshot, so revealing this flow needs no extra network round-trip. */
function DiffDetectionFlow({ existingIds, onSelected }: { existingIds: string[]; onSelected: (deviceId: string) => void }) {
  const [candidates, setCandidates] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [checked, setChecked] = useState(false);

  const detectNew = async () => {
    setError(null);
    try {
      const after = await listUsbDevices();
      const beforeSet = new Set(existingIds);
      setCandidates(after.filter((id) => !beforeSet.has(id)));
      setChecked(true);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="flex flex-col gap-2 border-t border-neutral-200 pt-3 dark:border-neutral-700">
      <p className="text-sm text-neutral-600 dark:text-neutral-400">
        Plug in (or unplug and replug) the device, then click below.
      </p>
      <button
        onClick={detectNew}
        className="self-start rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
      >
        I plugged it in
      </button>

      {checked && candidates.length === 0 && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          No new device detected — try unplugging and replugging it.
        </p>
      )}

      {candidates.length > 0 && (
        <ul className="flex flex-col gap-2">
          {candidates.map((id) => (
            <li
              key={id}
              className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
            >
              <span className="text-sm">{id}</span>
              <button
                onClick={() => onSelected(id)}
                className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700"
              >
                Use this
              </button>
            </li>
          ))}
        </ul>
      )}

      <InlineError message={error} />
    </div>
  );
}
```

- [ ] **Step 5: Run the tests to confirm they pass**

Run: `cd crates/gui/frontend && npm run test -- DeviceStep`
Expected: `7 passed`.

- [ ] **Step 6: Run the full frontend suite and build**

Run: `cd crates/gui/frontend && npm run test && npm run build`
Expected: all tests pass. Baseline before this plan started was 33 tests (10 files). Task 3 already took `usbVendorLabels.test.ts` from 4 to 9 tests (33 + 5 = 38). This task takes `DeviceStep.test.tsx` from 4 to 7 tests (38 + 3 = 41 total, still 10 files — no new test files, only two existing ones grew). `tsc && vite build` succeeds. `Wizard.tsx` needs no changes — `DeviceStep`'s external props are unchanged, confirm `npm run build`'s `tsc` step raises no type errors anywhere else.

- [ ] **Step 7: Commit**

```bash
git add crates/gui/frontend/src/api.ts crates/gui/frontend/src/wizard/DeviceStep.tsx crates/gui/frontend/src/wizard/DeviceStep.test.tsx
git commit -m "feat: show already-connected devices immediately in the setup wizard"
```

---

### Task 6: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Full Rust workspace check**

Run: `cargo build --workspace && cargo test --workspace`
Expected: success, all tests green (27: the 24 pre-existing plus this plan's 3 new `device_database` tests).

- [ ] **Step 2: Full frontend check**

Run: `cd crates/gui/frontend && npm run test && npm run build`
Expected: all Vitest tests pass (41, per Task 5's count); `tsc && vite build` succeeds with no type errors.

- [ ] **Step 3: Manual pass in this environment (no real KVM/MX Keys hardware required)**

Run: `cargo tauri dev`
Confirm, without needing the actual KVM switch or MX Keys hardware: any USB devices already connected to this machine (even an internal webcam, a keyboard, anything `list_usb_devices()` picks up) appear immediately in the switch-device step's list, each showing either a database name + id or just the raw id; the "Not sure which one? Plug it in now" link reveals the old flow correctly; deleting/corrupting `%APPDATA%\kvm-switch-gui\device-database.json` and reopening the step still shows the id list (degraded to raw ids, no blocking error).

- [ ] **Step 4: Real-hardware manual test (user must run this)**

Walk through the new `MANUAL_TEST_GUI.md` scenarios (Task 4) with the real KVM switch and MX Keys/Unifying receiver already connected, per `docs/DECISIONS.md`. This is the acceptance gate this whole feature exists to satisfy — **do not consider this plan done until the user confirms the wizard no longer requires unplugging their permanently-connected hardware.**
