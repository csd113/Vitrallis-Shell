//! X display preferences stay per-user; the original X session is not edited.
use super::{Hardware, Native};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::platform::system::SCREEN_TIMEOUTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Timer {
    timeout: u16,
    cycle: u16,
    standby: u16,
    suspend: u16,
    off: u16,
    enabled: bool,
}
impl Timer {
    fn parse(text: &str) -> Option<Self> {
        // XKB also prints a "Suspend: off" indicator above these sections.
        let timers = text.split_once("Screen Saver:")?.1;
        let words: Vec<_> = timers.split_whitespace().collect();
        let number = |label| {
            words
                .windows(2)
                .find(|pair| pair.first().is_some_and(|word| *word == label))?
                .get(1)?
                .parse()
                .ok()
        };
        Some(Self {
            timeout: number("timeout:")?,
            cycle: number("cycle:")?,
            standby: number("Standby:")?,
            suspend: number("Suspend:")?,
            off: number("Off:")?,
            enabled: timers.contains("DPMS is Enabled"),
        })
    }
    fn apply(self, io: &impl Hardware) -> Result<(), String> {
        io.command(
            "xset",
            &[
                "s",
                &self.timeout.to_string(),
                &self.cycle.to_string(),
                "dpms",
                &self.standby.to_string(),
                &self.suspend.to_string(),
                &self.off.to_string(),
                if self.enabled { "+dpms" } else { "-dpms" },
            ],
        )
        .map(|_| ())
    }
    const fn seconds(self) -> u16 {
        if self.enabled && self.off > 0 && (self.timeout == 0 || self.off < self.timeout) {
            self.off
        } else {
            self.timeout
        }
    }
}
pub fn timeout(io: &impl Hardware) -> Option<u16> {
    match io.command("xset", &["q"]) {
        Ok(text) => {
            let timer = Timer::parse(&text);
            if timer.is_none() {
                eprintln!("level=warn event=display_timer_unrecognized");
            }
            timer.map(Timer::seconds)
        }
        Err(error) => {
            eprintln!("level=warn event=display_timer_unavailable message={error:?}");
            None
        }
    }
}
fn setting_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or("Missing absolute home directory")?;
    Ok(home.join(".config/vitrallis/screen-timeout"))
}
fn safe(path: &Path) -> Result<(), String> {
    crate::app_center::storage::safe(path)?;
    if path.exists() && !path.is_file() {
        return Err("Screen timeout path must be a regular file".into());
    }
    Ok(())
}
enum Saved {
    Durable,
    SyncFailed(String),
}
fn save(path: &Path, seconds: u16) -> Result<Saved, String> {
    save_with_sync(path, seconds, crate::app_center::storage::sync)
}
fn save_with_sync(
    path: &Path,
    seconds: u16,
    sync: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<Saved, String> {
    safe(path)?;
    let parent = path.parent().ok_or("Missing settings directory")?;
    let mut directories = fs::DirBuilder::new();
    let _recursive_directories = directories.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let _private_directories = directories.mode(0o700);
    }
    directories.create(parent).map_err(|e| e.to_string())?;
    safe(path)?;
    let temp = parent.join(format!(".screen-timeout-{}", std::process::id()));
    let mut options = fs::OpenOptions::new();
    let _exclusive_options = options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let _private_options = options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        writeln!(file, "{seconds}")
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        safe(path)?;
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        // Rename commits the preference. A subsequent sync failure must not
        // restore the old X timer while leaving the new value on disk.
        Ok(match sync(parent) {
            Ok(()) => Saved::Durable,
            Err(error) => Saved::SyncFailed(error),
        })
    })();
    if let Err(error) = fs::remove_file(&temp)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(match result {
            Ok(_) => format!("Cannot remove screen timeout staging file: {error}"),
            Err(primary) => {
                format!("{primary}; cannot remove screen timeout staging file: {error}")
            }
        });
    }
    result
}
fn set(io: &impl Hardware, seconds: u16, path: Option<&Path>) -> Result<(), String> {
    set_with_save(io, seconds, path, save)
}
fn set_with_save(
    io: &impl Hardware,
    seconds: u16,
    path: Option<&Path>,
    persist: impl FnOnce(&Path, u16) -> Result<Saved, String>,
) -> Result<(), String> {
    if !SCREEN_TIMEOUTS.contains(&seconds) {
        return Err("Unsupported screen timeout".into());
    }
    let old = Timer::parse(&io.command("xset", &["q"])?).ok_or("Display sleep is unavailable")?;
    let new = Timer {
        timeout: seconds,
        cycle: seconds,
        standby: 0,
        suspend: 0,
        off: seconds,
        enabled: seconds > 0,
    };
    let result = new.apply(io).and_then(|()| {
        let readback = Timer::parse(&io.command("xset", &["q"])?)
            .ok_or("Display sleep readback unavailable")?;
        if readback != new {
            return Err("Display sleep setting was not accepted".into());
        }
        path.map_or(Ok(Saved::Durable), |destination| {
            persist(destination, seconds)
        })
    });
    if let Ok(Saved::SyncFailed(error)) = &result {
        eprintln!("level=warn event=screen_timeout_sync message={error:?}");
        return Err("Timer saved; storage sync failed. Reboot persistence is uncertain.".into());
    }
    if let Err(error) = result {
        return match old.apply(io) {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!(
                "{error}; restoring previous timer failed: {rollback}"
            )),
        };
    }
    Ok(())
}
pub fn apply(io: &impl Hardware, seconds: u16) -> Result<(), String> {
    set(io, seconds, Some(&setting_path()?))
}
pub fn restore() -> Result<(), String> {
    let path = setting_path()?;
    safe(&path)?;
    if !path.exists() {
        return Ok(());
    }
    let mut text = String::new();
    let _bytes_read = fs::File::open(&path)
        .and_then(|file| file.take(16).read_to_string(&mut text))
        .map_err(|e| e.to_string())?;
    set(
        &Native,
        text.trim()
            .parse()
            .map_err(|error| format!("Invalid saved screen timeout: {error}"))?,
        None,
    )
}

