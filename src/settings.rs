//! Shared System Settings screen, independent of hardware commands and paths.
mod device;
mod footer;
mod geometry;
#[cfg(test)]
mod keyboard_tests;
mod pointer;
pub mod preferences;
mod storage;
mod tor;
mod update;
mod wireless;
pub use geometry::PanelLayout;
pub use storage::StorageView;

use crate::settings::footer::BACK;
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
    RestorePrevious,
}

/// Which destructive update action a confirmation dialog is guarding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateConfirmation {
    Install,
    Restore,
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

/// One level of navigation: the home menu lists categories, each category lists
/// settings. No setting is hidden behind a subcategory or a footer shortcut.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    #[default]
    Home,
    Display,
    DateTime,
    Timezones,
    Wireless,
    Tor,
    TorDetails,
    Applications,
    Storage,
    Device,
    Updates,
    About,
}

/// Large home options, in display order. Two columns by four rows.
pub const HOME_ROWS: usize = 8;
const HOME_COLUMNS: usize = 2;
const HOME: [Page; HOME_ROWS] = [
    Page::Display,
    Page::DateTime,
    Page::Wireless,
    Page::Applications,
    Page::Storage,
    Page::Device,
    Page::Updates,
    Page::About,
];

#[must_use]
pub const fn home_page(index: usize) -> Option<Page> {
    if index < HOME_ROWS {
        Some(HOME[index])
    } else {
        None
    }
}

