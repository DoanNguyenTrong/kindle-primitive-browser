//! The device's own controls, for the quick settings panel: frontlight brightness and warmth,
//! battery, Wi-Fi, and the clock. The Kindle driver implements `DeviceControl` with the device's
//! system properties. Anywhere else, `NoDevice` answers with defaults and changes nothing.

/// The top of the brightness and warmth scales (both run from 0).
pub const LEVEL_MAX: u32 = 24;

/// What the quick settings panel shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceStatus {
    pub brightness: u32,
    pub warmth: u32,
    pub battery: Option<u32>,
    pub charging: bool,
    pub wifi_on: bool,
    /// "connected", "not connected", "off", or "" when unknown.
    pub wifi_state: String,
    /// The local time, "10:38".
    pub clock: String,
    /// The local date, "Mon 16 Nov".
    pub date: String,
}

impl DeviceStatus {
    /// The status shown where there is no device.
    pub fn unavailable() -> DeviceStatus {
        DeviceStatus {
            brightness: 12,
            warmth: 0,
            battery: None,
            charging: false,
            wifi_on: false,
            wifi_state: String::new(),
            clock: "--:--".to_string(),
            date: String::new(),
        }
    }
}

/// Reads and sets the device's controls.
pub trait DeviceControl {
    fn status(&mut self) -> DeviceStatus;
    fn set_brightness(&mut self, level: u32);
    fn set_warmth(&mut self, level: u32);
    fn set_wifi(&mut self, on: bool);
}

/// No device: the panel shows defaults, and changes stay on screen only.
pub struct NoDevice;

impl DeviceControl for NoDevice {
    fn status(&mut self) -> DeviceStatus {
        DeviceStatus::unavailable()
    }
    fn set_brightness(&mut self, _level: u32) {}
    fn set_warmth(&mut self, _level: u32) {}
    fn set_wifi(&mut self, _on: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_device_reports_defaults_and_ignores_changes() {
        let mut d = NoDevice;
        let s = d.status();
        assert_eq!(s.brightness, 12);
        assert!(s.battery.is_none());
        d.set_brightness(3);
        d.set_warmth(5);
        d.set_wifi(true);
        assert_eq!(d.status(), s);
    }
}