pub fn valid_zone(zone: &str) -> bool {
    zone.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && zone.len() <= 96
        && zone.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_+-".contains(&c))
        })
}
pub fn zones() -> Vec<String> {
    let mut text = String::new();
    let Ok(file) = fs::File::open("/usr/share/zoneinfo/zone.tab") else {
        return Vec::new();
    };
    if file.take(65537).read_to_string(&mut text).is_err() || text.len() > 65536 {
        return Vec::new();
    }
    let mut zones: Vec<_> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split('\t').nth(2))
        .filter(|zone| valid_zone(zone))
        .map(str::to_owned)
        .collect();
    zones.push("UTC".into());
    zones.sort();
    zones.dedup();
    zones
}
pub fn ip(text: &str) -> Option<std::net::Ipv4Addr> {
    text.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        let address = words
            .find(|word| *word == "inet")
            .and_then(|_| words.next())?
            .split('/')
            .next()?
            .parse::<std::net::Ipv4Addr>()
            .ok()?;
        (!address.is_loopback() && !address.is_unspecified() && !address.is_multicast())
            .then_some(address)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timer_zone_and_ip_validation() {
        let value = "XKB indicators: 06: Suspend: off\nScreen Saver:\n timeout: 600 cycle: 600\nStandby: 600 Suspend: 600 Off: 600\nDPMS is Enabled";
        assert_eq!(Timer::parse(value).map(Timer::seconds), Some(600));
        assert_eq!(Timer::parse("DPMS unavailable"), None);
        for zone in ["America/Vancouver", "UTC", "Etc/GMT+8"] {
            assert!(valid_zone(zone));
        }
        for zone in ["../etc/passwd", "/UTC", "a//b", "UTC\n", "--help", ""] {
            assert!(!valid_zone(zone));
        }
        assert_eq!(
            ip("3: wlan0 inet 10.0.0.137/24 scope global"),
            Some(std::net::Ipv4Addr::new(10, 0, 0, 137))
        );
        assert_eq!(ip("1: lo inet 127.0.0.1/8"), None);
        assert_eq!(ip("malformed"), None);
    }
    #[derive(Clone)]
    struct FakeDisplay(std::cell::Cell<Timer>);
    impl Hardware for FakeDisplay {
        fn read(&self, _: &str) -> Result<String, String> {
            Err("unexpected read".into())
        }
        fn write(&self, _: &str, _: &str) -> Result<(), String> {
            Err("unexpected write".into())
        }
        fn command(&self, name: &str, args: &[&str]) -> Result<String, String> {
            assert_eq!(name, "xset");
            if args == ["q"] {
                let t = self.0.get();
                return Ok(format!(
                    "Screen Saver: timeout: {} cycle: {} Standby: {} Suspend: {} Off: {} DPMS is {}",
                    t.timeout,
                    t.cycle,
                    t.standby,
                    t.suspend,
                    t.off,
                    if t.enabled { "Enabled" } else { "Disabled" }
                ));
            }
            assert_eq!(args.len(), 8);
            let number = |index: usize| {
                args.get(index)
                    .ok_or("missing timer argument")?
                    .parse()
                    .map_err(|error| format!("invalid timer: {error}"))
            };
            self.0.set(Timer {
                timeout: number(1)?,
                cycle: number(2)?,
                standby: number(4)?,
                suspend: number(5)?,
                off: number(6)?,
                enabled: args.get(7) == Some(&"+dpms"),
            });
            Ok(String::new())
        }
    }
    #[test]
    fn timer_apply_readback_and_failed_save_restore_original_display_state() -> Result<(), String> {
        let original = Timer {
            timeout: 600,
            cycle: 600,
            standby: 600,
            suspend: 600,
            off: 600,
            enabled: true,
        };
        let io = FakeDisplay(std::cell::Cell::new(original));
        assert!(set(&io, 13, None).is_err());
        assert_eq!(io.0.get(), original);
        for seconds in SCREEN_TIMEOUTS {
            set(&io, seconds, None)?;
            assert_eq!(timeout(&io), Some(seconds));
        }
        original.apply(&io)?;
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let path = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        // A directory cannot be replaced by the preference file.
        assert!(set(&io, 30, Some(&path)).is_err());
        assert_eq!(io.0.get(), original);
        Ok(())
    }
    #[test]
    fn preference_write_is_atomic_and_preserves_symlink_target() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let path = scratch
            .0
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("settings/screen-timeout");
        assert!(matches!(save(&path, 60)?, Saved::Durable));
        assert_eq!(
            fs::read_to_string(&path).map_err(|e| e.to_string())?,
            "60\n"
        );
        assert!(matches!(save(&path, 0)?, Saved::Durable));
        #[cfg(unix)]
        {
            let link = scratch.0.join("link");
            std::os::unix::fs::symlink(&path, &link).map_err(|e| e.to_string())?;
            assert!(save(&link, 600).is_err());
            assert_eq!(fs::read_to_string(&path).map_err(|e| e.to_string())?, "0\n");
        }
        Ok(())
    }
    #[test]
    fn committed_timeout_sync_failure_keeps_timer_and_reports_uncertainty() -> Result<(), String> {
        let io = FakeDisplay(std::cell::Cell::new(Timer {
            timeout: 600,
            cycle: 600,
            standby: 600,
            suspend: 600,
            off: 600,
            enabled: true,
        }));
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let path = scratch
            .0
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("screen-timeout");
        let result = set_with_save(&io, 30, Some(&path), |save_path, seconds| {
            save_with_sync(save_path, seconds, |_| {
                Err("injected directory sync failure".into())
            })
        });
        assert!(result.is_err_and(|error| error.contains("persistence is uncertain")));
        assert_eq!(timeout(&io), Some(30));
        assert_eq!(fs::read_to_string(path).map_err(|e| e.to_string())?, "30\n");
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn timeout_storage_rejects_unsafe_paths_before_creating_directories() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        for path in [
            root.join("new/../screen-timeout"),
            root.join("control\nscreen-timeout"),
        ] {
            assert!(save(&path, 30).is_err());
        }
        assert!(!root.join("new").exists());
        let unsafe_parent = root.join("unsafe");
        fs::create_dir(&unsafe_parent).map_err(|e| e.to_string())?;
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o775))
            .map_err(|e| e.to_string())?;
        assert!(save(&unsafe_parent.join("settings/screen-timeout"), 30).is_err());
        assert!(!unsafe_parent.join("settings").exists());
        let path = root.join("private/settings/screen-timeout");
        assert!(matches!(save(&path, 60)?, Saved::Durable));
        for directory in [root.join("private"), root.join("private/settings")] {
            assert_eq!(
                fs::metadata(directory)
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        Ok(())
    }
}
