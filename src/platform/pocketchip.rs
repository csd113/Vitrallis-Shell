use super::Platform;

#[derive(Debug, Clone, Copy)]
pub struct PocketChip;
impl Platform for PocketChip {
    fn fullscreen(&self) -> bool {
        true
    }
    fn resolution(&self) -> (u16, u16) {
        (480, 272)
    }
}

use super::{
    command,
    system::{Control, Percent, Power, Status, System, Wifi},
};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
const BRIGHTNESS: &str = "/sys/class/backlight/backlight/brightness";
const MAX_BRIGHTNESS: &str = "/sys/class/backlight/backlight/max_brightness";

trait Hardware {
    fn kernel_battery(&self) -> bool {
        false
    }
    fn read(&self, path: &str) -> Result<String, String>;
    fn write(&self, path: &str, value: &str) -> Result<(), String>;
    fn command(&self, name: &str, args: &[&str]) -> Result<String, String>;
}
struct Native;
impl Hardware for Native {
    fn kernel_battery(&self) -> bool {
        Path::new("/sys/class/power_supply/axp20x-battery").exists()
    }
    fn read(&self, path: &str) -> Result<String, String> {
        let mut value = String::new();
        File::open(path)
            .map_err(|e| e.to_string())?
            .take(129)
            .read_to_string(&mut value)
            .map_err(|e| e.to_string())?;
        if value.len() > 128 {
            return Err("oversized hardware value".into());
        }
        Ok(value)
    }
    fn write(&self, path: &str, value: &str) -> Result<(), String> {
        // Never create a missing sysfs node.
        use std::io::Write;
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|mut file| file.write_all(value.as_bytes()))
            .map_err(|e| e.to_string())
    }
    fn command(&self, name: &str, args: &[&str]) -> Result<String, String> {
        // Do not resolve privileged operations through the inherited PATH.
        let path = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"]
            .into_iter()
            .map(|dir| format!("{dir}/{name}"))
            .find(|path| Path::new(path).is_file())
            .ok_or_else(|| format!("{name} unavailable"))?;
        command::run(&path, args)
    }
}
impl System for PocketChip {
    fn refresh(&mut self) -> Status {
        snapshot(&Native)
    }
    fn control(&mut self, control: Control, status: &mut Status) -> Result<(), String> {
        apply(&Native, control)?;
        match control {
            Control::Brightness(_) => {
                status.brightness = brightness_percent(&Native);
                status.brightness.ok_or("brightness readback unavailable")?;
            }
            Control::Volume(_) => {
                status.volume = None;
                status.muted = None;
                let (volume, muted) =
                    audio(&Native.command("amixer", &["sget", "Power Amplifier"])?)?;
                status.volume = Some(volume);
                status.muted = muted;
            }
            Control::Power(_) => {}
        }
        Ok(())
    }
}
fn byte(value: &str) -> Result<u8, String> {
    let value = value
        .trim()
        .strip_prefix("0x")
        .ok_or("expected hexadecimal register")?;
    if value.len() != 2 {
        return Err("expected one register byte".into());
    }
    u8::from_str_radix(value, 16).map_err(|e| e.to_string())
}
fn register(io: &impl Hardware, address: &str) -> Option<u8> {
    // Same forced bus/address access as the reference; i2c-tools is optional.
    byte(
        &io.command("i2cget", &["-y", "-f", "0", "0x34", address, "b"])
            .ok()?,
    )
    .ok()
}
fn brightness(io: &impl Hardware) -> Result<(u8, u8), String> {
    let max = io
        .read(MAX_BRIGHTNESS)?
        .trim()
        .parse::<u8>()
        .map_err(|e| e.to_string())?;
    let current = io
        .read(BRIGHTNESS)?
        .trim()
        .parse::<u8>()
        .map_err(|e| e.to_string())?;
    // Marshmallow writes native levels 1..10. Fail closed on another driver.
    if max != 10 || !(1..=max).contains(&current) {
        return Err("unsupported backlight range".into());
    }
    Ok((current, max))
}
fn brightness_percent(io: &impl Hardware) -> Option<Percent> {
    let (level, _) = brightness(io).ok()?;
    Percent::new(u8::try_from(u16::from(level - 1) * 100 / 9).ok()?).ok()
}

