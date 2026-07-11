use anyhow::{Context, Result};
use paste::paste;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

        impl SymbolicInputSource {
            fn label(&self) -> &'static str {
                paste! {
                    match self {
                        $(Self::$name => stringify!([< $name:lower >]),)*
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
                .map_err(|_| serde::de::Error::custom(format!("Invalid input source: {}", str)))
        }
    }
}

impl Serialize for InputSource {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Symbolic(sym) => serializer.serialize_str(sym.label()),
            Self::Raw(value) => serializer.serialize_str(&format!("0x{value:x}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(deserialize_with = "Configuration::deserialize_usb_device")]
    pub usb_device: String,
    #[serde(default)]
    pub mxkeys_usb_device: Option<String>,
    pub on_usb_connect: Option<InputSource>,
    pub on_usb_disconnect: Option<InputSource>,
    #[serde(default)]
    pub on_usb_connect_source_addr: Option<u8>,
    /// VCP feature code for input select. Defaults to the DDC/CI standard
    /// `0x60` — see `vcp_code()`. Only macOS is expected to need an override
    /// (see `docs/superpowers/specs/2026-07-07-macos-backend-design.md`);
    /// Windows' validated recipe (DECISIONS.md #4) always uses `0x60`.
    #[serde(default)]
    pub on_usb_connect_vcp_code: Option<u8>,
    #[serde(default)]
    pub display_index: Option<u32>,
}

impl Configuration {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).with_context(|| format!("failed to read {:?}", path))?;
        serde_json::from_str(&contents).with_context(|| format!("failed to parse {:?} as JSON", path))
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let contents = serde_json::to_string_pretty(self).context("failed to serialize configuration")?;
        std::fs::write(path, contents).with_context(|| format!("failed to write {:?}", path))
    }

    fn deserialize_usb_device<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(s.to_lowercase())
    }

    pub fn display_index(&self) -> u32 {
        self.display_index.unwrap_or(0)
    }

    /// VCP feature code for input select — `0x60` (DDC/CI standard,
    /// `orchestrator::INPUT_SELECT`) unless overridden.
    pub fn vcp_code(&self) -> u8 {
        self.on_usb_connect_vcp_code.unwrap_or(0x60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_device_is_lowercased() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17E9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.usb_device, "17e9:6000");
    }

    #[test]
    fn symbolic_input_source_resolves_to_vcp_value() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn hex_input_source_is_accepted() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "0x11"}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect.unwrap().value(), 0x11);
    }

    #[test]
    fn source_addr_defaults_to_none() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, None);
    }

    #[test]
    fn source_addr_override_is_a_plain_number() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1", "on_usb_connect_source_addr": 80}"#,
        )
        .unwrap();
        assert_eq!(config.on_usb_connect_source_addr, Some(0x50));
    }

    #[test]
    fn display_index_defaults_to_zero() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.display_index(), 0);
    }

    #[test]
    fn vcp_code_defaults_to_input_select() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.vcp_code(), 0x60);
    }

    #[test]
    fn mxkeys_usb_device_defaults_to_none() {
        let config: Configuration = serde_json::from_str(
            r#"{"usb_device": "17e9:6000", "on_usb_connect": "Hdmi1"}"#,
        )
        .unwrap();
        assert_eq!(config.mxkeys_usb_device, None);
    }

    #[test]
    fn round_trips_through_save_and_load() {
        let config = Configuration {
            usb_device: "17e9:6000".to_string(),
            mxkeys_usb_device: Some("046d:c52b".to_string()),
            on_usb_connect: Some(InputSource::Symbolic(SymbolicInputSource::Hdmi1)),
            on_usb_disconnect: None,
            on_usb_connect_source_addr: Some(0x50),
            on_usb_connect_vcp_code: None,
            display_index: Some(0),
        };
        let dir = std::env::temp_dir().join(format!("kvm-switch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        config.save(&path).unwrap();
        let loaded = Configuration::load(&path).unwrap();

        assert_eq!(loaded.usb_device, config.usb_device);
        assert_eq!(loaded.mxkeys_usb_device, config.mxkeys_usb_device);
        assert_eq!(loaded.on_usb_connect.unwrap().value(), 0x11);
        assert_eq!(loaded.on_usb_connect_source_addr, Some(0x50));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
