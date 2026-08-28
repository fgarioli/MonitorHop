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

/// Same `%APPDATA%\MonitorHop\` (Windows) / `$HOME/Library/Application
/// Support/MonitorHop` (macOS) directory as `config_path()` in
/// `main.rs` — falls back to a CWD-relative path if that directory can't be
/// resolved, matching that function's existing defensive behavior.
pub(crate) fn device_database_path() -> PathBuf {
    crate::paths::app_support_dir()
        .map(|dir| dir.join("device-database.json"))
        .unwrap_or_else(|| PathBuf::from("device-database.json"))
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
