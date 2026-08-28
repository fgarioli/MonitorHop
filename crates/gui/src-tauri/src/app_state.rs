use kvm_core::orchestrator::DaemonEvent;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

pub struct AppState {
    pub events: Mutex<Sender<DaemonEvent>>,
    /// Filled in by Task 9's tray setup once the menu is built; `None` until
    /// then. The MX Keys forwarder thread below updates its text whenever
    /// presence changes.
    pub mxkeys_status_item: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
    /// Holds the `DaemonEvent` receiver when no configuration existed at
    /// process startup (a genuine first run): `main()` cannot spawn the
    /// trigger/consumer threads without a `Configuration`, so rather than
    /// dropping `rx` it parks it here. `commands::save_config` takes it out
    /// and starts the pipeline the first time the setup wizard writes a
    /// config. Left `None` once the pipeline has been started, whether that
    /// happened at startup or from `save_config` — a second `save_config`
    /// call (a "Reconfigure") finds this empty and knows not to spawn a
    /// second, duplicate set of threads.
    pub pending_rx: Mutex<Option<std::sync::mpsc::Receiver<DaemonEvent>>>,
    /// The tray icon built once in `.setup()`. Stored so `save_config` can
    /// refresh the quick-switch menu items in place (via `TrayIcon::set_menu`)
    /// after a first-run config write starts the pipeline, without building a
    /// second, duplicate tray icon.
    pub tray_icon: Mutex<Option<tauri::tray::TrayIcon<tauri::Wry>>>,
}
