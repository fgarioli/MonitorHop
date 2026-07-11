use crate::config::{Configuration, InputSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchDirection {
    Connect,
    Disconnect,
}

pub struct SwitchTarget {
    pub display_index: u32,
    pub input_source: InputSource,
    pub source_addr: Option<u8>,
    pub vcp_code: u8,
}

pub fn resolve(config: &Configuration, direction: SwitchDirection) -> Option<SwitchTarget> {
    let input_source = match direction {
        SwitchDirection::Connect => config.on_usb_connect,
        SwitchDirection::Disconnect => config.on_usb_disconnect,
    }?;
    Some(SwitchTarget {
        display_index: config.display_index(),
        input_source,
        source_addr: config.on_usb_connect_source_addr,
        vcp_code: config.vcp_code(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Configuration;

    #[test]
    fn resolves_connect_target_from_config() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1", "on_usb_connect_source_addr": 80}"#,
        )
        .unwrap();
        let target = resolve(&config, SwitchDirection::Connect).unwrap();
        assert_eq!(target.display_index, 0);
        assert_eq!(target.input_source.value(), 0x11);
        assert_eq!(target.source_addr, Some(0x50));
        assert_eq!(target.vcp_code, 0x60);
    }

    #[test]
    fn disconnect_with_no_config_resolves_to_none() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert!(resolve(&config, SwitchDirection::Disconnect).is_none());
    }
}