/// Parent page for the shared back behaviour: dismiss a modal first, otherwise
/// return exactly one level, and only leave Settings from the home menu.
#[must_use]
pub const fn parent(page: Page) -> Page {
    match page {
        // Only the home menu leaves Settings; the time zone list and Tor have
        // their own one-level parents.
        Page::Timezones => Page::DateTime,
        Page::TorDetails => Page::Tor,
        Page::Tor => Page::Wireless,
        _ => Page::Home,
    }
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
    pub policy: crate::preferences::Policy,
    pub policy_apps: Vec<(String, String)>,
    pub policy_app: usize,
    pub tor: crate::tor::Snapshot,
    pub tor_control: Option<crate::tor::Control>,
    pub storage: crate::storage::Storage,
    pub storage_view: StorageView,
    pub storage_start: usize,
    /// Where Storage returns to; always one of the Settings pages.
    pub storage_parent: Page,
    pub power_transition: Option<PowerTransition>,
    pub updater: crate::updater::Updater,
    pub update_confirmation: Option<(UpdateConfirmation, Instant)>,
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
    /// Active renderer description, shown read-only on the About page.
    pub renderer: String,
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
        self.page = Page::Home;
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
            .is_some_and(|(_, time)| time.elapsed() >= Duration::from_secs(15))
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
        if !self.open {
            // The Power key or the Settings tile reopens the panel.
            if action == Action::System {
                self.show();
            }
            return None;
        }
        if self.page == Page::Storage {
            self.storage_input(action);
            return None;
        }
        if self.footer_input(action) {
            return None;
        }
        match self.page {
            Page::Home => self.home_input(action),
            Page::Display => self.display_input(action),
            Page::DateTime => self.datetime_input(action),
            Page::Timezones => self.zone_input(action),
            Page::Wireless => self.wireless_input(action),
            Page::Tor | Page::TorDetails => {
                self.tor_input(action);
                None
            }
            Page::Applications => {
                self.preferences_input(action);
                None
            }
            Page::Updates => self.update_input(action),
            Page::Device => self.device_input(action),
            Page::About => self.about_input(action),
            Page::Storage => None,
        }
    }
    /// Home menu: two columns of large options. Escape closes Settings here.
    fn home_input(&mut self, action: Action) -> Option<Request> {
        if matches!(action, Action::Back | Action::System) {
            self.cancel();
            return None;
        }
        match action {
            Action::Move(direction) => {
                self.selected = grid_move(self.selected, direction, HOME_ROWS, HOME_COLUMNS);
            }
            Action::SelectAndActivate(index) if index < HOME_ROWS => {
                self.selected = index;
                return self.home_input(Action::Activate);
            }
            Action::Activate | Action::SelectAndActivate(_) => {
                if let Some(page) = home_page(self.selected) {
                    self.page(page);
                }
            }
            Action::Page(_) | Action::Back | Action::System => {}
        }
        None
    }
    /// Read-only system information: only back navigation applies.
    fn about_input(&mut self, action: Action) -> Option<Request> {
        if matches!(action, Action::Back | Action::System | Action::Page(_)) {
            self.page(Page::Home);
        }
        None
    }
    /// Brightness and volume share the existing slider interaction.
    fn display_input(&mut self, action: Action) -> Option<Request> {
        match action {
            Action::Back | Action::System | Action::Page(_) => self.page(Page::Home),
            Action::Move(direction @ (Direction::Left | Direction::Right)) => {
                if self.selected < 2
                    && let Some(value) = self.value(self.selected)
                {
                    return self.adjust(self.selected, value.step(direction == Direction::Right));
                }
            }
            Action::Move(Direction::Up) => self.selected = self.selected.saturating_sub(1),
            Action::Move(Direction::Down) => {
                self.selected = if self.selected >= 1 { BACK } else { 1 };
            }
            Action::SelectAndActivate(index) if index < 2 => {
                self.selected = index;
                return self.display_input(Action::Activate);
            }
            Action::Activate => {
                if self.available(self.selected) {
                    self.message = "Drag the slider or use left / right".into();
                } else {
                    self.message = "Control unavailable on this device".into();
                }
            }
            Action::SelectAndActivate(_) => {}
        }
        None
    }
    /// Date, clock format and the time-zone selector.
    fn datetime_input(&mut self, action: Action) -> Option<Request> {
        if matches!(action, Action::Back | Action::System | Action::Page(_)) {
            self.page(Page::Home);
            return None;
        }
        match action {
            Action::Move(Direction::Up) => self.selected = self.selected.saturating_sub(1),
            Action::Move(Direction::Down) => {
                self.selected = if self.selected >= 1 { BACK } else { 1 };
            }
            Action::Move(Direction::Left | Direction::Right) | Action::Activate
                if self.selected == 0 =>
            {
                self.toggle_clock();
            }
            Action::SelectAndActivate(index) if index < 2 => {
                self.selected = index;
                return self.datetime_input(Action::Activate);
            }
            Action::Activate if self.selected == 1 && !self.status.timezones.is_empty() => {
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
            Action::Activate => self.message = "Time zone unavailable on this device".into(),
            Action::Move(_)
            | Action::SelectAndActivate(_)
            | Action::Back
            | Action::System
            | Action::Page(_) => {}
        }
        None
    }
    /// Clock format is a persisted preference, not a hardware command.
    pub(super) fn toggle_clock(&mut self) {
        let mut policy = self.policy.clone();
        policy.ampm = !policy.ampm;
        match policy.save() {
            Ok(()) => {
                self.policy = policy;
                self.message.clear();
            }
            Err(error) => self.message = error,
        }
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
            _ => false,
        }
    }

    /// Large home options with their current, real values. The first line names
    /// the category; the second shows the setting a user is most likely to want.
    pub fn home_rows(&self) -> [(String, String); HOME_ROWS] {
        let percent = |value: Option<Percent>| {
            value.map_or_else(|| "--".into(), |value| format!("{}%", value.value()))
        };
        [
            (
                "Display & Sound".into(),
                format!(
                    "Brightness {}   Volume {}",
                    percent(self.status.brightness),
                    percent(self.status.volume)
                ),
            ),
            (
                "Date & Time".into(),
                format!(
                    "{}   {}",
                    self.status.clock.clone().unwrap_or_else(|| "--:--".into()),
                    self.status
                        .timezone
                        .clone()
                        .unwrap_or_else(|| "Time zone unavailable".into())
                ),
            ),
            (
                "Wireless Network".into(),
                format!(
                    "Wi-Fi {}   Tor {}",
                    wifi_word(&self.status),
                    tor_word(self.tor.state)
                ),
            ),
            (
                "Applications".into(),
                if self.policy.background_seconds == 0 {
                    "Background apps stay open".into()
                } else {
                    format!(
                        "Close in background: {}",
                        minutes(self.policy.background_seconds)
                    )
                },
            ),
            ("Storage".into(), self.storage_summary()),
            (
                "Device".into(),
                format!(
                    "Screen timeout: {}",
                    match self.status.screen_timeout {
                        Some(0) => "Never".to_owned(),
                        Some(seconds) => format!("{} min", (seconds / 60).max(1)),
                        None => "Unavailable".to_owned(),
                    }
                ),
            ),
            ("Software Updates".into(), self.update_summary()),
            (
                "About".into(),
                if self.renderer.is_empty() {
                    format!("Version {}", crate::updater::settings_display_version())
                } else {
                    format!(
                        "Version {}   {}",
                        crate::updater::settings_display_version(),
                        self.renderer
                    )
                },
            ),
        ]
    }
    fn storage_summary(&self) -> String {
        self.storage.report.as_ref().map_or_else(
            || "Tap to scan app storage".into(),
            |report| {
                format!(
                    "{} apps   {} used on /",
                    report.apps.len(),
                    crate::storage::format_bytes(report.root_bytes)
                )
            },
        )
    }
    fn update_summary(&self) -> String {
        use crate::updater::State;
        match &self.updater.state {
            State::Idle => format!("Version {}", crate::updater::settings_display_version()),
            State::Checking => "Checking for updates...".into(),
            State::Current => "Up to date".into(),
            State::Available(release) => format!("Version {} available", release.version),
            State::Downloading { received, total } => format!(
                "Downloading {}%",
                received
                    .saturating_mul(100)
                    .checked_div(*total)
                    .unwrap_or(0)
            ),
            State::Installing => "Installing update...".into(),
            State::Installed { version, .. } => format!("Version {version} installed"),
            State::Restoring => "Restoring previous build...".into(),
            State::Restored {
                version: Some(version),
                ..
            } => {
                format!("Restored version {version}")
            }
            State::Restored { version: None, .. } => "Previous build restored".into(),
            State::Failed(_) => "Last update failed - open for details".into(),
        }
    }
}