fn brightness_level(percent: Percent) -> u8 {
    1 + u8::try_from((u16::from(percent.value()) * 9 + 50) / 100).unwrap_or(9)
}
fn audio(value: &str) -> Result<(Percent, Option<bool>), String> {
    let mut level = None;
    let mut muted = None;
    for field in value
        .split('[')
        .skip(1)
        .filter_map(|s| s.split_once(']').map(|p| p.0))
    {
        if let Some(number) = field.strip_suffix('%') {
            let parsed = Percent::new(number.parse::<u8>().map_err(|e| e.to_string())?)?;
            if level.is_some_and(|previous| previous != parsed) {
                return Err("unequal audio channels".into());
            }
            level = Some(parsed);
        } else if matches!(field, "on" | "off") {
            let parsed = field == "off";
            if muted.is_some_and(|previous| previous != parsed) {
                return Err("unequal mute channels".into());
            }
            muted = Some(parsed);
        }
    }
    Ok((level.ok_or("missing audio percentage")?, muted))
}
fn wifi(radio: &str, state: &str) -> Option<Wifi> {
    match radio.trim() {
        "disabled" => Some(Wifi::Off),
        "enabled" => match state
            .trim()
            .strip_prefix("GENERAL.STATE:")?
            .split_whitespace()
            .next()?
        {
            "30" | "110" | "120" => Some(Wifi::Disconnected),
            "40" | "50" | "60" | "70" | "80" | "90" => Some(Wifi::Connecting),
            "100" => Some(Wifi::Connected),
            _ => None,
        },
        _ => None,
    }
}
fn snapshot(io: &impl Hardware) -> Status {
    let (battery, charging, external_power) = battery_status(io);
    let audio = io
        .command("amixer", &["sget", "Power Amplifier"])
        .ok()
        .and_then(|v| audio(&v).ok());
    let radio = io.command("nmcli", &["radio", "wifi"]);
    let wifi = match radio {
        Ok(radio) if radio.trim() == "disabled" => Some(Wifi::Off),
        Ok(radio) => io
            .command(
                "nmcli",
                &["-t", "-f", "GENERAL.STATE", "device", "show", "wlan0"],
            )
            .ok()
            .and_then(|state| wifi(&radio, &state)),
        Err(_) => None,
    };
    Status {
        battery,
        charging,
        external_power,
        wifi,
        bluetooth: None,
        brightness: brightness_percent(io),
        volume: audio.map(|v| v.0),
        muted: audio.and_then(|v| v.1),
        clock: command::clock(),
        power_controls: cfg!(target_os = "linux"),
    }
}
fn battery_status(io: &impl Hardware) -> (Option<Percent>, Option<bool>, Option<bool>) {
    if io.kernel_battery() {
        // A bound kernel driver owns the PMIC. Malformed sysfs data must never
        // trigger forced I2C access behind that driver's back.
        let read = |name| io.read(name).ok();
        let present = read("/sys/class/power_supply/axp20x-battery/present");
        let battery = (present.as_deref().map(str::trim) == Some("1"))
            .then(|| read("/sys/class/power_supply/axp20x-battery/capacity"))
            .flatten()
            .and_then(|v| v.trim().parse().ok())
            .and_then(|v| Percent::new(v).ok());
        let charging =
            read("/sys/class/power_supply/axp20x-battery/status").and_then(|v| match v.trim() {
                "Charging" => Some(true),
                "Discharging" | "Not charging" | "Full" => Some(false),
                _ => None,
            });
        let online = |path| {
            read(path).and_then(|v| match v.trim() {
                "0" => Some(false),
                "1" => Some(true),
                _ => None,
            })
        };
        let usb = online("/sys/class/power_supply/axp20x-usb/online");
        let ac = online("/sys/class/power_supply/axp20x-ac/online");
        let power = match (usb, ac) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        };
        return (battery, charging, power);
    }
    let power = register(io, "0x00");
    let charge = register(io, "0x01");
    let battery = register(io, "0xb9")
        .and_then(|v| Percent::new(v).ok())
        .filter(|_| charge.is_some_and(|v| v & 0x20 != 0));
    (
        battery,
        charge.map(|v| v & 0x60 == 0x60),
        power.map(|v| v & 0x50 != 0),
    )
}
fn apply(io: &impl Hardware, control: Control) -> Result<(), String> {
    match control {
        Control::Brightness(value) => {
            brightness(io)?;
            io.write(BRIGHTNESS, &format!("{}\n", brightness_level(value)))
        }
        Control::Volume(value) => {
            audio(&io.command("amixer", &["sget", "Power Amplifier"])?)?;
            io.command(
                "amixer",
                &["sset", "Power Amplifier", &format!("{}%", value.value())],
            )
            .map(|_| ())
        }
        Control::Power(power) => io
            .command(
                "systemctl",
                &[
                    "--no-ask-password",
                    match power {
                        Power::Reboot => "reboot",
                        Power::Shutdown => "poweroff",
                    },
                ],
            )
            .map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kernel_battery_uses_sysfs_and_never_falls_back_to_i2c() -> Result<(), String> {
        struct Kernel {
            capacity: &'static str,
            present: &'static str,
            status: &'static str,
        }
        impl Hardware for Kernel {
            fn kernel_battery(&self) -> bool {
                true
            }
            fn read(&self, path: &str) -> Result<String, String> {
                let value = match path {
                    "/sys/class/power_supply/axp20x-battery/capacity" => self.capacity,
                    "/sys/class/power_supply/axp20x-battery/present" => self.present,
                    "/sys/class/power_supply/axp20x-battery/status" => self.status,
                    "/sys/class/power_supply/axp20x-usb/online" => "1",
                    "/sys/class/power_supply/axp20x-ac/online" => "0",
                    _ => return Err("missing".into()),
                };
                Ok(value.into())
            }
            fn write(&self, _: &str, _: &str) -> Result<(), String> {
                panic!("battery reads must not write")
            }
            fn command(&self, _: &str, _: &[&str]) -> Result<String, String> {
                panic!("kernel battery must never use forced I2C")
            }
        }
        let mut io = Kernel {
            capacity: "95\n",
            present: "1",
            status: "Charging\n",
        };
        assert_eq!(
            battery_status(&io),
            (Some(Percent::new(95)?), Some(true), Some(true))
        );
        io.capacity = "101";
        assert_eq!(battery_status(&io).0, None);
        io.capacity = "95";
        io.present = "0";
        assert_eq!(battery_status(&io).0, None);
        io.status = "broken";
        assert_eq!(battery_status(&io).1, None);
        io.status = "Full";
        assert_eq!(battery_status(&io).1, Some(false));
        Ok(())
    }
    use std::cell::RefCell;
    struct Fake {
        current: &'static str,
        max: &'static str,
        writes: RefCell<Vec<String>>,
        registers: Option<[u8; 3]>,
    }
    impl Hardware for Fake {
        fn read(&self, path: &str) -> Result<String, String> {
            Ok(if path == BRIGHTNESS {
                self.current
            } else {
                self.max
            }
            .into())
        }
        fn write(&self, path: &str, value: &str) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("{path}={value}"));
            Ok(())
        }
        fn command(&self, name: &str, args: &[&str]) -> Result<String, String> {
            self.writes.borrow_mut().push(format!("{name} {args:?}"));
            if name == "i2cget" {
                if let Some(registers) = self.registers {
                    let index = match args.get(4) {
                        Some(&"0x00") => 0,
                        Some(&"0x01") => 1,
                        Some(&"0xb9") => 2,
                        _ => return Err("unexpected register".into()),
                    };
                    return Ok(format!("0x{:02x}", registers[index]));
                }
            }
            Err("unavailable".into())
        }
    }
    #[test]
    fn parsers_reject_malformed_data() -> Result<(), String> {
        assert_eq!(byte("0x64\n")?, 100);
        for value in ["64", "0x100", "0xgg", "", "0x01 extra"] {
            assert!(byte(value).is_err());
        }
        assert_eq!(
            audio("Mono: Playback 15 [50%] [-5dB] [off]")?,
            (Percent::new(50)?, Some(true))
        );
        for value in ["", "[101%]", "[-1%]", "[50%] [60%]", "[4%] [on] [off]"] {
            assert!(audio(value).is_err());
        }
        assert_eq!(
            wifi("enabled", "GENERAL.STATE:100 (connected)"),
            Some(Wifi::Connected)
        );
        assert_eq!(wifi("disabled", ""), Some(Wifi::Off));
        assert_eq!(wifi("enabled", "GENERAL.STATE:20 (unavailable)"), None);
        assert_eq!(wifi("bad", "GENERAL.STATE:100"), None);
        Ok(())
    }
    #[test]
    fn backlight_is_bounded_and_missing_data_is_nonfatal() -> Result<(), String> {
        let mut io = Fake {
            current: "5",
            max: "10",
            writes: RefCell::default(),
            registers: None,
        };
        for value in 0..=100 {
            let value = Percent::new(value)?;
            assert!((1..=10).contains(&brightness_level(value)));
            apply(&io, Control::Brightness(value))?;
        }
        assert!(
            io.writes
                .borrow()
                .last()
                .is_some_and(|s| s.ends_with("=10\n"))
        );
        io.max = "255";
        let count = io.writes.borrow().len();
        assert!(apply(&io, Control::Brightness(Percent::new(100)?)).is_err());
        assert_eq!(io.writes.borrow().len(), count);
        io.current = "broken";
        let status = snapshot(&io);
        assert_eq!(status.battery, None);
        assert_eq!(status.brightness, None);
        assert_eq!(status.wifi, None);
        assert_eq!(status.volume, None);
        assert_eq!(status.bluetooth, None);
        Ok(())
    }
    #[test]
    fn battery_flags_distinguish_external_power_charging_and_invalid_gauge() -> Result<(), String> {
        let mut io = Fake {
            current: "5",
            max: "10",
            writes: RefCell::default(),
            registers: Some([0x10, 0x60, 73]),
        };
        let status = snapshot(&io);
        assert_eq!(status.battery, Some(Percent::new(73)?));
        assert_eq!(status.charging, Some(true));
        assert_eq!(status.external_power, Some(true));
        io.registers = Some([0x10, 0x20, 100]);
        assert_eq!(snapshot(&io).charging, Some(false)); // Full on external power.
        io.registers = Some([0x01, 0x20, 0x7f]);
        let status = snapshot(&io);
        assert_eq!(status.battery, None); // Gauge not initialized.
        assert_eq!(status.external_power, Some(false)); // Boot source is not live power.
        io.registers = Some([0, 0, 73]);
        assert_eq!(snapshot(&io).battery, None); // No battery.
        io.registers = Some([0, 0x20, 0x80 | 0x49]);
        assert_eq!(snapshot(&io).battery, None); // Suspended gauge.
        Ok(())
    }
    #[test]
    fn power_uses_only_fixed_arguments_and_propagates_denial() {
        let io = Fake {
            current: "1",
            max: "10",
            writes: RefCell::default(),
            registers: None,
        };
        assert!(apply(&io, Control::Power(Power::Shutdown)).is_err());
        assert_eq!(
            io.writes.borrow()[0],
            "systemctl [\"--no-ask-password\", \"poweroff\"]"
        );
    }
}
