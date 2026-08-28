//! Self-update against the `latest.json` manifest published by
//! `.github/workflows/release.yml` (endpoint and public key live in
//! `tauri.conf.json` under `plugins.updater`).
//!
//! The check is deliberately advisory: it runs in the background at startup
//! and only *emits* `update-available`, leaving the decision to install to
//! the user. A tray app that silently restarted itself mid-session would be
//! a poor citizen, and this one holds the DDC/CI write path.

use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// Checks for a newer release in the background, emitting `update-available`
/// with the new version string when there is one.
///
/// Every failure path is a log line rather than a user-visible error: no
/// network, a rate-limited GitHub, or a release with no manifest yet are all
/// perfectly normal, and none of them should interrupt someone who launched
/// this app to switch a monitor input.
pub(crate) fn spawn_update_check(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(updater) => updater,
            Err(err) => {
                log::warn!("updater unavailable: {err}");
                return;
            }
        };
        match updater.check().await {
            Ok(Some(update)) => {
                log::info!("update available: {}", update.version);
                let _ = app.emit("update-available", update.version.clone());
            }
            Ok(None) => log::info!("no update available"),
            Err(err) => log::warn!("update check failed: {err}"),
        }
    });
}

/// Downloads and installs the pending update, then restarts into it.
///
/// Re-runs `check()` rather than caching the `Update` from
/// `spawn_update_check`: the handle is not `'static`-friendly to park in
/// `AppState`, and a second HTTP round-trip is a fine price for a button the
/// user presses at most once per release.
#[tauri::command]
pub(crate) async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|err| err.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "No update available.".to_string())?;

    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|err| err.to_string())?;

    app.restart();
}
