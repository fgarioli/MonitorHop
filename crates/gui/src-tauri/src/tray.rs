use kvm_core::config::Configuration;

/// Builds the tray's menu items: Open, the MX Keys status item, one
/// "Switch to <code>" item per DDC input code the configured display
/// reports (when `config` is `Some` and the display responds), and Quit.
///
/// Shared by `.setup()` (which builds the tray, and these items, once at
/// startup) and `commands::save_config` (which rebuilds just the items and
/// swaps them onto the existing tray icon via `TrayIcon::set_menu` after a
/// first-run config write starts the pipeline) so both stay in sync.
pub(crate) fn build_quick_switch_items(
    app: &tauri::AppHandle,
    config: Option<&Configuration>,
) -> tauri::Result<Vec<tauri::menu::MenuItem<tauri::Wry>>> {
    use ddc_backend::MonitorReader;
    use tauri::menu::MenuItem;

    let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let status_i = MenuItem::with_id(app, "mxkeys-status", "MX Keys: unknown", false, None::<&str>)?;

    // `Menu::with_items` wants `&[&dyn IsMenuItem<Wry>]`; keep the concrete
    // `MenuItem<Wry>`s owned here and build trait-object refs at the call
    // site once the full set is known.
    let mut items: Vec<MenuItem<tauri::Wry>> = vec![open_i, status_i];

    if let Some(config) = config {
        if let Ok(codes) = ddc_backend::ddchi_reader::DdcHiMonitorReader.input_codes(config.display_index()) {
            for code in codes {
                let id = format!("switch:{code:#04x}");
                let label = format!("Switch to {code:#04x}");
                items.push(MenuItem::with_id(app, id, label, true, None::<&str>)?);
            }
        }
    }

    items.push(MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?);
    Ok(items)
}
