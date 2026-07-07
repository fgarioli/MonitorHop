use crate::config::Configuration;
use crate::monitor_map::{self, SwitchDirection};
use ddc_backend::DdcBackend;
use power_fallback::PowerFallback;
use trigger::TriggerEvent;

/// VCP feature code for input select (DDC/CI standard) — see DECISIONS.md #4.
const INPUT_SELECT: u8 = 0x60;

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
    let attempt = |ddc_backend: &dyn DdcBackend| {
        ddc_backend.set_vcp(target.display_index, INPUT_SELECT, target.input_source.value(), target.source_addr)
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
        log::info!("Display switched to {:?} for {:?}", target.input_source, direction);
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
        config::Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Ini))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap()
    }

    #[test]
    fn successful_switch_calls_ddc_backend_once_with_resolved_target() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_source_addr = "0x50"
        "#,
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
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        );
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
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        );
        let ddc = FakeDdc {
            calls: RefCell::new(vec![]),
            fail_first_n: RefCell::new(0),
        };
        let power = FakePower { called: RefCell::new(0) };

        handle_event(TriggerEvent::HostLostFocus, &config, &ddc, &power);

        assert!(ddc.calls.borrow().is_empty());
    }
}
