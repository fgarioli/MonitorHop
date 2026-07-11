use crate::config::{Configuration, InputSource};
use crate::monitor_map::{self, SwitchDirection, SwitchTarget};
use ddc_backend::DdcBackend;
use power_fallback::PowerFallback;
use trigger::TriggerEvent;

/// VCP feature code for input select (DDC/CI standard) — see DECISIONS.md #4.
pub const INPUT_SELECT: u8 = 0x60;

/// Events consumed by `run`'s single consumer loop. `Trigger` comes from a
/// background `TriggerSource` watcher; `ManualSwitch` comes from the GUI's
/// "switch now" button. Both funnel through the same handlers below, so only
/// one thing ever calls into the DDC write path.
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    Trigger(TriggerEvent),
    ManualSwitch(InputSource),
}

fn perform_switch(target: &SwitchTarget, ddc_backend: &dyn DdcBackend, power_fallback: &dyn PowerFallback) {
    let attempt = |ddc_backend: &dyn DdcBackend| {
        ddc_backend.set_vcp(target.display_index, target.vcp_code, target.input_source.value(), target.source_addr)
    };
    if let Err(err) = attempt(ddc_backend) {
        log::warn!("Failed to switch display input: {:?}. Retrying after power fallback.", err);
        if let Err(err) = power_fallback.blank_and_restore() {
            log::error!("Power fallback failed: {:?}", err);
        }
        if let Err(err) = attempt(ddc_backend) {
            log::error!("Retry failed, giving up: {:?}", err);
        }
    } else {
        log::info!("Display switched to {:?}", target.input_source);
    }
}

pub fn handle_event(
    event: TriggerEvent,
    config: &Configuration,
    ddc_backend: &dyn DdcBackend,
    power_fallback: &dyn PowerFallback,
) {
    let direction = match event {
        TriggerEvent::HostGainedFocus => SwitchDirection::Connect,
        TriggerEvent::HostLostFocus => SwitchDirection::Disconnect,
    };
    let Some(target) = monitor_map::resolve(config, direction) else {
        log::info!("No input source configured for {:?}, skipping", direction);
        return;
    };
    perform_switch(&target, ddc_backend, power_fallback);
}

/// Switches directly to `input`, using the monitor's configured
/// `display_index`/`source_addr`/`vcp_code` but ignoring the
/// connect/disconnect mapping — used by the GUI's manual "switch now" button.
pub fn handle_manual_switch(
    input: InputSource,
    config: &Configuration,
    ddc_backend: &dyn DdcBackend,
    power_fallback: &dyn PowerFallback,
) {
    let target = SwitchTarget {
        display_index: config.display_index(),
        input_source: input,
        source_addr: config.on_usb_connect_source_addr,
        vcp_code: config.vcp_code(),
    };
    perform_switch(&target, ddc_backend, power_fallback);
}

/// The single consumer of `DaemonEvent`s. Runs until `events`'s sender is
/// dropped. Intended to run on its own background thread in the GUI binary
/// (Task 7) — everything that can trigger a switch sends into the channel
/// this reads from, so exactly one thread ever calls `DdcBackend::set_vcp`.
pub fn run(
    events: std::sync::mpsc::Receiver<DaemonEvent>,
    config: &Configuration,
    ddc_backend: &dyn DdcBackend,
    power_fallback: &dyn PowerFallback,
) {
    for event in events {
        match event {
            DaemonEvent::Trigger(trigger_event) => handle_event(trigger_event, config, ddc_backend, power_fallback),
            DaemonEvent::ManualSwitch(input) => handle_manual_switch(input, config, ddc_backend, power_fallback),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Configuration;
    use ddc_backend::DdcBackend;
    use power_fallback::PowerFallback;
    use std::cell::RefCell;
    use trigger::TriggerEvent;

    struct FakeDdc {
        calls: RefCell<Vec<(u32, u8, u16, Option<u8>)>>,
        fail_first_n: RefCell<u32>,
    }

    impl DdcBackend for FakeDdc {
        fn set_vcp(&self, monitor_index: u32, code: u8, value: u16, source_addr: Option<u8>) -> anyhow::Result<()> {
            self.calls.borrow_mut().push((monitor_index, code, value, source_addr));
            let mut remaining = self.fail_first_n.borrow_mut();
            if *remaining > 0 {
                *remaining -= 1;
                return Err(anyhow::anyhow!("simulated failure"));
            }
            Ok(())
        }
    }

    struct FakePower {
        called: RefCell<u32>,
    }

    impl PowerFallback for FakePower {
        fn blank_and_restore(&self) -> anyhow::Result<()> {
            *self.called.borrow_mut() += 1;
            Ok(())
        }
    }

    fn load(config_str: &str) -> Configuration {
        serde_json::from_str(config_str).unwrap()
    }

    #[test]
    fn successful_switch_calls_ddc_backend_once_with_resolved_target() {
        let config = load(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1", "on_usb_connect_source_addr": 80}"#,
        );
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostGainedFocus, &config, &ddc, &power);

        assert_eq!(*ddc.calls.borrow(), vec![(0, 0x60, 0x11, Some(0x50))]);
        assert_eq!(*power.called.borrow(), 0);
    }

    #[test]
    fn failed_switch_triggers_power_fallback_and_retries_once() {
        let config = load(r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#);
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(1),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostGainedFocus, &config, &ddc, &power);

        assert_eq!(ddc.calls.borrow().len(), 2);
        assert_eq!(*power.called.borrow(), 1);
    }

    #[test]
    fn unconfigured_direction_does_not_touch_backend() {
        let config = load(r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#);
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostLostFocus, &config, &ddc, &power);

        assert!(ddc.calls.borrow().is_empty());
    }

    #[test]
    fn manual_switch_calls_ddc_backend_with_given_input_and_configured_display() {
        let config = load(r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1", "on_usb_connect_source_addr": 80}"#);
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_manual_switch(InputSource::Symbolic(crate::config::SymbolicInputSource::DisplayPort1), &config, &ddc, &power);

        assert_eq!(*ddc.calls.borrow(), vec![(0, 0x60, 0x0f, Some(0x50))]);
    }

    #[test]
    fn run_processes_trigger_and_manual_events_through_the_same_handlers() {
        let config = load(r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#);
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(DaemonEvent::Trigger(TriggerEvent::HostGainedFocus)).unwrap();
        tx.send(DaemonEvent::ManualSwitch(InputSource::Symbolic(crate::config::SymbolicInputSource::Hdmi2))).unwrap();
        drop(tx);

        run(rx, &config, &ddc, &power);

        assert_eq!(
            *ddc.calls.borrow(),
            vec![(0, 0x60, 0x11, None), (0, 0x60, 0x12, None)]
        );
    }
}
