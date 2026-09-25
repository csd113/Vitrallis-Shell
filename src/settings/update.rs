//! Shell update page uses the existing keypad and matched-release pointer routing.
use super::{Page, Request, Settings, UpdateConfirmation, footer::BACK};
use crate::{input::Action, navigation::Direction, updater::State};
use std::time::{Duration, Instant};

impl Settings {
    pub(super) fn update_input(&mut self, action: Action) -> Option<Request> {
        if matches!(action, Action::Back | Action::System | Action::Page(_)) {
            if self.update_confirmation.take().is_some() {
                self.selected = 0;
                self.clear_pointer();
            } else {
                self.page(Page::Home);
            }
            return None;
        }
        match action {
            Action::Move(Direction::Down)
                if self.update_confirmation.is_none() || self.selected == 1 =>
            {
                self.selected = BACK;
            }
            Action::Move(Direction::Left | Direction::Right | Direction::Up) => {
                self.selected = usize::from(matches!(
                    action,
                    Action::Move(Direction::Right | Direction::Up)
                ));
            }
            Action::SelectAndActivate(index) if index < 2 => {
                self.selected = index;
                return self.update_input(Action::Activate);
            }
            Action::Activate if self.selected == 0 && self.update_confirmation.is_none() => {
                self.page(Page::Home);
            }
            Action::Activate if !self.updater.state.busy() => {
                if let Some((confirmation, time)) = self.update_confirmation.take() {
                    // A second activation of the second button confirms; every
                    // other path cancels, including an expired confirmation.
                    let confirmed = self.selected == 1 && time.elapsed() < Duration::from_secs(15);
                    self.selected = 0;
                    self.clear_pointer();
                    return confirmed.then_some(match confirmation {
                        UpdateConfirmation::Install => Request::InstallUpdate,
                        UpdateConfirmation::Restore => Request::RestorePrevious,
                    });
                }
                if matches!(self.updater.state, State::Available(_)) {
                    self.update_confirmation = Some((UpdateConfirmation::Install, Instant::now()));
                    self.selected = 0; // A second Enter always cancels.
                    self.clear_pointer();
                } else if matches!(
                    self.updater.state,
                    State::Installed { .. } | State::Restored { .. }
                ) {
                    return Some(Request::RelaunchUpdate);
                } else {
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
    fn installed_button_requests_relaunch_for_keyboard_and_pointer_actions() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Updates);
        settings.updater.state = State::Installed {
            version: semver::Version::new(1, 2, 3),
            durable: true,
            relaunch: crate::platform::update::Relaunch {
                executable: "/installed/vitrallis".into(),
                sha256: [0; 32],
            },
        };
        settings.input(Action::Move(Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::RelaunchUpdate)
        );
        assert_eq!(
            settings.input(Action::SelectAndActivate(1)),
            Some(Request::RelaunchUpdate)
        );
        assert!(settings.update_confirmation.is_none());
        assert_eq!(settings.input(Action::SelectAndActivate(0)), None);
        assert_eq!(settings.page, Page::Home);
    }
    #[test]
    fn check_is_explicit_and_navigation_does_not_start_a_worker() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Updates);
        assert!(matches!(settings.updater.state, State::Idle));
        assert_eq!(settings.input(Action::Activate), None);
        assert_eq!(settings.page, Page::Home);
        settings.page(Page::Updates);
        assert_eq!(
            settings.input(Action::SelectAndActivate(1)),
            Some(Request::CheckUpdates)
        );
        settings.updater.state = State::Checking;
        assert_eq!(settings.input(Action::Activate), None);
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::Home);
    }
    #[test]
    fn install_confirmation_defaults_to_cancel_and_expires() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Updates);
        // Confirmation routing is independent of remote data and cannot install
        // unless the controller still holds an available, validated release.
        settings.update_confirmation = Some((UpdateConfirmation::Install, Instant::now()));
        assert_eq!(settings.input(Action::Activate), None);
        settings.update_confirmation = Some((UpdateConfirmation::Install, Instant::now()));
        settings.input(Action::Move(Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::InstallUpdate)
        );
        settings.update_confirmation = Instant::now()
            .checked_sub(Duration::from_secs(16))
            .map(|time| (UpdateConfirmation::Install, time));
        assert!(settings.expire());
        assert!(settings.update_confirmation.is_none());
        settings.update_confirmation = Some((UpdateConfirmation::Install, Instant::now()));
        settings.lost_focus();
        assert!(settings.update_confirmation.is_none());
        settings.update_confirmation = Some((UpdateConfirmation::Install, Instant::now()));
        settings.input(Action::Back);
        assert!(settings.update_confirmation.is_none());
        assert_eq!(settings.page, Page::Updates);
    }
    #[test]
    fn restore_control_needs_a_retained_build_and_its_own_confirmation() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Updates);
        // Without a retained build the footer carries no restore control.
        assert!(
            !settings
                .footer_controls()
                .iter()
                .flatten()
                .any(|(index, _)| *index == super::super::footer::RESTORE)
        );
        settings.updater.restore_available = true;
        let restore = settings
            .footer_controls()
            .into_iter()
            .flatten()
            .find(|(index, _)| *index == super::super::footer::RESTORE);
        assert!(restore.is_some_and(|(_, label)| label == "Restore"));
        // Activating the footer control opens the confirmation with Cancel
        // selected; a second activation of Confirm Restore completes it.
        settings.selected = super::super::footer::RESTORE;
        settings.input(Action::Activate);
        assert!(
            settings
                .update_confirmation
                .is_some_and(|(kind, _)| kind == UpdateConfirmation::Restore)
        );
        assert_eq!(settings.selected, 0);
        assert_eq!(settings.input(Action::Activate), None);
        assert!(settings.update_confirmation.is_none());
        settings.selected = super::super::footer::RESTORE;
        settings.input(Action::Activate);
        settings.input(Action::Move(Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::RestorePrevious)
        );
    }
    #[test]
    fn restore_is_hidden_while_an_update_action_is_pending() {
        for state in [
            State::Checking,
            State::Downloading {
                received: 1,
                total: 2,
            },
            State::Installing,
            State::Restoring,
            State::Installed {
                version: semver::Version::new(1, 2, 3),
                durable: true,
                relaunch: crate::platform::update::Relaunch {
                    executable: "/installed/vitrallis".into(),
                    sha256: [0; 32],
                },
            },
        ] {
            let mut settings = Settings::default();
            settings.updater.restore_available = true;
            settings.show();
            settings.page(Page::Updates);
            settings.updater.state = state;
            assert!(
                !settings
                    .footer_controls()
                    .iter()
                    .flatten()
                    .any(|(index, _)| *index == super::super::footer::RESTORE)
            );
        }
    }
}
