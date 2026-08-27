# IMPROVEMENTS.md Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every actionable gap identified in `docs/IMPROVEMENTS.md`'s file-by-file review — a macOS config-path bug, a `todo!()` panic risk, a real DDC/CI read/write race, unclear NVAPI failure diagnostics, a missing macOS write-path retry, and a missing wizard UI path for the `source_addr`/`vcp_code` overrides — plus a documentation nudge for the not-yet-built `linux_ddcutil.rs`.

**Architecture:** No new crates or dependencies. Every task is a targeted fix inside the existing workspace (`kvm_core`, `ddc-backend`, `gui`), following patterns already established in the codebase (the existing `retry()` helper, the existing `Configuration` schema, the existing Vitest/cargo-test conventions). The one new shared primitive is a process-wide `ddc_io_lock()` in `ddc-backend`'s root, since the read (`ddc-hi`) and write (NVAPI/IOAVService) paths currently have no way to see each other.

**Tech Stack:** Rust (workspace crates), `anyhow`, `std::sync::OnceLock`/`Mutex`, React + TypeScript (Vite, Vitest, Testing Library), Tauri.

## Global Constraints

- Every new/changed function gets a test **first** (red), then the minimal implementation (green) — no exceptions, per `superpowers:test-driven-development`.
- No new Cargo or npm dependencies for any task in this plan.
- Don't touch `docs/DECISIONS.md` or `docs/IMPROVEMENTS.md` further — both were already updated in the prior session (superseded-item annotations and the findings list itself). Reference them, don't re-edit them.
- Preserve every existing passing test unchanged unless a task explicitly says to modify one — `cargo test` (workspace) and `npm test` (in `crates/gui/frontend`) must stay green after each task.
- IMPROVEMENTS.md #6 (NVAPI-preference heuristic ignoring `display_index` in multi-monitor setups) and #8 (docs drift) are **intentionally not tasks** here: #6 is an accepted, untestable-without-hardware risk already documented inline in `ddchi_reader.rs`; #8 was already resolved directly in `docs/DECISIONS.md` in the prior session. Do not create tasks for either.

---

## File Structure

| File | Change |
|---|---|
| `crates/gui/src-tauri/src/main.rs` | New `app_support_dir()` helper; `config_path()` rewritten to use it |
| `crates/gui/src-tauri/src/device_database.rs` | `device_database_path()` rewritten to use `crate::app_support_dir()` |
| `crates/ddc-backend/src/windows_generic.rs` | `todo!()` → `Err(anyhow!(...))` |
| `crates/ddc-backend/src/lib.rs` | New `ddc_io_lock()` (process-wide mutex) and relocated `retry()` helper |
| `crates/ddc-backend/src/ddchi_reader.rs` | All three `MonitorReader` methods take `ddc_io_lock()`; local `retry()` removed in favor of `crate::retry()` |
| `crates/ddc-backend/src/windows_nvapi.rs` | Takes `ddc_io_lock()`; captures subprocess stdout/stderr for a clearer NVIDIA-GPU-absent hint |
| `crates/ddc-backend/src/macos_ioavservice.rs` | Takes `ddc_io_lock()`; wraps the write in `crate::retry()` |
| `crates/gui/frontend/src/wizard/InputMappingStep.tsx` | New exported `parseHexByte()`; two new optional "Advanced" fields |
| `crates/gui/frontend/src/wizard/InputMappingStep.test.tsx` | New tests for `parseHexByte` and the advanced fields |
| `crates/gui/frontend/src/wizard/Wizard.tsx` | Threads `sourceAddr`/`vcpCode` from `InputMappingStep` into the saved `Configuration` |
| `crates/gui/frontend/src/wizard/Wizard.test.tsx` | Two new tests for the override passthrough/default |
| `crates/ddc-backend/src/lib.rs` (comment only) | `linux_ddcutil.rs` TODO comment updated with the subprocess-not-FFI note |

---

### Task 1: Fix macOS config/device-database path resolution

**Files:**
- Modify: `crates/gui/src-tauri/src/main.rs:43-51`
- Modify: `crates/gui/src-tauri/src/device_database.rs:32-40`
- Test: `crates/gui/src-tauri/src/main.rs` (`#[cfg(test)] mod tests`, same file)

