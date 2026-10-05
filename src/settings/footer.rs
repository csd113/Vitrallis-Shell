//! Shared focus and activation targets for the visible keyboard/touch footer.
//!
//! Content entries always use indices below [`CONTENT_LIMIT`]; footer controls
//! use the constants below it so a page can never confuse a row with a button.
use super::{Page, Settings, UpdateConfirmation};
use crate::{input::Action, navigation::Direction};

/// Highest content index any Settings page uses (Tor has six controls).
pub(super) const CONTENT_LIMIT: usize = 5;
pub(super) const BACK: usize = 8;
pub(super) const PREVIOUS: usize = 9;
pub(super) const NEXT: usize = 10;
pub(super) const REFRESH: usize = 11;
pub(super) const RESTORE: usize = 12;
pub(super) const RESTART: usize = 13;
pub(super) const SHUTDOWN: usize = 14;

impl Settings {
    pub const fn footer_controls(&self) -> [Option<(usize, &'static str)>; 3] {
        if self.confirmation.is_some() {
            return [None; 3];
        }
        match self.page {
            Page::Home => [
                Some((BACK, "Close")),
                Some((RESTART, "Restart")),
                Some((SHUTDOWN, "Power off")),
            ],
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
                super::StorageView::App(_) | super::StorageView::Categories => {
                    [Some((BACK, "< Back")), None, None]
                }
            },
            // The restore control is offered only while a validated previous
            // generation exists and no update action is already pending.
            Page::Updates
                if self.updater.restore_available
                    && self.update_confirmation.is_none()
                    && matches!(
                        self.updater.state,
                        crate::updater::State::Idle
                            | crate::updater::State::Current
                            | crate::updater::State::Available(_)
                            | crate::updater::State::Failed(_)
                    ) =>
            {
                [Some((BACK, "< Back")), Some((RESTORE, "Restore")), None]
            }
            Page::Display
            | Page::DateTime
            | Page::Wireless
            | Page::Tor
            | Page::TorDetails
            | Page::Applications
            | Page::Device
            | Page::Updates
            | Page::About => [Some((BACK, "< Back")), None, None],
        }
    }

    pub(super) fn footer_input(&mut self, action: Action) -> bool {
        let controls = self.footer_controls();
        let index = match action {
            Action::SelectAndActivate(index) => index,
            Action::Activate | Action::Move(_) => self.selected,
            Action::System | Action::Back | Action::Page(_) => return false,
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
                    controls.iter().take(position).rev().flatten().next()
                } else {
                    controls
                        .iter()
                        .skip(position.saturating_add(1))
                        .flatten()
                        .next()
                };
                if let Some(&(target, _)) = neighbor {
                    self.selected = target;
                }
            }
            Action::Move(Direction::Up) => {
                self.selected = self.content_rows().checked_sub(1).unwrap_or(BACK);
            }
            Action::Move(Direction::Down) => {}
            Action::Activate | Action::SelectAndActivate(_) => match self.page {
                Page::Timezones => match index {
                    PREVIOUS => {
                        // Paging Timezones only changes its selection, without a hardware request.
                        let _page_navigation = self.input(Action::Page(false));
                    }
                    NEXT => {
                        let _page_navigation = self.input(Action::Page(true));
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
                Page::Updates if index == RESTORE => {
                    self.update_confirmation =
                        Some((UpdateConfirmation::Restore, std::time::Instant::now()));
                    self.selected = 0; // Cancel is always the default.
                    self.clear_pointer();
                }
                Page::Home
                | Page::Display
                | Page::DateTime
                | Page::Wireless
                | Page::Tor
                | Page::TorDetails
                | Page::Applications
                | Page::Storage
                | Page::Device
                | Page::Updates
                | Page::About => self.back(),
            },
            Action::System | Action::Back | Action::Page(_) => return false,
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
    pub(super) fn content_rows(&self) -> usize {
        match self.page {
            Page::Home => super::HOME_ROWS,
            Page::Display | Page::DateTime | Page::Device | Page::Updates => 2,
            Page::Timezones => self.visible_zones(),
            Page::Wireless => super::wireless::WIRELESS_ROWS,
            Page::Tor => 6,
            Page::Applications => super::preferences::APP_ROWS,
            Page::About | Page::TorDetails | Page::Storage => 0,
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
                self.selected = if self.selected.saturating_add(1) < rows {
                    self.selected.saturating_add(1)
                } else {
                    BACK
                };
            }
            Direction::Left | Direction::Right => {
                let controls = self.footer_controls();
                if let Some(target) = self
                    .footer_neighbor(direction)
                    .or_else(|| controls.iter().flatten().next().map(|(target, _)| *target))
                {
                    self.selected = target;
                }
            }
        }
    }
    pub(super) fn footer_neighbor(&self, direction: Direction) -> Option<usize> {
        let controls = self.footer_controls();
        let position = controls
            .iter()
            .position(|control| control.is_some_and(|(index, _)| index == self.selected))?;
        let neighbor = if direction == Direction::Left {
            controls.iter().take(position).rev().flatten().next()
        } else {
            controls
                .iter()
                .skip(position.saturating_add(1))
                .flatten()
                .next()
        };
        neighbor.map(|(target, _)| *target)
    }
}
