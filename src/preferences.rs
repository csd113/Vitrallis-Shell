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
            color: [13, 22, 33],
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