**Interfaces:**
- Produces: `pub(crate) fn app_support_dir() -> Option<std::path::PathBuf>` in `main.rs`'s crate root — resolves and creates the per-OS app-support directory, or `None` if it can't (caller falls back to a CWD-relative path).
- Consumes (Task 3+ don't depend on this, no cross-task interface beyond the above).

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `crates/gui/src-tauri/src/main.rs` (after the existing `default_exe_path_fallback_resolves_to_real_file` test):

```rust
    #[cfg(windows)]
    #[test]
    fn app_support_dir_uses_appdata_on_windows() {
        let dir = app_support_dir().expect("APPDATA should be set in the test environment");
        assert!(dir.ends_with("kvm-switch-gui"));
        assert!(dir.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_support_dir_uses_library_application_support_on_macos() {
        let dir = app_support_dir().expect("HOME should be set in the test environment");
        assert!(dir.ends_with("kvm-switch-gui"));
        assert!(dir.to_string_lossy().contains("Library/Application Support"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kvm-switch-gui app_support_dir --  --nocapture`
Expected: FAIL to compile — `app_support_dir` not found in this scope.

- [ ] **Step 3: Implement `app_support_dir` and rewrite `config_path`**

Replace `crates/gui/src-tauri/src/main.rs:35-51` (the `config_path` function and its doc comment) with:

```rust
/// Resolves and creates the per-OS application-support directory
/// (`%APPDATA%\kvm-switch-gui` on Windows, `$HOME/Library/Application
/// Support/kvm-switch-gui` on macOS), returning `None` if the relevant
/// environment variable isn't set or the directory can't be created —
/// callers fall back to a CWD-relative path in that case. Shared by
/// `config_path` below and `device_database::device_database_path`, so both
/// resolve to the same directory.
pub(crate) fn app_support_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let base = std::env::var("APPDATA").ok().map(std::path::PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var("HOME")
        .ok()
        .map(|home| std::path::PathBuf::from(home).join("Library/Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base: Option<std::path::PathBuf> = None;

    let dir = base?.join("kvm-switch-gui");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Resolves the config file to a stable, OS-independent location rather than
/// the process's current working directory. This matters because
/// `tauri-plugin-autostart` launches the app at login with an unpredictable
/// CWD on both Windows (often `C:\Windows\System32`) and macOS (a
/// `LaunchAgent`'s CWD is similarly not the install directory), so a
/// CWD-relative path would silently fail to find the config on an
/// autostarted run. Falls back to the old CWD-relative behavior if
/// `app_support_dir` returns `None`, which should not happen on a real
/// Windows or macOS machine but keeps this defensive rather than panicking.
pub(crate) fn config_path() -> std::path::PathBuf {
    app_support_dir()
        .map(|dir| dir.join("kvm-switch-config.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("kvm-switch-config.json"))
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run (on Windows): `cargo test -p kvm-switch-gui app_support_dir`
Expected: PASS — `app_support_dir_uses_appdata_on_windows` passes; the macOS test is not compiled on this platform.

- [ ] **Step 5: Rewrite `device_database_path` to reuse the same helper**

Replace `crates/gui/src-tauri/src/device_database.rs:29-40` with:

```rust
/// Same `%APPDATA%\kvm-switch-gui\` (Windows) / `$HOME/Library/Application
/// Support/kvm-switch-gui` (macOS) directory as `config_path()` in
/// `main.rs` — falls back to a CWD-relative path if that directory can't be
/// resolved, matching that function's existing defensive behavior.
pub(crate) fn device_database_path() -> PathBuf {
    crate::app_support_dir()
        .map(|dir| dir.join("device-database.json"))
        .unwrap_or_else(|| PathBuf::from("device-database.json"))
}
```

- [ ] **Step 6: Run the full test suite for this crate**

Run: `cargo test -p kvm-switch-gui`
Expected: PASS — all existing tests (including `default_exe_path_fallback_resolves_to_real_file` and the `device_database` module's `validate_or_fallback`/`load_or_seed` tests, which don't call `device_database_path` directly) plus the two new tests.

- [ ] **Step 7: Commit**

```bash
git add crates/gui/src-tauri/src/main.rs crates/gui/src-tauri/src/device_database.rs
git commit -m "fix: resolve config and device-database paths on macOS, not just Windows

IMPROVEMENTS.md #1: tauri-plugin-autostart launches as a LaunchAgent on
macOS with an unpredictable CWD, same class of problem config_path()
already handled for Windows via %APPDATA%. Both paths now share a single
app_support_dir() helper that also covers $HOME/Library/Application
Support."
```

---

### Task 2: Replace `windows_generic.rs`'s `todo!()` with a returned error

**Files:**
- Modify: `crates/ddc-backend/src/windows_generic.rs:9-13`
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::DdcBackend` trait (unchanged signature).
- Produces: no new public interface — `GenericDdcBackend::set_vcp` now returns `Err` instead of panicking, so any future caller (there is none yet — see IMPROVEMENTS.md #4) can handle the failure instead of crashing the orchestrator's consumer thread.

- [ ] **Step 1: Write the failing test**

Add to `crates/ddc-backend/src/windows_generic.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_vcp_returns_an_error_instead_of_panicking() {
        let backend = GenericDdcBackend;
        let result = backend.set_vcp(0, 0x60, 0x11, None);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ddc-backend set_vcp_returns_an_error_instead_of_panicking` (this workspace has no Cargo feature flags for platform selection — `windows_generic` is compiled automatically on a Windows host via the `#[cfg(windows)]` gate on its `mod` declaration in `lib.rs`)
Expected: FAIL — the test process panics (`todo!()`) instead of returning a normal test failure/pass, visible as a "panicked at" abort in the test runner's output.

- [ ] **Step 3: Implement**

Replace `crates/ddc-backend/src/windows_generic.rs:9-13`:

```rust
impl DdcBackend for GenericDdcBackend {
    fn set_vcp(&self, _monitor_index: u32, _code: u8, _value: u16, _source_addr: Option<u8>) -> Result<()> {
        Err(anyhow::anyhow!(
            "GenericDdcBackend (AMD/ADL fallback) is not implemented yet — see DECISIONS.md #4/#10 \
             and IMPROVEMENTS.md #4. This backend is not currently selected by any code path."
        ))
    }
}
```

(`anyhow::anyhow!` replaces the bare `anyhow!` import if it isn't already imported that way — check the top of the file: it currently has `use anyhow::Result;` only, so either add `anyhow` to that `use` line or call it fully qualified as above. Fully-qualified is simpler here since this is the only place in the file that needs the macro.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ddc-backend set_vcp_returns_an_error_instead_of_panicking`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ddc-backend/src/windows_generic.rs
git commit -m "fix: return an error instead of panicking in GenericDdcBackend::set_vcp

IMPROVEMENTS.md #2: this backend is dead code today, but a todo!() panic
here would kill the orchestrator's single consumer thread (no supervisor/
restart) the moment it's ever wired in."
```

---

### Task 3: Add a process-wide DDC/CI I/O lock shared by reads and writes

**Files:**
- Modify: `crates/ddc-backend/src/lib.rs` (new `ddc_io_lock()`)
- Modify: `crates/ddc-backend/src/ddchi_reader.rs:61-109` (all three `MonitorReader` methods)
- Modify: `crates/ddc-backend/src/windows_nvapi.rs:36-48` (`set_vcp`)
- Modify: `crates/ddc-backend/src/macos_ioavservice.rs:7-24` (`set_vcp`)

**Interfaces:**
- Produces: `pub fn ddc_io_lock() -> std::sync::MutexGuard<'static, ()>` in `ddc-backend`'s crate root — callers hold the returned guard for the full duration of their DDC/CI I/O.
- Consumes: nothing new from other tasks.

- [ ] **Step 1: Write the failing test for the lock itself**

Add to `crates/ddc-backend/src/lib.rs` (new `#[cfg(test)] mod tests` block at the end of the file):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    /// Proves `ddc_io_lock()` actually serializes callers: thread `a` holds
    /// the lock across a sleep, thread `b` starts shortly after and must
    /// block until `a` both entered AND exited its critical section — so the
    /// recorded order can only be a, A, b, never a, b, A.
    #[test]
    fn ddc_io_lock_serializes_concurrent_callers() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let order_a = Arc::clone(&order);
        let order_b = Arc::clone(&order);

        let a = thread::spawn(move || {
            let _guard = ddc_io_lock();
            order_a.lock().unwrap().push('a');
            thread::sleep(Duration::from_millis(50));
            order_a.lock().unwrap().push('A');
        });
        thread::sleep(Duration::from_millis(10));
        let b = thread::spawn(move || {
            let _guard = ddc_io_lock();
            order_b.lock().unwrap().push('b');
        });

        a.join().unwrap();
        b.join().unwrap();

        assert_eq!(*order.lock().unwrap(), vec!['a', 'A', 'b']);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ddc-backend ddc_io_lock_serializes_concurrent_callers`
Expected: FAIL to compile — `ddc_io_lock` not found in this scope.

- [ ] **Step 3: Implement `ddc_io_lock`**

Add near the top of `crates/ddc-backend/src/lib.rs`, after the existing doc comment and before the `DdcBackend` trait:

```rust
use std::sync::{Mutex, MutexGuard, OnceLock};

static DDC_IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Serializes all access to the shared DDC/CI I2C channel. Reads
/// (`MonitorReader`) and writes (`DdcBackend::set_vcp`) contend for the same
/// physical bus regardless of which Rust type issues the call — a real
/// manual-test session found a read landing mid-write silently corrupts the
/// write (screen stayed black, success still reported; see
/// `docs/IMPROVEMENTS.md` #3). Callers must hold the returned guard for the
/// full duration of their DDC/CI I/O, not just part of it.
pub fn ddc_io_lock() -> MutexGuard<'static, ()> {
    DDC_IO_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ddc-backend ddc_io_lock_serializes_concurrent_callers`
Expected: PASS.

- [ ] **Step 5: Wrap the three `DdcHiMonitorReader` methods**

In `crates/ddc-backend/src/ddchi_reader.rs`, add `let _guard = crate::ddc_io_lock();` as the first line of each method body:

```rust
impl MonitorReader for DdcHiMonitorReader {
    fn enumerate(&self) -> Result<Vec<MonitorInfo>> {
        let _guard = crate::ddc_io_lock();
        Ok(Display::enumerate()
            .into_iter()
            .enumerate()
            .map(|(index, display)| MonitorInfo {
                display_index: index as u32,
                id: display.info.id.clone(),
                model_name: display.info.model_name.clone(),
            })
            .collect())
    }

    fn input_codes(&self, display_index: u32) -> Result<Vec<u8>> {
        let _guard = crate::ddc_io_lock();
        retry(3, std::time::Duration::from_millis(50), || {
            let mut displays = Display::enumerate();
            let display = select_display(&mut displays, display_index)?;
            let raw = display
                .handle
                .capabilities_string()
                .map_err(|err| anyhow!("failed to read capabilities for display {}: {:?}", display_index, err))?;
            Ok(parse_input_codes(&String::from_utf8_lossy(&raw)))
        })
    }

    fn current_input(&self, display_index: u32) -> Result<u8> {
        let _guard = crate::ddc_io_lock();
        const INPUT_SELECT: u8 = 0x60;
        retry(3, std::time::Duration::from_millis(50), || {
            let mut displays = Display::enumerate();
            let display = select_display(&mut displays, display_index)?;
            let value = display
                .handle
                .get_vcp_feature(INPUT_SELECT)
                .map_err(|err| anyhow!("failed to read current input for display {}: {:?}", display_index, err))?;
            Ok(value.sl)
        })
    }
}
```

(The `retry` calls above still reference the module-local `retry` function — that's fine for this task; Task 5 relocates it to `lib.rs` and updates these call sites to `crate::retry`.)

- [ ] **Step 6: Wrap `NvapiBackend::set_vcp`**

In `crates/ddc-backend/src/windows_nvapi.rs`, add the guard as the first line:

```rust
impl DdcBackend for NvapiBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        let _guard = crate::ddc_io_lock();
        let args = build_args(monitor_index, code, value, source_addr);
        log::debug!("Running {:?} {:?}", self.exe_path, args);
        let status = Command::new(&self.exe_path).args(&args).status()?;
        if !status.success() {
            return Err(anyhow!(
                "writeValueToDisplay.exe exited with {:?} (args: {:?})",
                status.code(),
                args
            ));
        }
        Ok(())
    }
}
```

(Task 4 rewrites this body further — the guard line added here stays as-is.)

- [ ] **Step 7: Wrap `MacosIoavserviceBackend::set_vcp`**

In `crates/ddc-backend/src/macos_ioavservice.rs`, add the guard as the first line:

```rust
impl DdcBackend for MacosIoavserviceBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        let _guard = crate::ddc_io_lock();
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

(Task 5 rewrites this body further to add retry — the guard line added here stays as-is.)

- [ ] **Step 8: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS — every previously-passing test still passes (the guard is a no-op for correctness in single-threaded tests since `Mutex<()>` is uncontended), plus the new `ddc_io_lock_serializes_concurrent_callers` test.

- [ ] **Step 9: Commit**

```bash
git add crates/ddc-backend/src/lib.rs crates/ddc-backend/src/ddchi_reader.rs crates/ddc-backend/src/windows_nvapi.rs crates/ddc-backend/src/macos_ioavservice.rs
git commit -m "fix: serialize DDC/CI reads and writes behind a shared lock

IMPROVEMENTS.md #3: a real manual-test session found a read (wizard/tray/
current_input) landing mid-write on the same NVAPI I2C channel silently
corrupted the write. Removing the interval poll (already done) treated the
symptom; this adds the actual mutual exclusion, shared by every read and
write entry point in ddc-backend."
```

---

### Task 4: Capture NVAPI subprocess output and surface a clearer no-NVIDIA-GPU hint

**Files:**
- Modify: `crates/ddc-backend/src/windows_nvapi.rs:35-48` (`set_vcp`, post-Task-3 state)
- Test: same file, existing `#[cfg(test)] mod tests` block

**Interfaces:**
- Produces: `fn describe_failure(args: &[String; 4], output: &std::process::Output) -> String` (private to this module) — used by `set_vcp`'s error path.
- Consumes: `build_args` (existing, unchanged).

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `crates/ddc-backend/src/windows_nvapi.rs`:

```rust
    #[test]
    fn describe_failure_includes_captured_output_and_the_nvidia_hint() {
        let output = std::process::Output {
            status: fake_exit_status(1),
            stdout: b"".to_vec(),
            stderr: b"NVAPI initialization failed".to_vec(),
        };
        let args = build_args(0, 0x60, 0x11, None);

        let message = describe_failure(&args, &output);

        assert!(message.contains("NVAPI initialization failed"));
        assert!(message.contains("NVIDIA GPU"));
    }

    fn fake_exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ddc-backend describe_failure_includes_captured_output_and_the_nvidia_hint`
Expected: FAIL to compile — `describe_failure` not found in this scope.

- [ ] **Step 3: Implement `describe_failure` and switch `set_vcp` to capture output**

Replace `crates/ddc-backend/src/windows_nvapi.rs:35-48` (the `impl DdcBackend for NvapiBackend` block, as left by Task 3) with:

```rust
fn describe_failure(args: &[String; 4], output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "writeValueToDisplay.exe exited with {:?} (args: {:?}). stdout: {:?}, stderr: {:?}. \
         This tool depends on an NVIDIA GPU/driver (NVAPI) being available — if this machine has \
         switched to an AMD/integrated-only graphics mode (e.g. a laptop's \"Eco\"/iGPU-only \
         setting), switching back to the NVIDIA GPU is currently the only known fix (see \
         DECISIONS.md #4/#10, IMPROVEMENTS.md #4).",
        output.status.code(),
        args,
        stdout.trim(),
        stderr.trim(),
    )
}

impl DdcBackend for NvapiBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        let _guard = crate::ddc_io_lock();
        let args = build_args(monitor_index, code, value, source_addr);
        log::debug!("Running {:?} {:?}", self.exe_path, args);
        let output = Command::new(&self.exe_path).args(&args).output()?;
        if !output.status.success() {
            return Err(anyhow!(describe_failure(&args, &output)));
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ddc-backend describe_failure_includes_captured_output_and_the_nvidia_hint`
Expected: PASS.

- [ ] **Step 5: Run the full module test suite to confirm no regression**

Run: `cargo test -p ddc-backend`
Expected: PASS — `build_args_uses_validated_default_source_addr`, `build_args_honors_explicit_source_addr_override`, and the two `ddc_io_lock`/`describe_failure` tests all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/ddc-backend/src/windows_nvapi.rs
git commit -m "fix: capture writeValueToDisplay.exe output and hint at missing NVIDIA GPU

IMPROVEMENTS.md #4: a Windows machine with no active NVIDIA GPU (e.g. a
laptop's iGPU-only/Eco mode) previously surfaced only a bare exit-code
error. This captures stdout/stderr and adds an explicit hint pointing at
the GPU-mode cause, without adding GPU-vendor-detection machinery."
```

---

### Task 5: Add retry to the macOS write path, relocating the shared `retry()` helper

**Files:**
- Modify: `crates/ddc-backend/src/lib.rs` (relocate `retry()` here)
- Modify: `crates/ddc-backend/src/ddchi_reader.rs` (remove local `retry()` + its tests, call `crate::retry` instead)
- Modify: `crates/ddc-backend/src/macos_ioavservice.rs:7-24` (post-Task-3 state; wrap the write in `crate::retry`)

**Interfaces:**
- Produces: `pub(crate) fn retry<T>(attempts: u32, delay: std::time::Duration, f: impl FnMut() -> Result<T>) -> Result<T>` moved to `ddc-backend`'s crate root.
- Consumes: nothing new.

- [ ] **Step 1: Move `retry` and its tests into `lib.rs` (test-first is not applicable here — this is a pure relocation, verified by the pre-existing tests continuing to pass)**

In `crates/ddc-backend/src/ddchi_reader.rs`, delete the `retry` function (currently lines 38-59) and its three tests (`retry_returns_ok_immediately_on_first_success`, `retry_succeeds_after_transient_failures`, `retry_gives_up_after_exhausting_attempts_and_returns_the_last_error`) from the bottom `#[cfg(test)] mod tests` block. Replace the two call sites (`input_codes` and `current_input`) that call bare `retry(...)` with `crate::retry(...)`.

In `crates/ddc-backend/src/lib.rs`, add the relocated function right after the `ddc_io_lock` block added in Task 3:

```rust
/// Retries a fallible operation up to `attempts` times with a short delay
/// between tries. DDC/CI over a switch/dongle chain has been observed (real
/// manual-test session) to intermittently fail with transient errors — a
/// checksum mismatch and an invalid message-length field, on two separate
/// reads — that a bare retry resolves. Shared by the read path
/// (`ddchi_reader`) and, as of IMPROVEMENTS.md #5, the macOS write path
/// (`macos_ioavservice`) as well.
pub(crate) fn retry<T>(attempts: u32, delay: std::time::Duration, mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let mut last_err = None;
    for attempt in 0..attempts {
        match f() {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_err = Some(err);
                if attempt + 1 < attempts {
                    std::thread::sleep(delay);
                }
            }
        }
    }
    Err(last_err.unwrap())
}
```

And move the three relocated tests into `lib.rs`'s existing `#[cfg(test)] mod tests` block (added in Task 3), unchanged apart from dropping the now-redundant `use super::*;` duplication (the block already has it):

```rust
    #[test]
    fn retry_returns_ok_immediately_on_first_success() {
        let calls = std::cell::RefCell::new(0);
        let result = retry(3, Duration::from_millis(1), || {
            *calls.borrow_mut() += 1;
            Ok::<_, anyhow::Error>(42)
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn retry_succeeds_after_transient_failures() {
        let calls = std::cell::RefCell::new(0);
        let result = retry(3, Duration::from_millis(1), || {
            *calls.borrow_mut() += 1;
            if *calls.borrow() < 3 {
                Err(anyhow::anyhow!("transient"))
            } else {
                Ok(42)
            }
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*calls.borrow(), 3);
    }

    #[test]
    fn retry_gives_up_after_exhausting_attempts_and_returns_the_last_error() {
        let calls = std::cell::RefCell::new(0);
        let result = retry(3, Duration::from_millis(1), || {
            *calls.borrow_mut() += 1;
            Err::<i32, _>(anyhow::anyhow!("attempt {}", calls.borrow()))
        });
        assert_eq!(*calls.borrow(), 3);
        assert_eq!(result.unwrap_err().to_string(), "attempt 3");
    }
```

- [ ] **Step 2: Run the tests to verify the relocation didn't break anything**

Run: `cargo test -p ddc-backend`
Expected: PASS — all `retry_*` tests now run from `lib.rs`; `ddchi_reader.rs` no longer defines or tests `retry` itself, and its `input_codes`/`current_input` tests (exercised indirectly through the trait, if any exist — check the file; if not, this step just confirms compilation) still pass.

- [ ] **Step 3: Write the failing test proving the macOS write path retries**

This step cannot mock `ddc_hi::Display` (a concrete external-crate type, not a trait `MacosIoavserviceBackend` takes generically) — consistent with the pre-existing test coverage in this file, which is none beyond compilation, since the read path's tests only cover the generic `retry` helper in isolation, never real hardware I/O. Skip a hardware-dependent test here; Step 2 above (the relocated `retry` tests) is the coverage this task relies on, same as the read path already did.

- [ ] **Step 4: Wrap the macOS write in `crate::retry`**

Replace `crates/ddc-backend/src/macos_ioavservice.rs:7-24` (the `impl DdcBackend for MacosIoavserviceBackend` block, as left by Task 3) with:

```rust
impl DdcBackend for MacosIoavserviceBackend {
    fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()> {
        let _guard = crate::ddc_io_lock();
        if source_addr.is_some() {
            log::warn!(
                "source_addr override ({:?}) requested but unsupported on macOS (ddc-hi hardcodes 0x51); ignoring",
                source_addr
            );
        }
        crate::retry(3, std::time::Duration::from_millis(50), || {
            let mut displays = Display::enumerate();
            let display = displays
                .get_mut(monitor_index as usize)
                .ok_or_else(|| anyhow!("no display at index {}", monitor_index))?;
            display
                .handle
                .set_vcp_feature(code, value)
                .map_err(|err| anyhow!("failed to set VCP {:#04x}={:#06x}: {:?}", code, value, err))
        })
    }
}
```

- [ ] **Step 5: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ddc-backend/src/lib.rs crates/ddc-backend/src/ddchi_reader.rs crates/ddc-backend/src/macos_ioavservice.rs
git commit -m "fix: retry transient failures on the macOS DDC/CI write path

IMPROVEMENTS.md #5: the read path already retried transient DDC/CI errors
(checksum mismatch, invalid message length); the write path had no retry at
all. Relocates the shared retry() helper to ddc-backend's root so both
paths use the same implementation. Does not implement full bus-recovery
detection — DECISIONS.md Spike #2 remains open for that."
```

---

### Task 6: Add wizard "Advanced" fields for `source_addr`/`vcp_code` overrides

**Files:**
- Modify: `crates/gui/frontend/src/wizard/InputMappingStep.tsx`
- Modify: `crates/gui/frontend/src/wizard/InputMappingStep.test.tsx`
- Modify: `crates/gui/frontend/src/wizard/Wizard.tsx`
- Modify: `crates/gui/frontend/src/wizard/Wizard.test.tsx`

**Interfaces:**
- Produces: `export function parseHexByte(raw: string): number | null | undefined` in `InputMappingStep.tsx` — `null` for blank input, `undefined` for invalid/out-of-range input, otherwise the parsed byte.
- Produces: `InputMappingStep`'s `onComplete` now passes `{ onConnect, onDisconnect, sourceAddr, vcpCode }` (previously `{ onConnect, onDisconnect }`).
- Consumes: none new.

- [ ] **Step 1: Write the failing tests for `parseHexByte` and the new fields**

Replace the top of `crates/gui/frontend/src/wizard/InputMappingStep.test.tsx` (add the new import) and append new `describe` blocks:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { InputMappingStep, parseHexByte } from "./InputMappingStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("parseHexByte", () => {
  it("returns null for blank input", () => {
    expect(parseHexByte("")).toBeNull();
    expect(parseHexByte("   ")).toBeNull();
  });

  it("parses values with or without the 0x prefix", () => {
    expect(parseHexByte("0x50")).toBe(0x50);
    expect(parseHexByte("50")).toBe(0x50);
    expect(parseHexByte("0xFF")).toBe(0xff);
  });

  it("returns undefined for out-of-range or non-hex input", () => {
    expect(parseHexByte("zz")).toBeUndefined();
    expect(parseHexByte("0x256")).toBeUndefined();
  });
});

describe("InputMappingStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue([]); // default: empty inputs
  });

  it("shows an inline error when reading inputs fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("failed to read capabilities");
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("failed to read capabilities");
  });

  it("shows friendly labels instead of raw hex", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} />);
    expect(await screen.findAllByText("DisplayPort 1")).toHaveLength(2);
    expect(screen.getAllByText("HDMI 1")).toHaveLength(2);
  });

  it("pre-fills the previous selections when navigating back to this step", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(
      <InputMappingStep
        displayIndex={0}
        initialOnConnect={0x11}
        initialOnDisconnect={0x0f}
        onComplete={() => {}}
      />,
    );
    const selects = await screen.findAllByRole("combobox");
    expect((selects[0] as HTMLSelectElement).value).toBe("17"); // 0x11 == 17
    expect((selects[1] as HTMLSelectElement).value).toBe("15"); // 0x0f == 15
  });

  it("calls onBack when the back button is clicked", () => {
    const onBack = vi.fn();
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });

  it("submits parsed source-address and VCP-code overrides on finish", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    const onComplete = vi.fn();
    render(<InputMappingStep displayIndex={0} onComplete={onComplete} />);

    const connectSelect = (await screen.findAllByRole("combobox"))[0];
    fireEvent.change(connectSelect, { target: { value: "17" } });
    fireEvent.change(screen.getByPlaceholderText("0x50"), { target: { value: "0x50" } });
    fireEvent.change(screen.getByPlaceholderText("0x60"), { target: { value: "0x60" } });
    fireEvent.click(screen.getByText("Finish"));

    expect(onComplete).toHaveBeenCalledWith({
      onConnect: 0x11,
      onDisconnect: null,
      sourceAddr: 0x50,
      vcpCode: 0x60,
    });
  });

  it("disables Finish and shows an inline error for an invalid override", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(<InputMappingStep displayIndex={0} initialOnConnect={0x11} onComplete={() => {}} />);

    fireEvent.change(await screen.findByPlaceholderText("0x50"), { target: { value: "zz" } });

    expect(screen.getByRole("alert")).toHaveTextContent("valid hex byte");
    expect(screen.getByText("Finish")).toBeDisabled();
  });

  it("pre-fills the advanced overrides as hex text when navigating back", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(
      <InputMappingStep
        displayIndex={0}
        initialOnConnect={0x11}
        initialSourceAddr={0x50}
        initialVcpCode={0x60}
        onComplete={() => {}}
      />,
    );

    expect(await screen.findByPlaceholderText("0x50")).toHaveValue("0x50");
    expect(screen.getByPlaceholderText("0x60")).toHaveValue("0x60");
  });
});
```

- [ ] **Step 2: Run the tests to verify the new ones fail**

Run: `npm --prefix crates/gui/frontend test -- InputMappingStep`
Expected: FAIL — `parseHexByte` is not exported yet; the three new `it` blocks fail (no matching placeholder text, `onComplete` called with a different shape).

- [ ] **Step 3: Implement `parseHexByte` and the advanced fields**

Replace the full contents of `crates/gui/frontend/src/wizard/InputMappingStep.tsx` with:

```tsx
import { useEffect, useState } from "react";
import { ChevronLeft } from "lucide-react";
import { listInputs } from "../api";
import { vcpInputLabel } from "../vcpLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  displayIndex: number;
  initialOnConnect?: number | null;
  initialOnDisconnect?: number | null;
  initialSourceAddr?: number | null;
  initialVcpCode?: number | null;
  onBack?: () => void;
  onComplete: (mapping: {
    onConnect: number;
    onDisconnect: number | null;
    sourceAddr: number | null;
    vcpCode: number | null;
  }) => void;
}

/** Parses an optional hex byte the user typed (e.g. "0x50", "50", ""),
 * returning `null` for a blank/whitespace-only input and `undefined` if the
 * text doesn't parse to a valid 0-255 byte. Exported so its edge cases are
 * testable without mounting the component. */
export function parseHexByte(raw: string): number | null | undefined {
  const trimmed = raw.trim();
  if (trimmed === "") return null;
  const withoutPrefix = trimmed.toLowerCase().startsWith("0x") ? trimmed.slice(2) : trimmed;
  if (!/^[0-9a-f]+$/i.test(withoutPrefix)) return undefined;
  const value = parseInt(withoutPrefix, 16);
  return value >= 0 && value <= 0xff ? value : undefined;
}

export function InputMappingStep({
  displayIndex,
  initialOnConnect = null,
  initialOnDisconnect = null,
  initialSourceAddr = null,
  initialVcpCode = null,
  onBack,
  onComplete,
}: Props) {
  const [inputs, setInputs] = useState<number[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [onConnect, setOnConnect] = useState<number | null>(initialOnConnect);
  const [onDisconnect, setOnDisconnect] = useState<number | null>(initialOnDisconnect);
  const [sourceAddrText, setSourceAddrText] = useState(
    initialSourceAddr != null ? `0x${initialSourceAddr.toString(16)}` : "",
  );
  const [vcpCodeText, setVcpCodeText] = useState(
    initialVcpCode != null ? `0x${initialVcpCode.toString(16)}` : "",
  );

  useEffect(() => {
    listInputs(displayIndex)
      .then(setInputs)
      .catch((err) => setError(String(err)));
  }, [displayIndex]);

  const sourceAddr = parseHexByte(sourceAddrText);
  const vcpCode = parseHexByte(vcpCodeText);
  const advancedFieldsInvalid = sourceAddr === undefined || vcpCode === undefined;

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
        <h2 className="text-base font-semibold text-neutral-900 dark:text-neutral-100">Map inputs</h2>
      </div>

      {inputs === null && !error && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">Reading supported inputs…</p>
      )}

      {inputs !== null && (
        <>
          <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
            Switch to this input when the KVM switch connects to this host:
            <select
              value={onConnect ?? ""}
              onChange={(e) => setOnConnect(e.target.value ? Number(e.target.value) : null)}
              className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
            >
              <option value="">Select…</option>
              {inputs.map((v) => (
                <option key={v} value={v}>
                  {vcpInputLabel(v)}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
            Switch to this input on disconnect (optional):
            <select
              value={onDisconnect ?? ""}
              onChange={(e) => setOnDisconnect(e.target.value ? Number(e.target.value) : null)}
              className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
            >
              <option value="">None</option>
              {inputs.map((v) => (
                <option key={v} value={v}>
                  {vcpInputLabel(v)}
                </option>
              ))}
            </select>
          </label>

          <div className="flex flex-col gap-2 rounded-md border border-neutral-200 p-2 dark:border-neutral-700">
            <span className="text-xs font-medium uppercase tracking-wide text-neutral-500">
              Advanced (optional — only if your monitor needs a non-standard address)
            </span>
            <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
              I2C source-address override (hex; Windows only, ignored on macOS). Blank uses this
              app's default, 0x50.
              <input
                type="text"
                placeholder="0x50"
                value={sourceAddrText}
                onChange={(e) => setSourceAddrText(e.target.value)}
                className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
              />
            </label>
            <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
              VCP feature-code override (hex). Blank uses the DDC/CI standard, 0x60.
              <input
                type="text"
                placeholder="0x60"
                value={vcpCodeText}
                onChange={(e) => setVcpCodeText(e.target.value)}
                className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
              />
            </label>
            {advancedFieldsInvalid && (
              <p role="alert" className="text-sm text-red-600 dark:text-red-400">
                Enter a valid hex byte (00–FF), or leave the field blank for the default.
              </p>
            )}
          </div>

          <button
            disabled={onConnect === null || advancedFieldsInvalid}
            onClick={() =>
              onComplete({
                onConnect: onConnect!,
                onDisconnect,
                sourceAddr: sourceAddr ?? null,
                vcpCode: vcpCode ?? null,
              })
            }
            className="self-start rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Finish
          </button>
        </>
      )}

      <InlineError message={error} />
    </div>
  );
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix crates/gui/frontend test -- InputMappingStep`
Expected: PASS — all 8 tests in the file.

- [ ] **Step 5: Write the failing tests for `Wizard.tsx`'s passthrough**

In `crates/gui/frontend/src/wizard/Wizard.test.tsx`, replace the `vi.mock("./InputMappingStep", ...)` block (lines 31-40) with:

```tsx
vi.mock("./InputMappingStep", () => ({
  InputMappingStep: ({ initialOnConnect, initialOnDisconnect, onComplete, onBack }: any) => (
    <div>
      <p data-testid="input-initial-connect">{JSON.stringify(initialOnConnect)}</p>
      <p data-testid="input-initial-disconnect">{JSON.stringify(initialOnDisconnect)}</p>
      <button onClick={() => onComplete({ onConnect: 0x11, onDisconnect: null })}>finish</button>
      <button
        onClick={() =>
          onComplete({ onConnect: 0x11, onDisconnect: null, sourceAddr: 0x50, vcpCode: 0x60 })
        }
      >
        finish-with-overrides
      </button>
      <button onClick={onBack}>back-inputs</button>
    </div>
  ),
}));
```

Then add two new `it` blocks at the end of the `describe("Wizard", ...)` block, right before its closing `});`:

```tsx
  it("defaults the source-address and VCP-code overrides to null when the input step doesn't provide them", async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor-a"));
    fireEvent.click(screen.getByText("finish"));

    await waitFor(() =>
      expect(onComplete).toHaveBeenCalledWith(
        expect.objectContaining({
          on_usb_connect_source_addr: null,
          on_usb_connect_vcp_code: null,
        }),
      ),
    );
  });

  it("passes through the source-address and VCP-code overrides when the input step provides them", async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor-a"));
    fireEvent.click(screen.getByText("finish-with-overrides"));

    await waitFor(() =>
      expect(onComplete).toHaveBeenCalledWith(
        expect.objectContaining({
          on_usb_connect_source_addr: 0x50,
          on_usb_connect_vcp_code: 0x60,
        }),
      ),
    );
  });
```

- [ ] **Step 6: Run the tests to verify the new ones fail**

Run: `npm --prefix crates/gui/frontend test -- Wizard.test.tsx`
Expected: FAIL — `finish` still hardcodes `on_usb_connect_source_addr: null`/`on_usb_connect_vcp_code: null` regardless of what `InputMappingStep` passes, so the "passes through" test fails (the "defaults" test happens to already pass, since `finish` currently ignores the overrides entirely — but leave both, the first guards against a regression once `finish` is rewritten).

- [ ] **Step 7: Implement the passthrough in `Wizard.tsx`**

Replace `crates/gui/frontend/src/wizard/Wizard.tsx` lines 10-52 (the `WizardAnswers` interface through the end of `finish`) with:

```tsx
interface WizardAnswers {
  switchDevice: string | null;
  mxkeysDevice: string | null;
  monitor: MonitorInfo | null;
  onConnect: number | null;
  onDisconnect: number | null;
  sourceAddr: number | null;
  vcpCode: number | null;
}

const STEP_COUNT = 4;

const emptyAnswers: WizardAnswers = {
  switchDevice: null,
  mxkeysDevice: null,
  monitor: null,
  onConnect: null,
  onDisconnect: null,
  sourceAddr: null,
  vcpCode: null,
};

export function Wizard({ onComplete }: { onComplete: (config: Configuration) => void }) {
  const [stepIndex, setStepIndex] = useState(0);
  const [answers, setAnswers] = useState<WizardAnswers>(emptyAnswers);
  const [saveError, setSaveError] = useState<string | null>(null);

  const goBack = () => setStepIndex((i) => Math.max(0, i - 1));

  const finish = async (
    onConnect: number,
    onDisconnect: number | null,
    sourceAddr: number | null,
    vcpCode: number | null,
    monitor: MonitorInfo,
  ) => {
    const config: Configuration = {
      usb_device: answers.switchDevice!,
      mxkeys_usb_device: answers.mxkeysDevice || null,
      on_usb_connect: `0x${onConnect.toString(16)}`,
      on_usb_disconnect: onDisconnect !== null ? `0x${onDisconnect.toString(16)}` : null,
      on_usb_connect_source_addr: sourceAddr,
      on_usb_connect_vcp_code: vcpCode,
      display_index: monitor.display_index,
    };
    try {
      setSaveError(null);
      await saveConfig(config);
      onComplete(config);
    } catch (err) {
      setSaveError(String(err));
    }
  };
```

Then replace the `MonitorStep` and `InputMappingStep` render blocks (currently lines 83-108) with:

```tsx
        {stepIndex === 2 && (
          <MonitorStep
            initialSelection={answers.monitor}
            onSelected={(monitor) => {
              setAnswers((a) =>
                a.monitor && a.monitor.display_index !== monitor.display_index
                  ? { ...a, monitor, onConnect: null, onDisconnect: null, sourceAddr: null, vcpCode: null }
                  : { ...a, monitor },
              );
              setStepIndex(3);
            }}
            onBack={goBack}
          />
        )}
        {stepIndex === 3 && answers.monitor && (
          <InputMappingStep
            displayIndex={answers.monitor.display_index}
            initialOnConnect={answers.onConnect}
            initialOnDisconnect={answers.onDisconnect}
            initialSourceAddr={answers.sourceAddr}
            initialVcpCode={answers.vcpCode}
            onBack={goBack}
            onComplete={({ onConnect, onDisconnect, sourceAddr = null, vcpCode = null }) => {
              setAnswers((a) => ({ ...a, onConnect, onDisconnect, sourceAddr, vcpCode }));
              finish(onConnect, onDisconnect, sourceAddr, vcpCode, answers.monitor!);
            }}
          />
        )}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `npm --prefix crates/gui/frontend test -- Wizard.test.tsx`
Expected: PASS — all 7 tests (5 pre-existing + 2 new).

- [ ] **Step 9: Run the full frontend test suite**

Run: `npm --prefix crates/gui/frontend test`
Expected: PASS — no regressions in `MainScreen.test.tsx`, `vcpLabels.test.ts`, `DeviceStep.test.tsx`, `MonitorStep.test.tsx`.

- [ ] **Step 10: Commit**

```bash
git add crates/gui/frontend/src/wizard/InputMappingStep.tsx crates/gui/frontend/src/wizard/InputMappingStep.test.tsx crates/gui/frontend/src/wizard/Wizard.tsx crates/gui/frontend/src/wizard/Wizard.test.tsx
git commit -m "feat: expose source-address and VCP-code overrides in the setup wizard

IMPROVEMENTS.md #7: the schema, serde plumbing, and orchestrator wiring for
on_usb_connect_source_addr/on_usb_connect_vcp_code already existed and were
tested, but the wizard hardcoded both to null with no UI path to set them.
Any monitor that isn't exactly the validated 34GL750 recipe (DECISIONS.md
#4) previously had no supported way to reach these overrides."
```

---

### Task 7: Point the future `linux_ddcutil.rs` at the subprocess approach, not FFI

**Files:**
- Modify: `crates/ddc-backend/src/lib.rs:41-42`

**Interfaces:** none — comment-only change, no compiled code affected.

- [ ] **Step 1: Update the TODO comment**

Replace `crates/ddc-backend/src/lib.rs:41-42`:

```rust
// TODO(v2): linux_ddcutil.rs — wrapper over ddcutil/i2c-dev, which already
// supports --i2c-source-addr natively (see DECISIONS.md #9). Out of scope.
```

with:

```rust
// TODO(v2): linux_ddcutil.rs — wrapper over the `ddcutil` CLI, invoked as a
// subprocess (same pattern as windows_nvapi.rs's Command::new), NOT linked
// via FFI/libddcutil bindings: ddcutil is GPL-2.0, this project is MIT, and
// --i2c-source-addr's presence in the public C API is unconfirmed anyway.
// See DECISIONS.md #9 and IMPROVEMENTS.md #9. Out of scope for this milestone.
```

- [ ] **Step 2: Verify the workspace still builds**

Run: `cargo build --workspace`
Expected: PASS — comment-only change.

- [ ] **Step 3: Commit**

```bash
git add crates/ddc-backend/src/lib.rs
git commit -m "docs: point the future linux_ddcutil.rs at the ddcutil CLI, not FFI

IMPROVEMENTS.md #9: linking libddcutil (GPL-2.0) into this MIT-licensed
project would create a combined work subject to GPL-2.0; invoking the
ddcutil binary as a subprocess (like the existing NVAPI helper) avoids
that, and is also where --i2c-source-addr is confirmed to work."
```

---

## Self-Review Notes

- **Spec coverage:** all 9 `IMPROVEMENTS.md` items are addressed — #1→Task 1, #2→Task 2, #3→Task 3, #4→Task 4, #5→Task 5, #6→explicitly out of scope (Global Constraints), #7→Task 6, #8→already done in `DECISIONS.md` (prior session, not re-done here), #9→Task 7.
- **Placeholder scan:** no `TODO`/`fill in`/"add appropriate" phrasing in any step; every step shows complete code.
- **Type consistency:** `DdcBackend::set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> Result<()>` is unchanged end-to-end across Tasks 2-5. `InputMappingStep`'s `onComplete` shape (`{ onConnect, onDisconnect, sourceAddr, vcpCode }`) is introduced in Task 6 and consumed consistently in the same task's `Wizard.tsx` edit — no other task touches it.