/// Short Tor wording that fits a home option label.
const fn tor_word(state: crate::tor::State) -> &'static str {
    use crate::tor::State;
    match state {
        State::Disabled => "off",
        State::Connected => "connected",
        State::Bootstrapping => "starting",
        State::Error => "error",
        _ => "idle",
    }
}

fn minutes(seconds: u32) -> String {
    match seconds {
        0 => "never".into(),
        seconds if seconds % 3600 == 0 => format!("{}h", seconds / 3600),
        seconds if seconds % 60 == 0 => format!("{} min", seconds / 60),
        seconds => format!("{seconds} sec"),
    }
}

const fn wifi_word(status: &Status) -> &'static str {
    match status.wifi {
        Some(crate::platform::system::Wifi::Connected) => "connected",
        Some(crate::platform::system::Wifi::Connecting) => "connecting",
        Some(crate::platform::system::Wifi::Off) => "off",
        Some(crate::platform::system::Wifi::Disconnected) => "not connected",
        None => "unavailable",
    }
}

/// Move focus inside a `rows` by `columns` grid, clamped to its bounds.
fn grid_move(index: usize, direction: Direction, rows: usize, columns: usize) -> usize {
    if rows == 0 || columns == 0 {
        return 0;
    }
    let last = rows - 1;
    match direction {
        Direction::Left => index.saturating_sub(1),
        Direction::Right => (index + 1).min(last),
        Direction::Up => index.saturating_sub(columns),
        Direction::Down => (index + columns).min(last),
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
        settings.page(Page::Device);
        assert_eq!(settings.input(Action::SelectAndActivate(3)), None);
        assert!(settings.confirmation.is_some());
        assert_eq!(settings.input(Action::Activate), None);
        settings.input(Action::SelectAndActivate(2));
        settings.input(Action::Move(Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::Control(Control::Power(Power::Reboot)))
        );
        settings.input(Action::SelectAndActivate(3));
        settings.input(Action::Back);
        assert!(settings.confirmation.is_none());
        assert!(settings.open);
        assert_eq!(settings.page, Page::Device);
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
        settings.page(Page::Display);
        assert_eq!(
            settings.input(Action::Move(Direction::Right)),
            Some(Request::Control(Control::Brightness(Percent::new(100)?)))
        );
        settings.input(Action::Move(Direction::Down));
        assert_eq!(
            settings.input(Action::Move(Direction::Left)),
            Some(Request::Control(Control::Volume(Percent::new(0)?)))
        );
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::Home);
        assert_eq!(settings.input(Action::SelectAndActivate(2)), None);
        assert_eq!(settings.page, Page::Wireless);
        assert_eq!(
            settings.input(Action::SelectAndActivate(2)),
            Some(Request::Network)
        );
        settings.input(Action::Back);
        settings.pending = true;
        assert_eq!(
            settings.adjust(0, Percent::new(50)?),
            Some(Request::Control(Control::Brightness(Percent::new(50)?)))
        );
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::Home);
        settings.input(Action::Back);
        assert!(!settings.open);
        Ok(())
    }
    #[test]
    fn home_menu_pages_are_all_reachable_and_back_returns_one_level() -> Result<(), String> {
        for index in 0..HOME_ROWS {
            let mut settings = Settings::default();
            settings.show();
            assert_eq!(settings.input(Action::SelectAndActivate(index)), None);
            let page = home_page(index).ok_or_else(|| format!("home option {index}"))?;
            assert_eq!(settings.page, page, "home option {index}");
            settings.input(Action::Back);
            assert_eq!(settings.page, parent(page));
            if page != Page::Home {
                // A single action returns to the home menu, never out of Settings.
                assert!(settings.open);
            }
        }
        // Only the home menu closes Settings.
        let mut settings = Settings::default();
        settings.show();
        settings.input(Action::Back);
        assert!(!settings.open);
        Ok(())
    }
    #[test]
    fn grid_navigation_stays_inside_the_home_menu() -> Result<(), String> {
        assert_eq!(grid_move(0, Direction::Left, HOME_ROWS, HOME_COLUMNS), 0);
        assert_eq!(grid_move(0, Direction::Up, HOME_ROWS, HOME_COLUMNS), 0);
        assert_eq!(grid_move(0, Direction::Right, HOME_ROWS, HOME_COLUMNS), 1);
        assert_eq!(grid_move(1, Direction::Down, HOME_ROWS, HOME_COLUMNS), 3);
        assert_eq!(grid_move(7, Direction::Right, HOME_ROWS, HOME_COLUMNS), 7);
        assert_eq!(grid_move(7, Direction::Down, HOME_ROWS, HOME_COLUMNS), 7);
        assert_eq!(grid_move(3, Direction::Up, HOME_ROWS, HOME_COLUMNS), 1);
        for index in 0..HOME_ROWS {
            let page = home_page(index).ok_or_else(|| format!("home option {index}"))?;
            assert_ne!(page, Page::Home);
            assert_eq!(parent(page), Page::Home);
        }
        assert!(home_page(HOME_ROWS).is_none());
        Ok(())
    }
    #[test]
    fn clock_format_toggles_and_persists_through_the_policy() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::DateTime);
        let before = settings.policy.ampm;
        settings.input(Action::Activate);
        // The default policy path is unavailable under a test HOME, so the page
        // reports the failure instead of silently discarding the change.
        if settings.policy.ampm == before {
            assert!(!settings.message.is_empty());
        }
    }
}
