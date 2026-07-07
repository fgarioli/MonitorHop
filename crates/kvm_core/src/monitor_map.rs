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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Configuration;

    fn load(config_str: &str) -> Configuration {
        config::Config::builder()
            .add_source(config::File::from_str(config_str, config::FileFormat::Ini))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap()
    }

    #[test]
    fn resolves_connect_target_from_config() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_source_addr = "0x50"
        "#,
        );
        let target = resolve(&config, SwitchDirection::Connect).unwrap();
        assert_eq!(target.display_index, 0);
        assert_eq!(target.input_source.value(), 0x11);
        assert_eq!(target.source_addr, Some(0x50));
    }

    #[test]
    fn disconnect_with_no_config_resolves_to_none() {
        let config = load(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        );
        assert!(resolve(&config, SwitchDirection::Disconnect).is_none());
    }
}
