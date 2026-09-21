//! Device page (screen timeout, calibration, power) and the paged time-zone selector.
use super::{Page, Request, Settings, footer::BACK};
use crate::{input::Action, navigation::Direction, platform::system::Control};
impl Settings {
    pub fn page(&mut self, page: Page) {
        if self.page == Page::Storage && page != Page::Storage {
            self.storage.close();
        }
        if page == Page::Storage {
            self.storage_parent = Page::Home;
            self.storage.enter();
            self.storage_view = super::StorageView::Overview;
            self.storage_start = 0;
        }
        self.page = page;
        self.selected = 0;
        self.confirmation = None;
        self.update_confirmation = None;
        self.clear_pointer();
        self.message.clear();
    }
    /// Device rows: screen timeout, touch calibration, restart and power off.
    /// Power actions keep the existing two-step confirmation.
    pub(super) fn device_input(&mut self, action: Action) -> Option<Request> {
        if matches!(action, Action::Back | Action::System | Action::Page(_)) {
            if let Some((power, time)) = self.confirmation.take() {
                let confirm =
                    self.selected == 1 && time.elapsed() < std::time::Duration::from_secs(15);
                self.selected = 0;
                self.message.clear();
                return confirm.then_some(Request::Control(Control::Power(power)));
            }
            self.back();
            return None;
        }
        if self.confirmation.is_some() {
            match action {
                Action::Move(Direction::Left | Direction::Right | Direction::Down) => {
                    self.selected = 1;
                }
                Action::Move(Direction::Up) => self.selected = 0,
                Action::SelectAndActivate(index) if index < 2 => {
                    self.selected = index;
                    return self.device_input(Action::Activate);
                }
                Action::Activate => {
                    let (power, time) = self.confirmation.take()?;
                    let confirm =
                        self.selected == 1 && time.elapsed() < std::time::Duration::from_secs(15);
                    self.selected = 0;
                    self.message.clear();
                    return confirm.then_some(Request::Control(Control::Power(power)));
                }
                _ => {}
            }
            return None;
        }
        match action {
            Action::Move(Direction::Left | Direction::Right) if self.selected == 0 => {
                return self.timeout(action == Action::Move(Direction::Right));
            }
            Action::Move(direction) => self.move_rows(direction),
            Action::SelectAndActivate(index) if index < 4 => {
                self.selected = index;
                return self.device_input(Action::Activate);
            }
            Action::Activate if !self.pending => match self.selected {
                0 => return self.timeout(true),
                1 if self.status.calibration => return Some(Request::Calibration),
                2 | 3 => {
                    let power = if self.selected == 2 {
                        crate::platform::system::Power::Reboot
                    } else {
                        crate::platform::system::Power::Shutdown
                    };
                    if !self.status.power_controls {
                        self.message = "Power control unavailable on this device".into();
                        return None;
                    }
                    self.confirmation = Some((power, std::time::Instant::now()));
                    self.selected = 0;
                    self.message.clear();
                    self.clear_pointer();
                }
                _ => self.message = "Control unavailable on this device".into(),
            },
            Action::Activate
            | Action::SelectAndActivate(_)
            | Action::Page(_)
            | Action::Back
            | Action::System => {}
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
    /// Bounded, paged time-zone selector reached from Date & Time.
    pub(super) fn zone_input(&mut self, action: Action) -> Option<Request> {
        let count = self.status.timezones.len();
        if count == 0 {
            self.page(Page::DateTime);
            return None;
        }
        match action {
            Action::Back | Action::System => {
                self.page(Page::DateTime);
            }
            Action::SelectAndActivate(index) if index < 5 && self.zone_start + index < count => {
                self.selected = index;
                return self.zone_input(Action::Activate);
            }
            Action::SelectAndActivate(BACK) => self.page(Page::DateTime),
            Action::Activate if !self.pending => {
                let index = self.zone_start + self.selected;
                self.page(Page::DateTime);
                return (index < count).then_some(Request::Control(Control::Timezone(index)));
            }
            Action::Move(direction) => {
                if direction == Direction::Down && self.selected + 1 >= self.visible_zones() {
                    self.selected = BACK;
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
    pub(super) fn visible_zones(&self) -> usize {
        self.status
            .timezones
            .len()
            .saturating_sub(self.zone_start)
            .min(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_and_timezone_work_with_keypad_and_return_to_their_parent() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Device);
        settings.status.screen_timeout = Some(600);
        assert_eq!(
            settings.input(Action::Move(Direction::Left)),
            Some(Request::Control(Control::ScreenTimeout(300)))
        );
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::Home);
        settings.page(Page::DateTime);
        settings.status.timezones = vec!["America/Vancouver".into(), "UTC".into()];
        settings.status.timezone = Some("America/Vancouver".into());
        settings.input(Action::SelectAndActivate(1));
        assert_eq!(settings.page, Page::Timezones);
        settings.input(Action::Move(Direction::Down));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::Control(Control::Timezone(1)))
        );
        assert_eq!(settings.page, Page::DateTime);
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::Home);
        assert!(settings.open);
    }
}
