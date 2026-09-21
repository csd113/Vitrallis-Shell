//! Shared focus and activation targets for the visible keyboard/touch footer.
//!
//! Content entries always use indices below [`CONTENT_LIMIT`]; footer controls
//! use the constants below it so a page can never confuse a row with a button.
use super::{Page, Settings};
use crate::{input::Action, navigation::Direction};

/// Highest content index any Settings page uses (Tor has six controls).
pub(super) const CONTENT_LIMIT: usize = 5;
pub(super) const BACK: usize = 8;
pub(super) const PREVIOUS: usize = 9;
pub(super) const NEXT: usize = 10;
pub(super) const REFRESH: usize = 11;

impl Settings {
    pub const fn footer_controls(&self) -> [Option<(usize, &'static str)>; 3] {
        if self.confirmation.is_some() {
            return [None; 3];
        }
        match self.page {
            Page::Home | Page::About => [None; 3],
            Page::Timezones => [
                Some((PREVIOUS, "< Previous")),
                Some((BACK, "Back")),
                Some((NEXT, "Next >")),
            ],
            Page::Storage => match self.storage_view {
                super::StorageView::Overview => {
                    [Some((BACK, "< Back")), Some((REFRESH, "Refresh")), None]
                }
                super::StorageView::Apps => [
                    Some((BACK, "< Back")),
                    Some((REFRESH, "Previous")),
                    Some((NEXT, "Next >")),
                ],
                _ => [Some((BACK, "< Back")), None, None],
            },
            _ => [Some((BACK, "< Back")), None, None],
        }
    }

    pub(super) fn footer_input(&mut self, action: Action) -> bool {
        let controls = self.footer_controls();
        let index = match action {
            Action::SelectAndActivate(index) => index,
            Action::Activate | Action::Move(_) => self.selected,
            _ => return false,
        };
        let Some(position) = controls
            .iter()
            .position(|control| control.is_some_and(|(target, _)| target == index))
        else {
            return false;
        };
        match action {
            Action::Move(Direction::Left | Direction::Right) => {
                let neighbor = if action == Action::Move(Direction::Left) {
                    controls[..position].iter().rev().flatten().next()
                } else {
                    controls[position + 1..].iter().flatten().next()
                };
                if let Some(&(target, _)) = neighbor {
                    self.selected = target;
                }
            }
            Action::Move(Direction::Up) => {
                self.selected = self.content_rows().saturating_sub(1);
            }
            Action::Move(Direction::Down) => {}
            Action::Activate | Action::SelectAndActivate(_) => match self.page {
                Page::Timezones => match index {
                    PREVIOUS => {
                        self.input(Action::Page(false));
                    }
                    NEXT => {
                        self.input(Action::Page(true));
                    }
                    _ => self.back(),
                },
                Page::Storage if index == REFRESH => {
                    self.storage_input(Action::Activate);
                }
                Page::Storage if index == NEXT => {
                    self.storage_input(Action::Page(true));
                }
                Page::Storage if index == PREVIOUS => {
                    self.storage_input(Action::Page(false));
                }
                _ => self.back(),
            },
            _ => return false,
        }
        true
    }

    /// One level of back navigation, exactly like Escape.
    pub(super) fn back(&mut self) {
        if self.confirmation.take().is_some() || self.update_confirmation.take().is_some() {
            self.selected = 0;
            self.message.clear();
            return;
        }
        self.page(super::parent(self.page));
    }

    /// Content rows used by the shared vertical navigation of one page.
    pub(super) const fn content_rows(&self) -> usize {
        match self.page {
            Page::Home => super::HOME_ROWS,
            Page::Display | Page::DateTime => 2,
            Page::Timezones | Page::About => 5,
            Page::Wireless => super::wireless::WIRELESS_ROWS,
            Page::Tor | Page::TorDetails => 6,
            Page::Applications => super::preferences::APP_ROWS,
            Page::Device => 4,
            Page::Storage | Page::Updates => 0,
        }
    }

    /// Shared vertical focus movement: past the last row focus reaches the
    /// visible footer Back control, and up returns to the last row.
    pub(super) fn move_rows(&mut self, direction: Direction) {
        let rows = self.content_rows();
        match direction {
            Direction::Up => {
                self.selected = if self.selected > CONTENT_LIMIT {
                    rows.saturating_sub(1)
                } else {
                    self.selected.saturating_sub(1)
                };
            }
            Direction::Down => {
                self.selected = if self.selected + 1 < rows {
                    self.selected + 1
                } else {
                    BACK
                };
            }
            Direction::Left | Direction::Right => {
                let controls = self.footer_controls();
                let targets: Vec<_> = controls.into_iter().flatten().map(|(i, _)| i).collect();
                if targets.is_empty() {
                    return;
                }
                if let Some(position) = targets.iter().position(|&i| i == self.selected) {
                    let next = if direction == Direction::Left {
                        position.saturating_sub(1)
                    } else {
                        (position + 1).min(targets.len() - 1)
                    };
                    self.selected = targets[next];
                } else {
                    self.selected = targets[0];
                }
            }
        }
    }
}
