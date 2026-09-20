//! Normalized display preferences and clock formatting.
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preferences {
    pub color: [u8; 3],
    pub wallpaper: Option<PathBuf>,
    pub show_clock: bool,
    pub ampm: bool,
    pub show_cursor: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            color: {
                let color = vitrallis_native::theme::BACKGROUND;
                [color.r, color.g, color.b]
            },
            wallpaper: None,
            show_clock: true,
            ampm: false,
            show_cursor: true,
        }
    }
}
impl Preferences {
    pub fn clock(&self, clock: Option<&str>) -> String {
        if !self.show_clock {
            return String::new();
        }
        let Some(clock) = clock else {
            return "--:--".into();
        };
        if self.ampm
            && let Some((hours, minutes)) = clock.split_once(':')
            && let Ok(hour) = hours.parse::<u8>()
            && hour < 24
            && minutes.parse::<u8>().is_ok_and(|m| m < 60)
        {
            return format!(
                "{}:{minutes}{}",
                (hour + 11) % 12 + 1,
                if hour < 12 { "AM" } else { "PM" }
            );
        }
        clock.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_respects_midnight_noon_and_hidden_preference() {
        let mut prefs = Preferences {
            ampm: true,
            ..Preferences::default()
        };
        assert_eq!(prefs.clock(Some("00:05")), "12:05AM");
        assert_eq!(prefs.clock(Some("12:30")), "12:30PM");
        assert_eq!(prefs.clock(Some("23:59")), "11:59PM");
        assert_eq!(prefs.clock(None), "--:--");
        prefs.show_clock = false;
        assert_eq!(prefs.clock(Some("23:59")), "");
    }
}

/// User choices independent of a refreshed app catalog. Zero disables auto-close.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    pub ampm: bool,
    pub background_seconds: u32,
    pub essential: std::collections::BTreeSet<String>,
}
impl Policy {
    fn path() -> Result<PathBuf, String> {
        Ok(crate::app_center::storage::Locations::current()?
            .sources
            .with_file_name("preferences.json"))
    }
    pub fn load(ampm: bool) -> Result<Self, String> {
        let path = Self::path()?;
        if crate::app_center::storage::read(&path, 512 * 1024)?.is_none() {
            Ok(Self {
                ampm,
                ..Self::default()
            })
        } else {
            Self::read(&path)
        }
    }
    fn read(path: &std::path::Path) -> Result<Self, String> {
        let Some(file) = crate::app_center::storage::read(path, 512 * 1024)? else {
            return Ok(Self::default());
        };
        let value = crate::app_center::metadata::json(&file.bytes)?;
        crate::app_center::metadata::fields(&value, "ampm background_seconds essential")?;
        let policy = Self {
            ampm: value["ampm"].as_bool().ok_or("Invalid clock preference")?,
            background_seconds: value["background_seconds"]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v <= 86400)
                .ok_or("Invalid background timeout")?,
            essential: value["essential"]
                .as_array()
                .filter(|v| v.len() <= 2000)
                .ok_or("Invalid app policy")?
                .iter()
                .map(|v| {
                    let id = v.as_str().ok_or("Invalid app ID")?;
                    if id.is_empty()
                        || id.len() > 256
                        || !id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                    {
                        return Err("Invalid app ID");
                    }
                    Ok(id.to_owned())
                })
                .collect::<Result<_, _>>()?,
        };
        Ok(policy)
    }
    pub fn save(&self) -> Result<(), String> {
        self.write(&Self::path()?)
    }
    fn write(&self, path: &std::path::Path) -> Result<(), String> {
        use crate::app_center::storage::{self, FileData};
        let bytes = serde_json::to_vec(&serde_json::json!({"ampm": self.ampm, "background_seconds": self.background_seconds, "essential": self.essential})).map_err(|e| e.to_string())?;
        storage::atomic(path, &FileData { bytes, mode: 0o600 })
    }
    pub fn timeout(&self, id: &str) -> Option<std::time::Duration> {
        (self.background_seconds > 0 && !self.essential.contains(id))
            .then(|| std::time::Duration::from_secs(u64::from(self.background_seconds)))
    }
}
#[cfg(test)]
mod policy_tests {
    use super::*;
    #[test]
    fn clock_and_background_choices_survive_restart() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let path = scratch
            .0
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("preferences.json");
        let mut policy = Policy {
            ampm: true,
            background_seconds: 300,
            essential: ["io.vitrallis.notepad".into()].into(),
        };
        policy.write(&path)?;
        assert_eq!(Policy::read(&path)?, policy);
        assert_eq!(policy.timeout("io.vitrallis.notepad"), None);
        assert_eq!(
            policy.timeout("other"),
            Some(std::time::Duration::from_secs(300))
        );
        policy.ampm = false;
        policy.background_seconds = 0;
        policy.write(&path)?;
        assert_eq!(Policy::read(&path)?, policy);
        assert_eq!(policy.timeout("other"), None);
        Ok(())
    }
}
