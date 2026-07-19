# Design: Isolate platform selection & split `main.rs` in `crates/gui/src-tauri`

## Context

`crates/kvm_core`, `crates/ddc-backend`, `crates/power-fallback`, and
`crates/trigger` already implement a ports-and-adapters style architecture:
`kvm_core` depends only on trait objects (`DdcBackend`, `PowerFallback`,
`TriggerSource`) and the adapter crates provide per-OS implementations, one
file per backend, gated with `#[cfg(windows)]` / `#[cfg(target_os =
"macos")]` `pub mod` declarations at the crate root (see
`ddc-backend/src/lib.rs`, `power-fallback/src/lib.rs`,
`trigger/src/lib.rs`). This is already the intended "clean architecture" for
this codebase — dependencies point inward, adapters are swappable, and
`docs/DECISIONS.md`/`docs/IMPROVEMENTS.md` document it as such.

The one place that does not follow this convention is
`crates/gui/src-tauri/src/main.rs` (415 lines). It mixes together: app-support
path resolution (`app_support_dir`, `config_path`, `default_exe_path`),
per-platform thread-spawning functions with inline `#[cfg(windows)]` /
`#[cfg(target_os = "macos")]` bodies (`spawn_switch_trigger`,
`spawn_consumer`, `spawn_mxkeys_trigger`), tray-menu construction
(`build_quick_switch_items`), the `AppState` struct, and the Tauri
`main()` entrypoint itself — all in one file, with platform dispatch
duplicated function-by-function rather than isolated in one place.

This matters concretely for future growth (Linux/`ddcutil` backend,
AMD/ADL backend — see `docs/IMPROVEMENTS.md` #4/#9): adding a new platform
today means editing three separate functions inside `main.rs`, each already
containing two `#[cfg]`-gated bodies, rather than adding one new file and one
line of dispatch.

## Goal

Make adding a future platform backend to the GUI a matter of adding one new
file + one line of `cfg` dispatch, with no edits to the Tauri wiring in
`main.rs` itself — by applying the same per-file-per-platform convention the
other four crates already use.

## Non-goals

- No change to `kvm_core`, `ddc-backend`, `power-fallback`, or `trigger`'s
  public APIs or traits. Their existing ports-and-adapters shape is correct
  and out of scope.
- No behavior change of any kind. This is a structural move, not a rewrite.
- No frontend/TypeScript changes.
- Does not implement a real Linux or AMD/ADL backend. That remains separate,
  future work tracked in `docs/IMPROVEMENTS.md` #4 and #9.
- No new abstraction beyond file/module reorganization — no runtime
  plugin registry, no dynamic backend selection. Platform selection stays a
  compile-time `#[cfg]` choice, matching the existing convention in the
  other crates.

## Design

Split `crates/gui/src-tauri/src/main.rs` into:

- **`app_state.rs`** — the `AppState` struct only (`events`,
  `mxkeys_status_item`, `pending_rx`, `tray_icon` fields), moved verbatim.
- **`paths.rs`** — `app_support_dir`, `config_path`, and
  `default_exe_path` (the latter `#[cfg(windows)]`-gated, as today), plus the
  three existing tests that cover them (`default_exe_path_fallback_resolves_to_real_file`,
  `app_support_dir_uses_appdata_on_windows`,
  `app_support_dir_uses_library_application_support_on_macos`). Pure
  relocation — no logic changes.
- **`platform/mod.rs`, `platform/windows.rs`, `platform/macos.rs`** — moves
  `spawn_switch_trigger`, `spawn_consumer`, and `spawn_mxkeys_trigger` out of
  `main.rs`'s inline `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` pairs
  into one file per OS, each exposing the same three function signatures.
  `platform/mod.rs` re-exports the active OS's implementations:
  ```rust
  #[cfg(windows)]
  mod windows;
  #[cfg(windows)]
  pub use windows::*;

  #[cfg(target_os = "macos")]
  mod macos;
  #[cfg(target_os = "macos")]
  pub use macos::*;
  ```
  This mirrors `ddc-backend/src/lib.rs`'s existing pattern
  (`#[cfg(windows)] pub mod windows_nvapi;` etc.) applied one level up, at
  the GUI crate's own platform-dispatch boundary.
- **`tray.rs`** — `build_quick_switch_items`, moved verbatim (it already
  depends only on `ddc_backend::MonitorReader` and `kvm_core::Configuration`,
  no platform-specific code of its own).
- **`main.rs`** — shrinks to `init_logging`, the `main()` function itself
  (Tauri `Builder` setup, `.setup()`, `.on_window_event()`,
  `.invoke_handler()`), and `mod` declarations for the new files. Calls into
  `app_state::AppState`, `paths::config_path`, `platform::spawn_consumer`
  (etc.), and `tray::build_quick_switch_items`.
- **`commands.rs`** — only its `use crate::{...}` import line changes, to
  point at the new module paths (`crate::paths::config_path`,
  `crate::platform::spawn_consumer`, `crate::tray::build_quick_switch_items`,
  `crate::app_state::AppState`). No logic changes.
- **`device_database.rs`** — its one call to `crate::app_support_dir()`
  becomes `crate::paths::app_support_dir()`. No other changes.

Adding a Linux backend later becomes: create `platform/linux.rs` implementing
the same three function signatures, add
`#[cfg(target_os = "linux")] mod linux; #[cfg(target_os = "linux")] pub use linux::*;`
to `platform/mod.rs`. `main.rs`, `commands.rs`, and `device_database.rs` do
not need to change.

## Data flow / behavior

Unchanged. Every function keeps its exact signature and body; only their
file location, `crate::` path prefixes, and `pub`/`pub(crate)` visibility
(as needed for cross-module calls) change. `AppState`'s field types, the
`DaemonEvent` channel wiring, the tray menu contents, and the Tauri command
surface are all identical before and after.

## Error handling

No change — this refactor touches no error paths. Existing `Result`/`?`
usage moves verbatim.

## Testing

- The three existing unit tests that cover `paths.rs`'s functions move with
  them and must still pass unchanged.
- `cargo build --workspace` must succeed with no new warnings.
- `cargo test --workspace` must still show the same pass count as today (40
  tests) — this is a reorganization, so the number and behavior of tests
  should not change, only (for the three path tests) which file they live in.
- `npm test` in `crates/gui/frontend` (52/52) is unaffected and is re-run
  only as a sanity check that nothing in the Rust move broke the Tauri
  command surface the frontend calls through `api.ts`.
- The macOS-only module (`platform/macos.rs`) cannot be compiled on this
  Windows machine. As in prior sessions, verify it by careful line-by-line
  diff review against the original `main.rs` code it was moved from, rather
  than by compiling it.

## Risks

- **Low overall risk**: this is a pure move-and-rename refactor with no
  logic changes, in a single crate, with no changes to any other crate's
  public API.
- The main risk is a broken import/visibility (`pub` vs `pub(crate)`) after
  the split, which `cargo build --workspace` will catch immediately.
- The macOS branch not being compile-checked locally is a pre-existing
  limitation of this dev machine (also true of the previous session's
  macOS-only changes), mitigated the same way: careful diff review.
