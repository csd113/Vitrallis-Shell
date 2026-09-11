//! Shell update page uses the existing keypad and matched-release pointer routing.
use super::{Page, Request, Settings};
use crate::{input::Action, navigation::Direction, updater::State};
use std::time::{Duration, Instant};

impl Settings {
    pub(super) fn update_input(&mut self, action: Action) -> Option<Request> {
        if matches!(action, Action::Back | Action::System | Action::Page(_)) {
            if self.update_confirmation.take().is_some() {
                self.selected = 0;
                self.clear_pointer();
            } else {
                self.page(Page::Device);
            }
            return None;
        }
        match action {
            Action::Move(direction) => {
                self.selected =
                    usize::from(matches!(direction, Direction::Right | Direction::Down));
            }
            Action::SelectAndActivate(index) if index < 2 => {
                self.selected = index;
                return self.update_input(Action::Activate);
            }
            Action::Activate if self.selected == 0 && self.update_confirmation.is_none() => {
                self.page(Page::Device);
            }
            Action::Activate if !self.updater.state.busy() => {
                if let Some(time) = self.update_confirmation.take() {
                    let install = self.selected == 1 && time.elapsed() < Duration::from_secs(15);
                    self.selected = 0;
                    self.clear_pointer();
                    return install.then_some(Request::InstallUpdate);
                }
                if matches!(self.updater.state, State::Available(_)) {
                    self.update_confirmation = Some(Instant::now());
                    self.selected = 0; // A second Enter always cancels.
                    self.clear_pointer();
                } else if !matches!(self.updater.state, State::Installed { .. }) {
                    return Some(Request::CheckUpdates);
                }
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
    fn check_is_explicit_and_navigation_does_not_start_a_worker() {
        let mut settings = Settings::default();
        settings.show();
        settings.input(Action::SelectAndActivate(5));
        settings.input(Action::SelectAndActivate(3));
        assert_eq!(settings.page, Page::Updates);
        assert!(matches!(settings.updater.state, State::Idle));
        assert_eq!(settings.input(Action::Activate), None);
        assert_eq!(settings.page, Page::Device);
        settings.page(Page::Updates);
        assert_eq!(
            settings.input(Action::SelectAndActivate(1)),
            Some(Request::CheckUpdates)
        );
        settings.updater.state = State::Checking;
        assert_eq!(settings.input(Action::Activate), None);
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::Device);
    }
    #[test]
    fn install_confirmation_defaults_to_cancel_and_expires() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Updates);
        // Confirmation routing is independent of remote data and cannot install
        // unless the controller still holds an available, validated release.
        settings.update_confirmation = Some(Instant::now());
        assert_eq!(settings.input(Action::Activate), None);
        settings.update_confirmation = Some(Instant::now());
        settings.input(Action::Move(Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::InstallUpdate)
        );
        settings.update_confirmation = Instant::now().checked_sub(Duration::from_secs(16));
        assert!(settings.expire());
        assert!(settings.update_confirmation.is_none());
        settings.update_confirmation = Some(Instant::now());
        settings.lost_focus();
        assert!(settings.update_confirmation.is_none());
        settings.update_confirmation = Some(Instant::now());
        settings.input(Action::Back);
        assert!(settings.update_confirmation.is_none());
        assert_eq!(settings.page, Page::Updates);
    }
}
