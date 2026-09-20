//! Bounded radio controls with authoritative readback, never optimistic UI state.
use super::{Hardware, Status, Wifi};
use crate::platform::system::Radio;

pub(super) fn read(io: &impl Hardware, radio: Radio) -> Result<bool, String> {
    match radio {
        Radio::Wifi => match io.command("nmcli", &["radio", "wifi"])?.trim() {
            "enabled" => Ok(true),
            "disabled" => Ok(false),
            _ => Err("Wi-Fi radio state unavailable".into()),
        },
        Radio::Bluetooth => powered(&io.command("bluetoothctl", &["--timeout", "1", "show"])?),
    }
}
fn powered(output: &str) -> Result<bool, String> {
    let mut values = output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Powered:"));
    let value = match values.next().map(str::trim) {
        Some("yes") => true,
        Some("no") => false,
        _ => return Err("Bluetooth controller unavailable".into()),
    };
    if values.next().is_some() {
        return Err("Ambiguous Bluetooth power state".into());
    }
    Ok(value)
}
pub(super) fn connection(io: &impl Hardware, enabled: Option<bool>) -> Option<Wifi> {
    match enabled {
        Some(false) => Some(Wifi::Off),
        Some(true) => io
            .command(
                "nmcli",
                &["-t", "-f", "GENERAL.STATE", "device", "show", "wlan0"],
            )
            .ok()
            .and_then(|state| super::wifi("enabled", &state)),
        None => None,
    }
}
pub(super) fn apply(
    io: &impl Hardware,
    radio: Radio,
    enabled: bool,
    status: &mut Status,
) -> Result<(), String> {
    if let Err(error) = read(io, radio) {
        match radio {
            Radio::Wifi => {
                status.wifi_enabled = None;
                status.wifi = None;
            }
            Radio::Bluetooth => status.bluetooth = None,
        }
        return Err(error);
    }
    let value = if enabled { "on" } else { "off" };
    let result = match radio {
        Radio::Wifi => io.command("nmcli", &["radio", "wifi", value]),
        Radio::Bluetooth => io.command("bluetoothctl", &["--timeout", "1", "power", value]),
    };
    // bluetoothctl can exit successfully after reporting a D-Bus error in its output.
    // Only readback can establish success, including when an adapter disappears.
    let actual = read(io, radio).ok();
    match radio {
        Radio::Wifi => {
            status.wifi_enabled = actual;
            status.wifi = connection(io, actual);
            status.ip = super::ip(io);
        }
        Radio::Bluetooth => status.bluetooth = actual,
    }
    result.map_err(|_| format!("{} change denied or unavailable", radio.label()))?;
    if actual != Some(enabled) {
        return Err(format!(
            "{} change not confirmed; check adapter / radio block",
            radio.label()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Fake {
        enabled: Cell<bool>,
        missing: Cell<bool>,
        apply: bool,
        denied: bool,
        calls: RefCell<Vec<String>>,
    }
    impl Hardware for Fake {
        fn read(&self, _: &str) -> Result<String, String> {
            Err("missing".into())
        }
        fn write(&self, _: &str, _: &str) -> Result<(), String> {
            panic!("No sysfs writes")
        }
        fn command(&self, name: &str, args: &[&str]) -> Result<String, String> {
            self.calls
                .borrow_mut()
                .push(format!("{name} {}", args.join(" ")));
            if self.missing.get() {
                return Err("no adapter".into());
            }
            if matches!(args.last(), Some(&"on" | &"off")) {
                if self.denied {
                    return Err("permission denied".into());
                }
                if self.apply {
                    self.enabled.set(args.last() == Some(&"on"));
                }
                return Ok("Failed to set power: blocked".into());
            }
            match name {
                "nmcli" if args == ["radio", "wifi"] => Ok(if self.enabled.get() {
                    "enabled"
                } else {
                    "disabled"
                }
                .into()),
                "bluetoothctl" => Ok(format!(
                    "Controller 00:11:22:33:44:55\n Powered: {}\n",
                    if self.enabled.get() { "yes" } else { "no" }
                )),
                "nmcli" => Ok("GENERAL.STATE:30 (disconnected)".into()),
                _ => Err("missing".into()),
            }
        }
    }
    fn fake() -> Fake {
        Fake {
            enabled: Cell::new(false),
            missing: Cell::new(false),
            apply: true,
            denied: false,
            calls: RefCell::default(),
        }
    }
    #[test]
    fn radios_toggle_both_directions_using_fixed_commands_and_readback() -> Result<(), String> {
        for radio in [Radio::Wifi, Radio::Bluetooth] {
            let io = fake();
            let mut status = Status::default();
            for enabled in [true, false] {
                apply(&io, radio, enabled, &mut status)?;
                assert_eq!(
                    if radio == Radio::Wifi {
                        status.wifi_enabled
                    } else {
                        status.bluetooth
                    },
                    Some(enabled)
                );
            }
            let command = if radio == Radio::Wifi {
                "nmcli radio wifi on"
            } else {
                "bluetoothctl --timeout 1 power on"
            };
            assert!(io.calls.borrow().iter().any(|call| call == command));
        }
        Ok(())
    }
    #[test]
    fn missing_denied_and_unconfirmed_radios_do_not_claim_success() {
        for radio in [Radio::Wifi, Radio::Bluetooth] {
            let mut io = fake();
            let mut status = Status::default();
            io.missing.set(true);
            assert!(apply(&io, radio, true, &mut status).is_err());
            assert_eq!(io.calls.borrow().len(), 1);
            io.missing.set(false);
            io.denied = true;
            assert!(apply(&io, radio, true, &mut status).is_err());
            io.denied = false;
            io.apply = false;
            assert!(apply(&io, radio, true, &mut status).is_err());
            assert_eq!(
                if radio == Radio::Wifi {
                    status.wifi_enabled
                } else {
                    status.bluetooth
                },
                Some(false)
            );
        }
        for value in [
            "",
            "No default controller available",
            "Powered: maybe",
            "Powered: yes\nPowered: no",
        ] {
            assert!(powered(value).is_err());
        }
    }
}
