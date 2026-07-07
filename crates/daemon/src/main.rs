use anyhow::{Context, Result};
use clap::Parser;
use ddc_backend::windows_nvapi::NvapiBackend;
use kvm_core::config::Configuration;
use kvm_core::orchestrator;
use power_fallback::windows_monitorpower::WindowsMonitorPower;
use trigger::usb_hotplug::UsbHotplugTrigger;
use trigger::TriggerSource;
use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Print debug information
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    /// Path to the configuration file
    #[arg(short = 'c', long = "config")]
    config_file_path: Option<std::path::PathBuf>,
}

/// Re-attach the console if the parent process has one, so log output shows
/// up when run from the command line.
fn attach_console() {
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn init_logging(debug: bool) -> Result<()> {
    use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
    let level = if debug { LevelFilter::Debug } else { LevelFilter::Info };
    CombinedLogger::init(vec![TermLogger::new(level, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)])
        .context("failed to initialize logging")
}

/// Resolves `tools/writeValueToDisplay.exe` relative to the daemon binary's
/// own directory (see `docs/superpowers/specs/2026-07-06-kvm-switch-fork-mvp-design.md`).
fn default_exe_path() -> Result<std::path::PathBuf> {
    let mut path = std::env::current_exe().context("failed to locate daemon executable")?;
    path.pop();
    path.push("tools");
    path.push("writeValueToDisplay.exe");
    Ok(path)
}

fn main() -> Result<()> {
    attach_console();
    let args = Args::parse();
    init_logging(args.debug)?;

    let config_path = args.config_file_path.unwrap_or_else(|| std::path::PathBuf::from("display-switch.ini"));
    let config = Configuration::load(&config_path)
        .with_context(|| format!("failed to load configuration from {:?}", config_path))?;

    let ddc_backend = NvapiBackend::new(default_exe_path()?);
    let power_fallback = WindowsMonitorPower;
    let trigger_source = UsbHotplugTrigger::new(&config.usb_device);

    log::info!("kvm-switch daemon started, watching USB device {}", config.usb_device);
    for event in trigger_source.watch() {
        orchestrator::handle_event(event, &config, &ddc_backend, &power_fallback);
    }
    Ok(())
}
