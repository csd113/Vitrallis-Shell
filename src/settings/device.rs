//! Additional settings and a bounded, paged system time-zone selector.
use super::{
    Page, Request, Settings,
    footer::{BACK, ZONE_BACK},
};
use crate::{input::Action, navigation::Direction, platform::system::Control};
impl Settings {
    pub fn page(&mut self, page: Page) {
        self.page = page;
        self.selected = 0;
        self.confirmation = None;
        self.update_confirmation = None;
        self.clear_pointer();
        self.message.clear();
    }
    pub(super) fn device_input(&mut self, action: Action) -> Option<Request> {
        if action == Action::Back || action == Action::System {
            self.page(if self.page == Page::Timezones {
                Page::Device
            } else {
                Page::General
            });
            return None;
        }
        if self.page == Page::Timezones {
            return self.zone_input(action);
        }
        match action {
            Action::Move(Direction::Up) => self.selected = self.selected.saturating_sub(1),
            Action::Move(Direction::Down) => {
                self.selected = if self.selected >= 3 {
                    BACK
                } else {
                    self.selected + 1
                };
            }
            Action::Move(Direction::Left | Direction::Right) if self.selected == 0 => {
                return self.timeout(action == Action::Move(Direction::Right));
            }
            Action::SelectAndActivate(index) if index < 4 => {
                self.selected = index;
                return self.device_input(Action::Activate);
            }
            Action::Activate if !self.pending => match self.selected {
                0 => return self.timeout(true),
                1 if !self.status.timezones.is_empty() => {
                    let index = self
                        .status
                        .timezones
                        .iter()
                        .position(|zone| Some(zone) == self.status.timezone.as_ref())
                        .unwrap_or(0);
                    self.page(Page::Timezones);
                    self.zone_start = index / 5 * 5;
                    self.selected = index % 5;
                }
                3 => self.page(Page::Updates),
                2 if self.status.calibration => return Some(Request::Calibration),
                _ => self.message = "Control unavailable on this device".into(),
            },
            Action::Page(_) => self.page(Page::General),
            _ => {}
        }
        None
    }
    pub(super) fn timeout(&self, up: bool) -> Option<Request> {
        if self.pending {
            return None;
        }
        let current = self.status.screen_timeout?;
        let choices = crate::platform::system::SCREEN_TIMEOUTS;
        let value = if up {
            choices
                .into_iter()
                .find(|&value| value > current)
                .unwrap_or(0)
        } else {
            choices
                .into_iter()
                .rev()
                .find(|&value| value < current)
                .unwrap_or(1800)
        };
        Some(Request::Control(Control::ScreenTimeout(value)))
    }
    fn zone_input(&mut self, action: Action) -> Option<Request> {
        let count = self.status.timezones.len();
        if count == 0 {
            self.page(Page::Device);
            return None;
        }
        match action {
            Action::SelectAndActivate(index) if index < 5 && self.zone_start + index < count => {
                self.selected = index;
                return self.zone_input(Action::Activate);
            }
            Action::Activate if !self.pending => {
                let index = self.zone_start + self.selected;
                self.page(Page::Device);
                return (index < count).then_some(Request::Control(Control::Timezone(index)));
            }
            Action::Move(direction) => {
                if direction == Direction::Down && self.selected + 1 >= self.visible_zones() {
                    self.selected = ZONE_BACK;
                    return None;
                }
                let current = self.zone_start + self.selected;
                let index = match direction {
                    Direction::Up => current.saturating_sub(1),
                    Direction::Down => (current + 1).min(count - 1),
                    Direction::Left => current.saturating_sub(5),
                    Direction::Right => (current + 5).min(count - 1),
                };
                self.zone_start = index / 5 * 5;
                self.selected = index % 5;
            }
            Action::Page(next) => {
                self.zone_start = if next {
                    (self.zone_start + 5).min((count - 1) / 5 * 5)
                } else {
                    self.zone_start.saturating_sub(5)
                };
                self.selected = 0;
            }
            _ => {}
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pages_timeout_and_timezone_work_with_keypad() {
        let mut settings = Settings::default();
        settings.show();
        settings.input(Action::SelectAndActivate(5));
        assert_eq!(settings.page, Page::Device);
        settings.status.screen_timeout = Some(600);
        assert_eq!(
            settings.input(Action::Move(Direction::Left)),
            Some(Request::Control(Control::ScreenTimeout(300)))
        );
        settings.status.timezones = vec!["America/Vancouver".into(), "UTC".into()];
        settings.status.timezone = Some("America/Vancouver".into());
        settings.input(Action::SelectAndActivate(1));
        assert_eq!(settings.page, Page::Timezones);
        settings.input(Action::Move(Direction::Down));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::Control(Control::Timezone(1)))
        );
        assert_eq!(settings.page, Page::Device);
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::General);
        assert!(settings.open);
    }
}
