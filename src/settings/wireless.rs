//! Radio switches use the same commands for keyboard and matched touch activation.
use super::{Page, Request, Settings, footer::BACK};
use crate::{
    input::Action,
    navigation::Direction,
    platform::system::{Control, Radio},
};
impl Settings {
    pub const fn radio_value(&self, index: usize) -> Option<bool> {
        match index {
            0 => self.status.wifi_enabled,
            1 => self.status.bluetooth,
            _ => None,
        }
    }
    fn set_radio(&mut self, index: usize, enabled: bool) -> Option<Request> {
        if self.pending {
            return None;
        }
        let Some(current) = self.radio_value(index) else {
            self.message = "Radio unavailable on this device".into();
            return None;
        };
        if current == enabled {
            return None;
        }
        Some(Request::Control(Control::Radio(
            if index == 0 {
                Radio::Wifi
            } else {
                Radio::Bluetooth
            },
            enabled,
        )))
    }
    pub(super) fn wireless_input(&mut self, action: Action) -> Option<Request> {
        match action {
            Action::Back | Action::System | Action::Page(_) => self.page(Page::General),
            Action::Move(Direction::Up) => self.selected = self.selected.saturating_sub(1),
            Action::Move(Direction::Down) => {
                self.selected = if self.selected >= 2 {
                    BACK
                } else {
                    self.selected + 1
                }
            }
            Action::Move(Direction::Left | Direction::Right) if self.selected < 2 => {
                return self.set_radio(self.selected, action == Action::Move(Direction::Right));
            }
            Action::SelectAndActivate(index) if index < 3 => {
                self.selected = index;
                return self.wireless_input(Action::Activate);
            }
            Action::Activate if !self.pending => {
                if self.selected < 2 {
                    return self.set_radio(
                        self.selected,
                        !self.radio_value(self.selected).unwrap_or(false),
                    );
                }
                if self.selected == 2 && self.network_available {
                    return Some(Request::Network);
                }
                self.message = "Wi-Fi connection manager unavailable".into();
            }
            _ => (),
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{layout::Layout, settings::PanelLayout};
    use sdl2::{event::Event, mouse::MouseButton};
    #[test]
    fn toggles_and_connections_are_reachable_with_keyboard_and_touch() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut settings = Settings::default();
        settings.show();
        settings.input(Action::SelectAndActivate(2));
        assert_eq!(settings.page, Page::Wireless);
        settings.status.wifi_enabled = Some(false);
        settings.status.bluetooth = Some(true);
        settings.network_available = true;
        assert_eq!(
            settings.input(Action::Move(Direction::Right)),
            Some(Request::Control(Control::Radio(Radio::Wifi, true)))
        );
        assert_eq!(settings.input(Action::Move(Direction::Left)), None);
        settings.input(Action::Move(Direction::Down));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::Control(Control::Radio(Radio::Bluetooth, false)))
        );
        settings.pending = true;
        assert_eq!(settings.input(Action::Activate), None);
        settings.pending = false;
        for (index, radio, enabled) in [(0, Radio::Wifi, true), (1, Radio::Bluetooth, false)] {
            let bounds = PanelLayout::rows(&layout, 3)[index];
            let event = |down| {
                if down {
                    Event::MouseButtonDown {
                        timestamp: 0,
                        window_id: 1,
                        which: 0,
                        mouse_btn: MouseButton::Left,
                        clicks: 1,
                        x: bounds.x + 10,
                        y: bounds.y + 10,
                    }
                } else {
                    Event::MouseButtonUp {
                        timestamp: 0,
                        window_id: 1,
                        which: 0,
                        mouse_btn: MouseButton::Left,
                        clicks: 1,
                        x: bounds.x + 10,
                        y: bounds.y + 10,
                    }
                }
            };
            assert_eq!(settings.event(&event(false), &layout), None);
            assert_eq!(settings.event(&event(true), &layout), None);
            assert_eq!(
                settings.event(&event(false), &layout),
                Some(Request::Control(Control::Radio(radio, enabled)))
            );
        }
        settings.input(Action::Move(Direction::Down));
        assert_eq!(settings.input(Action::Activate), Some(Request::Network));
        settings.input(Action::Move(Direction::Down));
        settings.input(Action::Activate);
        assert_eq!(settings.page, Page::General);
        Ok(())
    }
    #[test]
    fn unavailable_radios_never_submit_commands() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Wireless);
        for index in 0..2 {
            assert_eq!(settings.input(Action::SelectAndActivate(index)), None);
            assert_eq!(settings.message, "Radio unavailable on this device");
        }
    }
}
