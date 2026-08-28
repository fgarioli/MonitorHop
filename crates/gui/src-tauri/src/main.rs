use anyhow::{Context, Result};
use kvm_core::config::Configuration;
use kvm_core::orchestrator::DaemonEvent;
use std::sync::Mutex;
use tauri::Manager;

mod app_state;
mod commands;
mod device_database;
mod paths;
mod platform;
mod tray;
mod updater;

use app_state::AppState;
use paths::config_path;
use platform::{spawn_consumer, spawn_mxkeys_trigger, spawn_switch_trigger};
use tray::build_quick_switch_items;

fn init_logging() -> Result<()> {
    use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .context("failed to initialize logging")
}

fn main() {
    init_logging().expect("failed to initialize logging");

    let config = Configuration::load(&config_path()).ok();

    let (tx, rx) = std::sync::mpsc::channel::<DaemonEvent>();

    // When a config already exists at startup, start the pipeline right
    // away, same as before. When it doesn't (a genuine first run), `rx`
    // must NOT just be dropped here — that would silently discard the
    // receive half of the channel, so any later `switch_input`/tray/trigger
    // send would go into a channel nobody reads. Instead, park it in
    // `AppState.pending_rx` so `commands::save_config` can start the
    // pipeline once the setup wizard writes a config for the first time.
    //
    // `spawn_consumer` now needs a `tauri::AppHandle` (to emit
    // `current-input-changed`), which doesn't exist yet at this point in
    // `main()` — so unlike `spawn_switch_trigger` above, its call is
    // deferred into `.setup()` below, mirroring how `spawn_mxkeys_trigger`
    // already has to wait for the same reason.
    let mut pending_rx = None;
    let mut startup_consumer: Option<(std::sync::mpsc::Receiver<DaemonEvent>, Configuration)> = None;

    if let Some(config) = config.clone() {
        spawn_switch_trigger(config.usb_device.clone(), tx.clone());
        startup_consumer = Some((rx, config));
    } else {
        log::warn!(
            "No configuration found at {:?} yet; switching is disabled until the setup wizard runs.",
            config_path()
        );
        pending_rx = Some(rx);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            events: Mutex::new(tx),
            mxkeys_status_item: Mutex::new(None),
            pending_rx: Mutex::new(pending_rx),
            tray_icon: Mutex::new(None),
        })
        .setup(move |app| {
            updater::spawn_update_check(app.handle().clone());

            if let Some((rx, config)) = startup_consumer {
                let handle = app.handle().clone();
                spawn_consumer(rx, config, handle);
            }

            if let Some(config) = config.clone() {
                if let Some(mxkeys_device) = config.mxkeys_usb_device.clone() {
                    let handle = app.handle().clone();
                    spawn_mxkeys_trigger(mxkeys_device, handle);
                }
            }

            {
                use kvm_core::config::InputSource;
                use tauri::menu::{IsMenuItem, Menu};
                use tauri::tray::TrayIconBuilder;

                let handle = app.handle().clone();
                let items = build_quick_switch_items(&handle, config.as_ref())?;
                *app.state::<AppState>().mxkeys_status_item.lock().unwrap() = Some(items[1].clone());

                let item_refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
                    items.iter().map(|item| item as &dyn IsMenuItem<tauri::Wry>).collect();
                let menu = Menu::with_items(app, &item_refs)?;

                let tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, event| {
                        let id = event.id.as_ref();
                        match id {
                            "open" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.unminimize();
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "quit" => app.exit(0),
                            id if id.starts_with("switch:") => {
                                if let Ok(value) = u16::from_str_radix(id.trim_start_matches("switch:0x"), 16) {
                                    let state = app.state::<AppState>();
                                    let events = state.events.lock().unwrap();
                                    let _ = events.send(DaemonEvent::ManualSwitch(InputSource::Raw(value)));
                                }
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;

                *app.state::<AppState>().tray_icon.lock().unwrap() = Some(tray);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_usb_devices,
            commands::list_monitors,
            commands::list_inputs,
            commands::save_config,
            commands::load_config,
            commands::switch_input,
            commands::current_input,
            commands::load_device_database,
            updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
