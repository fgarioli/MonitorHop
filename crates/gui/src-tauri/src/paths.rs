/// Resolves and creates the per-OS application-support directory
/// (`%APPDATA%\MonitorHop` on Windows, `$HOME/Library/Application
/// Support/MonitorHop` on macOS), returning `None` if the relevant
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

    let dir = base?.join("MonitorHop");
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
        .map(|dir| dir.join("config.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("config.json"))
}

/// Resolves the switching tool relative to the running executable's own
/// directory rather than the CWD — this matches how a real installed/
/// autostarted build is laid out (the tool ships alongside the binary,
/// modulo the bundling caveat below), and autostart launches with an
/// unpredictable CWD (see `config_path` above). Falls back to a path anchored
/// on `CARGO_MANIFEST_DIR` (baked in at compile time as this crate's own
/// source directory) if the exe-relative one doesn't exist on disk, which
/// keeps `cargo tauri dev`/local development working regardless of the
/// process's *runtime* CWD. A plain CWD-relative fallback was tried first and
/// found broken: `cargo tauri dev` runs its DevCommand with CWD set to this
/// crate's own directory (`crates/gui/src-tauri`), not the repo root, so
/// `"tools/writeValueToDisplay.exe"` resolved to a directory that doesn't
/// exist there (confirmed via manual testing — `os error 3`, path not found).
///
/// NOTE: whether a real Tauri bundle actually places `tools/
/// writeValueToDisplay.exe` next to the installed binary is a packaging
/// concern (`tauri.conf.json`'s `bundle.resources`) that this function does
/// not address — it only fixes path *resolution* logic. See the final-review
/// fix report for details; resource bundling remains a follow-up.
#[cfg(windows)]
pub(crate) fn default_exe_path() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("tools/writeValueToDisplay.exe");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/writeValueToDisplay.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for the `os error 3` (path not found) bug found during
    /// manual testing: the exe-relative candidate never exists in a `cargo
    /// tauri dev` session (nothing copies the tool into `target/debug/tools`),
    /// so this asserts the `CARGO_MANIFEST_DIR`-anchored fallback actually
    /// resolves to the real file in this checkout, regardless of the test
    /// runner's CWD.
    #[cfg(windows)]
    #[test]
    fn default_exe_path_fallback_resolves_to_real_file() {
        assert!(default_exe_path().exists(), "{:?} does not exist", default_exe_path());
    }

    #[cfg(windows)]
    #[test]
    fn app_support_dir_uses_appdata_on_windows() {
        let dir = app_support_dir().expect("APPDATA should be set in the test environment");
        assert!(dir.ends_with("MonitorHop"));
        assert!(dir.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_support_dir_uses_library_application_support_on_macos() {
        let dir = app_support_dir().expect("HOME should be set in the test environment");
        assert!(dir.ends_with("MonitorHop"));
        assert!(dir.to_string_lossy().contains("Library/Application Support"));
    }
}
