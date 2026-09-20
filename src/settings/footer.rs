//! Shared focus and activation targets for the visible keyboard/touch footer.
use super::{Page, Settings};
use crate::{input::Action, navigation::Direction};

pub(super) const NEXT: usize = 5;
pub(super) const BACK: usize = 6;
pub(super) const ZONE_BACK: usize = 7;

impl Settings {
    pub const fn footer_controls(&self) -> [Option<(usize, &'static str)>; 3] {
        if self.confirmation.is_some() {
            return [None; 3];
        }
        match self.page {
            Page::General => [
                Some((BACK, "< Back")),
                Some((super::storage::REFRESH, "Storage")),
                Some((NEXT, "Device >")),
            ],
            Page::Device => [
                Some((BACK, "< Back")),
                Some((super::wireless::WIRELESS, "Wireless")),
                Some((NEXT, "Storage >")),
            ],
            Page::Wireless | Page::Updates => [Some((BACK, "< Back")), None, None],
            Page::Storage => match self.storage_view {
                super::StorageView::Overview => [
                    Some((BACK, "< Back")),
                    Some((super::storage::REFRESH, "Refresh")),
                    None,
                ],
                super::StorageView::Apps => [
                    Some((BACK, "< Back")),
                    Some((super::storage::REFRESH, "Previous")),
                    Some((NEXT, "Next >")),
                ],
                _ => [Some((BACK, "< Back")), None, None],
            },
            Page::Timezones => [
                Some((BACK, "< Previous")),
                Some((ZONE_BACK, "Back")),
                Some((NEXT, "Next >")),
            ],
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
                self.selected = match self.page {
                    Page::General if index == NEXT => 4,
                    Page::General | Page::Wireless => 2,
                    Page::Device => 3,
                    Page::Storage | Page::Updates => 0,
                    Page::Timezones => self.visible_zones().saturating_sub(1),
                };
            }
            Action::Move(Direction::Down) => {}
            Action::Activate | Action::SelectAndActivate(_) => {
                if self.page == Page::Device && index == super::wireless::WIRELESS {
                    self.page(Page::Wireless);
                    return true;
                }
                if (self.page == Page::Device && index == NEXT)
                    || (self.page == Page::General && index == super::storage::REFRESH)
                {
                    self.page(Page::Storage);
                    return true;
                }
                let action = match (self.page, index) {
                    (Page::General | Page::Timezones, NEXT) => Action::Page(true),
                    (Page::Timezones, BACK) => Action::Page(false),
                    _ => Action::Back,
                };
                self.input(action);
            }
            _ => return false,
        }
        true
    }

    pub(super) fn visible_zones(&self) -> usize {
        self.status
            .timezones
            .len()
            .saturating_sub(self.zone_start)
            .min(5)
    }
}
