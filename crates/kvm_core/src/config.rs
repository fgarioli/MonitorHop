use anyhow::Result;
use paste::paste;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};
use std::convert::TryFrom;
use std::fmt;

macro_rules! symbolic_input_source {
    ($($name:ident: $value:expr)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum SymbolicInputSource {
            $($name = $value,)*
        }

        impl TryFrom<u16> for SymbolicInputSource {
            type Error = ();
            fn try_from(v: u16) -> std::result::Result<Self, Self::Error> {
                match v {
                    $($value => Ok(Self::$name),)*
                    _ => Err(()),
                }
            }
        }

        impl TryFrom<&str> for SymbolicInputSource {
            type Error = ();
            fn try_from(v: &str) -> std::result::Result<Self, Self::Error> {
                paste! {
                    match v.to_lowercase().as_str() {
                        $(stringify!([< $name:lower >]) => Ok(Self::$name),)*
                        _ => Err(()),
                    }
                }
            }
        }
    }
}

symbolic_input_source! {
    DisplayPort1: 0x0f
    DisplayPort2: 0x10
    Hdmi1: 0x11
    Hdmi2: 0x12
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Raw(u16),
    Symbolic(SymbolicInputSource),
}

impl InputSource {
    pub fn value(&self) -> u16 {
        match self {
            Self::Symbolic(sym) => *sym as u16,
            Self::Raw(value) => *value,
        }
    }

    fn normalize(self) -> Self {
        match self {
            Self::Symbolic(_) => self,
            Self::Raw(value) => SymbolicInputSource::try_from(value).map(Self::Symbolic).unwrap_or(Self::Raw(value)),
        }
    }
}

impl fmt::Debug for InputSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symbolic(sym) => write!(f, "{:?}(0x{:x})", sym, *sym as u16),
            Self::Raw(value) => write!(f, "Custom(0x{:x})", value),
        }
    }
}

fn parse_int(s: &str) -> std::result::Result<u16, std::num::ParseIntError> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
    } else {
        s.parse::<u16>()
    }
}

impl<'de> Deserialize<'de> for InputSource {
    fn deserialize<D>(deserializer: D) -> std::result::Result<InputSource, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?.trim().to_lowercase();
        if let Ok(val) = parse_int(&str) {
            Ok(Self::Raw(val).normalize())
        } else {
            SymbolicInputSource::try_from(str.as_str())
                .map(Self::Symbolic)
                .map_err(|_| D::Error::custom(format!("Invalid input source: {}", str)))
        }
    }
}

fn parse_source_addr<'de, D>(deserializer: D) -> std::result::Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => {
            let s = s.trim();
            let hex = s.strip_prefix("0x").unwrap_or(s);
            u8::from_str_radix(hex, 16)
                .map(Some)
                .map_err(|_| DeError::custom(format!("Invalid source address: {}", s)))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Configuration {
    #[serde(deserialize_with = "Configuration::deserialize_usb_device")]
    pub usb_device: String,
    pub on_usb_connect: Option<InputSource>,
    pub on_usb_disconnect: Option<InputSource>,
    #[serde(default, deserialize_with = "parse_source_addr")]
    pub on_usb_connect_source_addr: Option<u8>,
    #[serde(default)]
    pub nvapi_display_index: Option<u32>,
}

impl Configuration {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let builder = config::Config::builder().add_source(config::File::from(path));
        let config: Self = builder.build()?.try_deserialize()?;
        Ok(config)
    }

    fn deserialize_usb_device<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(s.to_lowercase())
    }

    pub fn display_index(&self) -> u32 {
        self.nvapi_display_index.unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::FileFormat::Ini;

    fn load_test_config(config_str: &str) -> Result<Configuration, config::ConfigError> {
        config::Config::builder()
            .add_source(config::File::from_str(config_str, Ini))
            .build()?
            .try_deserialize()
    }

    #[test]
    fn usb_device_is_lowercased() {
        let config = load_test_config(
            r#"
            usb_device = "17E9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.usb_device, "17e9:6000");
    }

    #[test]
    fn symbolic_input_source_resolves_to_vcp_value() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn hex_input_source_is_accepted() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "0x11"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn source_addr_defaults_to_none() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, None);
    }

    #[test]
    fn source_addr_override_is_parsed_as_hex() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
            on_usb_connect_source_addr = "0x50"
        "#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, Some(0x50));
    }

    #[test]
    fn display_index_defaults_to_zero() {
        let config = load_test_config(
            r#"
            usb_device = "17e9:6000"
            on_usb_connect = "Hdmi1"
        "#,
        )
        .unwrap();
        assert_eq!(config.display_index(), 0);
    }
}
