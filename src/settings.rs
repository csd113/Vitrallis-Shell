//! UI state for generic system controls. No hardware paths or commands live here.
use crate::{
    input::Action,
    navigation,
    platform::system::{Control, Power, Status},
};
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct Settings {
    pub status: Status,
    pub open: bool,
    pub selected: usize,
    pub confirmation: Option<(Power, Instant)>,
    pub message: String,
    pub pending: bool,
}
impl Settings {
    pub fn cancel(&mut self) {
        self.message.clear();
        self.open = false;
        self.confirmation = None;
        self.selected = 0;
    }
    pub fn expire(&mut self) -> bool {
        if self
            .confirmation
            .is_some_and(|(_, time)| time.elapsed() >= Duration::from_secs(15))
        {
            self.confirmation = None;
            self.selected = 0;
            self.message = "CONFIRMATION EXPIRED".into();
            true
        } else {
            false
        }
    }
    pub fn input(&mut self, action: Action) -> Option<Control> {
        if matches!(action, Action::Back) {
            self.cancel();
            return None;
        }
        if matches!(action, Action::System) {
            if self.open {
                self.cancel();
            } else {
                self.open = true;
                self.selected = 0;
            }
            return None;
        }
        if !self.open || self.pending {
            return None;
        }
        let count = if self.confirmation.is_some() { 2 } else { 6 };
        match action {
            Action::Move(direction) => {
                self.selected = navigation::moved(
                    self.selected,
                    direction,
                    if count == 2 { 2 } else { 3 },
                    count,
                );
            }
            Action::SelectAndActivate(index) if index < count => {
                self.selected = index;
                return self.input(Action::Activate);
            }
            Action::Activate => {
                if let Some((power, time)) = self.confirmation.take() {
                    self.message.clear();
                    if self.selected == 1 && time.elapsed() < Duration::from_secs(15) {
                        self.selected = 0;
                        return Some(Control::Power(power));
                    }
                    self.selected = 0;
                    return None;
                }
                let command = match self.selected {
                    0 | 1 => self
                        .status
                        .brightness
                        .map(|value| Control::Brightness(value.step(self.selected == 1))),
                    3 | 4 => self
                        .status
                        .volume
                        .map(|value| Control::Volume(value.step(self.selected == 4))),
                    2 | 5 if self.status.power_controls => {
                        self.confirmation = Some((
                            if self.selected == 2 {
                                Power::Reboot
                            } else {
                                Power::Shutdown
                            },
                            Instant::now(),
                        ));
                        self.selected = 0;
                        self.message = "CANCEL SELECTED - CHOOSE CONFIRM".into();
                        return None;
                    }
                    _ => None,
                };
                if command.is_none() {
                    self.message = "CONTROL UNAVAILABLE".into();
                }
                return command;
            }
            _ => {}
        }
        None
    }
    pub const fn available(&self, index: usize) -> bool {
        if self.pending {
            return false;
        }
        if self.confirmation.is_some() {
            return index < 2;
        }
        match index {
            0 | 1 => self.status.brightness.is_some(),
            3 | 4 => self.status.volume.is_some(),
            2 | 5 => self.status.power_controls,
            _ => false,
        }
    }
    pub fn labels(&self) -> Vec<String> {
        if let Some((power, _)) = self.confirmation {
            return vec![
                "CANCEL".into(),
                format!(
                    "CONFIRM {}",
                    if power == Power::Reboot {
                        "REBOOT"
                    } else {
                        "SHUTDOWN"
                    }
                ),
            ];
        }
        let percent = |p: Option<crate::platform::system::Percent>| {
            p.map_or_else(|| "--".into(), |p| format!("{}%", p.value()))
        };
        let brightness = percent(self.status.brightness);
        let volume = if self.status.muted == Some(true) {
            "MUTE".into()
        } else {
            percent(self.status.volume)
        };
        vec![
            format!("LIGHT - {brightness}"),
            format!("LIGHT + {brightness}"),
            "REBOOT".into(),
            format!("VOL - {volume}"),
            format!("VOL + {volume}"),
            "SHUTDOWN".into(),
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::Direction;
    #[test]
    fn power_requires_distinct_confirmation_and_cancels_safely() {
        let mut settings = Settings {
            status: Status {
                power_controls: true,
                ..Status::default()
            },
            ..Settings::default()
        };
        settings.input(Action::System);
        assert_eq!(settings.input(Action::SelectAndActivate(5)), None);
        assert!(settings.confirmation.is_some());
        assert_eq!(settings.input(Action::Activate), None); // Cancel is the default.
        settings.input(Action::SelectAndActivate(2));
        settings.input(Action::Move(Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Control::Power(Power::Reboot))
        );
        settings.input(Action::SelectAndActivate(5));
        settings.input(Action::Back);
        assert!(settings.confirmation.is_none());
        assert!(!settings.open);
        assert!(settings.message.is_empty());
        settings.open = true;
        settings.confirmation = Some((
            Power::Shutdown,
            Instant::now()
                .checked_sub(Duration::from_secs(16))
                .unwrap_or_else(Instant::now),
        ));
        assert!(settings.expire());
        assert!(settings.confirmation.is_none());
    }
}
