//! Shared System Settings screen, independent of hardware commands and paths.
mod device;
mod footer;
mod geometry;
#[cfg(test)]
mod keyboard_tests;
mod pointer;
mod storage;
mod tor;
mod update;
mod wireless;
pub use geometry::PanelLayout;
pub use storage::StorageView;

use crate::{
    input::Action,
    navigation::Direction,
    platform::system::{Control, Percent, Power, Status},
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Control(Control),
    Network,
    Calibration,
    CheckUpdates,
    InstallUpdate,
    RelaunchUpdate,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    #[default]
    Idle,
    Requested,
    Open,
    CalibrationRequested,
    CalibrationOpen,
    TimezoneOpen,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    #[default]
    General,
    Device,
    Timezones,
    Updates,
    Storage,
    Wireless,
    Tor,
    TorDetails,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum TimezoneState {
    #[default]
    Idle,
    Applying(usize),
    Authentication(usize),
    Reading,
}

#[derive(Debug, Clone, Copy)]
pub enum PowerTransition {
    Requested(Power),
    Submitted(Power),
}
impl PowerTransition {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Requested(Power::Reboot) | Self::Submitted(Power::Reboot) => {
                "Device is rebooting"
            }
            Self::Requested(Power::Shutdown) | Self::Submitted(Power::Shutdown) => {
                "Device is shutting down"
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SystemState {
    #[default]
    Loading,
    Ready,
    Stale,
    Unavailable,
}

#[derive(Debug, Default)]
pub struct Settings {
    pub tor: crate::tor::Snapshot,
    pub tor_control: Option<crate::tor::Control>,
    pub storage: crate::storage::Storage,
    pub storage_view: StorageView,
    pub storage_start: usize,
    pub storage_parent: Page,
    pub power_transition: Option<PowerTransition>,
    pub updater: crate::updater::Updater,
    pub update_confirmation: Option<Instant>,
    pub timezone: TimezoneState,
    pub page: Page,
    pub zone_start: usize,
    pub status: Status,
    pub system_state: SystemState,
    pub open: bool,
    pub selected: usize,
    pub confirmation: Option<(Power, Instant)>,
    pub message: String,
    pub pending: bool,
    pub network_available: bool,
    pub network: NetworkState,
    pub applying: Option<(usize, Percent)>,
    pub queued: [Option<Percent>; 2],
    contact: Option<(pointer::ContactId, usize)>,
    preview: Option<Percent>,
}
impl Settings {
    pub const fn pointer_visual(&self) -> (usize, Option<Percent>) {
        (self.selected, self.preview)
    }
    pub fn show(&mut self) {
        self.cancel();
        self.open = true;
    }
    pub fn cancel(&mut self) {
        self.storage.close();
        self.message.clear();
        self.open = false;
        self.page = Page::General;
        self.confirmation = None;
        self.update_confirmation = None;
        self.selected = 0;
        self.clear_pointer();
    }
    pub fn lost_focus(&mut self) {
        self.clear_pointer();
        let update_confirmation = self.update_confirmation.take().is_some();
        if self.confirmation.take().is_some() || update_confirmation {
            self.selected = 0;
            self.message.clear();
        }
    }
    pub const fn clear_pointer(&mut self) {
        self.contact = None;
        self.preview = None;
    }
    pub fn expire(&mut self) -> bool {
        if self
            .update_confirmation
            .is_some_and(|time| time.elapsed() >= Duration::from_secs(15))
        {
            self.update_confirmation = None;
            self.selected = 0;
            self.clear_pointer();
            return true;
        }
        if self
            .confirmation
            .is_some_and(|(_, time)| time.elapsed() >= Duration::from_secs(15))
        {
            self.confirmation = None;
            self.selected = 0;
            self.clear_pointer();
            self.message = "Confirmation expired".into();
            true
        } else {
            false
        }
    }
    pub fn input(&mut self, action: Action) -> Option<Request> {
        if self.open && self.page == Page::Storage {
            self.storage_input(action);
            return None;
        }
        if self.open && self.footer_input(action) {
            return None;
        }
        if self.open && matches!(self.page, Page::Tor | Page::TorDetails) {
            self.tor_input(action);
            return None;
        }
        if self.open && self.page == Page::Wireless {
            return self.wireless_input(action);
        }
        if self.open && self.page == Page::Updates {
            return self.update_input(action);
        }
        if self.open && self.page != Page::General {
            return self.device_input(action);
        }
        if matches!(action, Action::Back | Action::System) {
            if action == Action::System && !self.open {
                self.show();
            } else {
                self.cancel();
            }
            return None;
        }
        if !self.open {
            return None;
        }
        match action {
            Action::Page(_) if self.confirmation.is_none() => self.page(Page::Device),
            Action::Move(direction) => {
                if self.confirmation.is_some() {
                    self.selected =
                        usize::from(matches!(direction, Direction::Right | Direction::Down));
                } else if self.selected < 2
                    && matches!(direction, Direction::Left | Direction::Right)
                {
                    if let Some(value) = self.value(self.selected) {
                        return self
                            .adjust(self.selected, value.step(direction == Direction::Right));
                    }
                } else {
                    self.selected = match direction {
                        Direction::Up => match self.selected {
                            0 | 1 => 0,
                            _ => 1,
                        },
                        Direction::Down | Direction::Right => (self.selected + 1).min(5),
                        Direction::Left => self.selected.saturating_sub(1).max(2),
                    };
                }
            }
            Action::SelectAndActivate(index)
                if index < if self.confirmation.is_some() { 2 } else { 6 } =>
            {
                self.selected = index;
                return self.input(Action::Activate);
            }
            Action::Activate if !self.pending => {
                if let Some((power, time)) = self.confirmation.take() {
                    let confirm = self.selected == 1 && time.elapsed() < Duration::from_secs(15);
                    self.selected = 0;
                    self.message.clear();
                    return confirm.then_some(Request::Control(Control::Power(power)));
                }
                if self.available(self.selected) {
                    match self.selected {
                        0 | 1 => self.message = "Drag the slider or use left / right".into(),
                        2 => return Some(Request::Network),
                        5 => self.page(Page::Device),
                        3 | 4 => {
                            self.confirmation = Some((
                                if self.selected == 3 {
                                    Power::Reboot
                                } else {
                                    Power::Shutdown
                                },
                                Instant::now(),
                            ));
                            self.selected = 0;
                            self.message.clear();
                            self.clear_pointer();
                        }
                        _ => {}
                    }
                } else {
                    self.message = "Control unavailable on this device".into();
                }
            }
            _ => {}
        }
        None
    }
    pub const fn adjust(&mut self, index: usize, value: Percent) -> Option<Request> {
        if !self.available(index) || self.confirmation.is_some() {
            return None;
        }
        self.selected = index;
        let value = self.normalize(index, value);
        match index {
            0 => Some(Request::Control(Control::Brightness(value))),
            1 => Some(Request::Control(Control::Volume(value))),
            _ => None,
        }
    }
    pub const fn normalize(&self, index: usize, value: Percent) -> Percent {
        let value = value.snapped();
        if index == 0
            && let Some(minimum) = self.status.brightness_minimum
            && value.value() < minimum.value()
        {
            return minimum;
        }
        value
    }
    pub fn value(&self, index: usize) -> Option<Percent> {
        if self.selected == index && self.preview.is_some() {
            return self.preview;
        }
        if index < 2 && self.queued[index].is_some() {
            return self.queued[index];
        }
        if let Some((control, value)) = self.applying
            && control == index
        {
            return Some(value);
        }
        match index {
            0 => self.status.brightness,
            // Mixer readback can quantize a requested step (for example 20% to 21%).
            1 => self.status.volume.map(Percent::snapped),
            _ => None,
        }
    }
    pub const fn available(&self, index: usize) -> bool {
        if self.pending && index >= 2 {
            return false;
        }
        if self.confirmation.is_some() {
            return index < 2;
        }
        match index {
            0 => self.status.brightness.is_some(),
            1 => self.status.volume.is_some(),
            2 => self.network_available,
            3 | 4 => self.status.power_controls,
            5 => true,
            _ => false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn power_requires_distinct_confirmation_and_cancels_safely() {
        let mut settings = Settings {
            status: Status {
                power_controls: true,
                ..Status::default()
            },
            ..Settings::default()
        };
        settings.show();
        assert_eq!(settings.input(Action::SelectAndActivate(4)), None);
        assert!(settings.confirmation.is_some());
        assert_eq!(settings.input(Action::Activate), None);
        settings.input(Action::SelectAndActivate(3));
        settings.input(Action::Move(Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::Control(Control::Power(Power::Reboot)))
        );
        settings.input(Action::SelectAndActivate(4));
        settings.input(Action::Back);
        assert!(settings.confirmation.is_none());
        assert!(!settings.open);
        settings.show();
        settings.confirmation = Some((
            Power::Shutdown,
            Instant::now()
                .checked_sub(Duration::from_secs(16))
                .unwrap_or_else(Instant::now),
        ));
        assert!(settings.expire());
        assert!(settings.confirmation.is_none());
    }
    #[test]
    fn sliders_use_typed_bounded_values_and_network_is_explicit() -> Result<(), String> {
        let mut settings = Settings {
            status: Status {
                brightness: Some(Percent::new(95)?),
                volume: Some(Percent::new(0)?),
                ..Status::default()
            },
            network_available: true,
            ..Settings::default()
        };
        settings.show();
        assert_eq!(
            settings.input(Action::Move(Direction::Right)),
            Some(Request::Control(Control::Brightness(Percent::new(100)?)))
        );
        settings.input(Action::Move(Direction::Down));
        assert_eq!(
            settings.input(Action::Move(Direction::Left)),
            Some(Request::Control(Control::Volume(Percent::new(0)?)))
        );
        assert_eq!(
            settings.input(Action::SelectAndActivate(2)),
            Some(Request::Network)
        );
        settings.pending = true;
        assert_eq!(
            settings.adjust(0, Percent::new(50)?),
            Some(Request::Control(Control::Brightness(Percent::new(50)?)))
        );
        settings.input(Action::Back);
        assert!(!settings.open);
        Ok(())
    }
}
