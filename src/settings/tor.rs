//! Keyboard and touch share the exact same six Tor actions.
use super::{Page, Settings, footer::BACK};
use crate::{
    input::Action,
    navigation::Direction,
    tor::{Control, Mode},
};
pub(super) const TOR: usize = 10;
impl Settings {
    pub(super) fn tor_input(&mut self, action: Action) {
        if matches!(action, Action::Back | Action::System) {
            self.page(if self.page == Page::TorDetails {
                Page::Tor
            } else {
                Page::Wireless
            });
            return;
        }
        if self.page == Page::TorDetails {
            self.selected = BACK;
            return;
        }
        match action {
            Action::Move(direction) => {
                self.selected = match direction {
                    Direction::Up => self.selected.saturating_sub(3),
                    Direction::Down => (self.selected + 3).min(BACK),
                    Direction::Left => self.selected.saturating_sub(1),
                    Direction::Right => (self.selected + 1).min(BACK),
                };
            }
            Action::SelectAndActivate(index) if index < 6 => {
                self.selected = index;
                self.tor_input(Action::Activate);
            }
            Action::Activate => {
                self.tor_control = match self.selected {
                    0 => Some(Control::Start),
                    1 => Some(Control::Stop),
                    2 => Some(Control::Restart),
                    3 => Some(Control::Mode(if self.tor.mode == Mode::Disabled {
                        Mode::OnDemand
                    } else {
                        Mode::Disabled
                    })),
                    4 => Some(Control::Mode(self.tor.mode.next())),
                    5 => {
                        self.page(Page::TorDetails);
                        self.selected = BACK;
                        None
                    }
                    _ => None,
                };
            }
            _ => (),
        }
    }
}
