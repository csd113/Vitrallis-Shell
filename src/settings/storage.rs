//! Keyboard and touch share storage selection and paging actions.
use super::{
    Settings,
    footer::{BACK, NEXT},
};
use crate::{input::Action, navigation::Direction};
pub const REFRESH: usize = 8;
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StorageView {
    #[default]
    Overview,
    Apps,
    App(usize),
    Categories,
}
impl Settings {
    pub fn poll_storage(&mut self) -> bool {
        let snapshot = self.storage.snapshot;
        let dirty = self
            .storage
            .poll(self.open && self.page == super::Page::Storage);
        if self.storage.snapshot != snapshot {
            self.storage_snapshot_changed();
        }
        dirty
    }
    fn storage_snapshot_changed(&mut self) {
        self.storage_start = 0;
        if let StorageView::App(_) = self.storage_view {
            self.storage_view = StorageView::Apps;
            self.selected = 0;
        }
        if self.storage_view == StorageView::Apps
            && self.selected < BACK
            && self.selected >= self.storage_rows()
        {
            self.selected = BACK;
        }
        // Keep focus on Refresh when results arrive; a held Enter must not open an app.
        self.clear_pointer();
    }
    pub fn storage_rows(&self) -> usize {
        if self.storage_view == StorageView::Apps {
            self.storage.report.as_ref().map_or(0, |report| {
                report.apps.len().saturating_sub(self.storage_start).min(4)
            })
        } else if self.storage_view == StorageView::Overview {
            2
        } else {
            0
        }
    }
    fn storage_view(&mut self, view: StorageView) {
        self.storage_view = view;
        self.selected = if self.storage_rows() == 0 { BACK } else { 0 };
        self.clear_pointer();
    }
    pub(super) fn storage_input(&mut self, action: Action) {
        let rows = self.storage_rows();
        match action {
            Action::Back | Action::System => match self.storage_view {
                StorageView::Overview => self.page(self.storage_parent),
                StorageView::App(_) => self.storage_view(StorageView::Apps),
                _ => self.storage_view(StorageView::Overview),
            },
            Action::SelectAndActivate(index)
                if index < rows || [BACK, NEXT, REFRESH].contains(&index) =>
            {
                self.selected = index;
                self.storage_input(Action::Activate);
            }
            Action::Activate => match self.selected {
                BACK => self.storage_input(Action::Back),
                REFRESH if self.storage_view == StorageView::Overview => self.storage.refresh(),
                REFRESH if self.storage_view == StorageView::Apps => {
                    self.storage_input(Action::Page(false));
                }
                NEXT if self.storage_view == StorageView::Apps => {
                    self.storage_input(Action::Page(true));
                }
                0 if self.storage_view == StorageView::Overview => {
                    self.storage_view(StorageView::Apps);
                }
                1 if self.storage_view == StorageView::Overview => {
                    self.storage_view(StorageView::Categories);
                }
                index if self.storage_view == StorageView::Apps && index < rows => {
                    self.storage_view(StorageView::App(self.storage_start + index));
                }
                _ => (),
            },
            Action::Move(Direction::Down) => {
                self.selected = if self.selected + 1 < rows {
                    self.selected + 1
                } else {
                    BACK
                }
            }
            Action::Move(Direction::Up) => {
                self.selected = if self.selected >= rows {
                    rows.checked_sub(1).unwrap_or(BACK)
                } else {
                    self.selected.saturating_sub(1)
                }
            }
            Action::Move(direction @ (Direction::Left | Direction::Right)) => {
                if self.selected < rows {
                    if self.storage_view == StorageView::Overview {
                        self.selected = if direction == Direction::Left {
                            self.selected.saturating_sub(1)
                        } else {
                            (self.selected + 1).min(rows - 1)
                        };
                    }
                    return;
                }
                let controls = self.footer_controls();
                let targets: Vec<_> = controls.into_iter().flatten().map(|(i, _)| i).collect();
                if let Some(index) = targets.iter().position(|&i| i == self.selected) {
                    let next = if direction == Direction::Left {
                        index.saturating_sub(1)
                    } else {
                        (index + 1).min(targets.len() - 1)
                    };
                    self.selected = targets[next];
                }
            }
            Action::Page(next) if self.storage_view == StorageView::Apps => {
                let count = self
                    .storage
                    .report
                    .as_ref()
                    .map_or(0, |report| report.apps.len());
                self.storage_start = if next {
                    (self.storage_start + 4).min(count.saturating_sub(1) / 4 * 4)
                } else {
                    self.storage_start.saturating_sub(4)
                };
                self.selected = if count == 0 { BACK } else { 0 };
                self.clear_pointer();
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overview_arrow_keys_match_button_positions_and_refresh_keeps_focus() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(super::super::Page::Storage);
        settings.input(Action::Move(Direction::Right));
        assert_eq!(settings.selected, 1);
        settings.input(Action::Activate);
        assert_eq!(settings.storage_view, StorageView::Categories);
        settings.input(Action::Back);
        settings.selected = REFRESH;
        settings.storage_snapshot_changed();
        assert_eq!(settings.selected, REFRESH);
        settings.storage_view = StorageView::App(20);
        settings.storage_snapshot_changed();
        assert_eq!(settings.storage_view, StorageView::Apps);
        assert_eq!(settings.selected, BACK);
    }
}
