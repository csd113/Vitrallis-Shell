//! Read-only compatibility preferences from the existing `PocketHome` document.
use crate::config::Paths;
use serde_json::Value;
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
            color: [13, 22, 33],
            wallpaper: None,
            show_clock: true,
            ampm: false,
            show_cursor: true,
        }
    }
}
impl Preferences {
    pub fn parse(root: &Value, paths: &Paths) -> Self {
        let mut prefs = Self {
            show_clock: root["showclock"]
                .as_str()
                .is_none_or(|v| v.is_empty() || v == "yes"),
            ampm: root["timeformat"] == "ampm",
            show_cursor: root["cursor"] != "notvisible",
            ..Self::default()
        };
        if let Some(background) = root["background"].as_str() {
            if background.len() == 6
                && background
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c))
            {
                if let Ok(rgb) = u32::from_str_radix(background, 16) {
                    let bytes = rgb.to_be_bytes();
                    prefs.color = [bytes[1], bytes[2], bytes[3]];
                }
            } else if !background.is_empty() && !background.chars().any(char::is_control) {
                prefs.wallpaper = Some(paths.asset(background));
            }
        }
        prefs
    }
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
    fn reference_preferences_validate_colors_paths_and_types() -> Result<(), String> {
        let paths = Paths::from_config(&crate::config::Config::default())?;
        let prefs = Preferences::parse(
            &serde_json::json!({
                "background": "FF0080", "timeformat": "ampm", "cursor": "notvisible", "showclock": "no"
            }),
            &paths,
        );
        assert_eq!(prefs.color, [255, 0, 128]);
        assert!(prefs.wallpaper.is_none());
        assert!(!prefs.show_clock);
        assert!(prefs.ampm);
        assert!(!prefs.show_cursor);
        let image =
            Preferences::parse(&serde_json::json!({"background": "background.png"}), &paths);
        assert_eq!(image.wallpaper, Some(paths.asset("background.png")));
        for background in [
            serde_json::json!("bad\0path"),
            serde_json::json!(42),
            serde_json::Value::Null,
        ] {
            assert!(
                Preferences::parse(&serde_json::json!({"background": background}), &paths)
                    .wallpaper
                    .is_none()
            );
        }
        Ok(())
    }
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
