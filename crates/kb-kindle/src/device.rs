//! The Kindle's own controls for the quick settings panel, read and set through the system
//! property bus (`lipc-get-prop` / `lipc-set-prop`). The values and property names follow the
//! ones KOReader uses on the same firmware. Nothing here is fatal: a value that cannot be read
//! shows as unknown, and a change that fails leaves the panel as it was.

use std::process::Command;

use kb_app::{DeviceControl, DeviceStatus, LEVEL_MAX};

const POWERD: &str = "com.lab126.powerd";
const WIFID: &str = "com.lab126.wifid";
const CMD: &str = "com.lab126.cmd";
const BATTERY_SYSFS: &str = "/sys/class/power_supply/max77796-battery/capacity";

/// Reads and sets the Kindle's frontlight, warmth, battery, Wi-Fi and clock.
pub struct KindleDevice;

/// Read an integer system property, or `None` when it is missing or not a number.
fn get_int(service: &str, prop: &str) -> Option<u32> {
    let out = Command::new("lipc-get-prop")
        .args(["-i", service, prop])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Read a text system property (empty when it cannot be read).
fn get_text(service: &str, prop: &str) -> String {
    Command::new("lipc-get-prop")
        .args([service, prop])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Set an integer system property. Returns whether the call succeeded.
fn set_int(service: &str, prop: &str, value: u32) -> bool {
    Command::new("lipc-set-prop")
        .args(["-i", service, prop, &value.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The local clock time and date, e.g. "10:38" and "Mon 16 Nov".
fn local_time() -> (String, String) {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    // SAFETY: `time` and `localtime_r` only write into the buffers passed here.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return ("--:--".to_string(), String::new());
        }
        let clock = format!("{:02}:{:02}", tm.tm_hour, tm.tm_min);
        let day = DAYS.get(tm.tm_wday as usize).copied().unwrap_or("");
        let month = MONTHS.get(tm.tm_mon as usize).copied().unwrap_or("");
        (clock, format!("{day} {} {month}", tm.tm_mday))
    }
}

/// Battery level from the property bus, or from the battery's sysfs node if the bus has none.
fn battery_level() -> Option<u32> {
    get_int(POWERD, "battLevel").or_else(|| {
        std::fs::read_to_string(BATTERY_SYSFS)
            .ok()?
            .trim()
            .parse()
            .ok()
    })
}

fn wifi_label(state: &str, on: bool) -> String {
    if !on {
        return "off".to_string();
    }
    match state.to_ascii_uppercase().as_str() {
        "CONNECTED" => "connected".to_string(),
        "" => String::new(),
        _ => "not connected".to_string(),
    }
}

impl DeviceControl for KindleDevice {
    fn status(&mut self) -> DeviceStatus {
        let (clock, date) = local_time();
        let wifi_on = get_int(WIFID, "enable").map_or(false, |v| v != 0);
        let wifi_state = get_text(WIFID, "cmState");
        DeviceStatus {
            brightness: get_int(POWERD, "flIntensity").unwrap_or(0).min(LEVEL_MAX),
            warmth: get_int(POWERD, "currentAmberLevel").unwrap_or(0).min(LEVEL_MAX),
            battery: battery_level(),
            charging: get_int(POWERD, "isCharging").map_or(false, |v| v != 0),
            wifi_on,
            wifi_state: wifi_label(&wifi_state, wifi_on),
            clock,
            date,
        }
    }

    fn set_brightness(&mut self, level: u32) {
        set_int(POWERD, "flIntensity", level.min(LEVEL_MAX));
    }

    fn set_warmth(&mut self, level: u32) {
        set_int(POWERD, "currentAmberLevel", level.min(LEVEL_MAX));
    }

    fn set_wifi(&mut self, on: bool) {
        set_int(CMD, "wirelessEnable", u32::from(on));
    }
}
