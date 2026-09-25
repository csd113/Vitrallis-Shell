use super::{Page, PanelLayout, Request, Settings};
use crate::{input::Action, layout::Layout, platform::system::Percent};
use sdl2::{event::Event, mouse::MouseButton};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContactId {
    Mouse(u32),
    Finger(i64, i64),
}
#[derive(Clone, Copy)]
enum Phase {
    Down,
    Move,
    Up,
}
fn contact(event: &Event, layout: &Layout) -> Option<(ContactId, Phase, f64, f64)> {
    let (id, phase, x, y) = match *event {
        Event::MouseButtonDown {
            which,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if which != u32::MAX => (
            ContactId::Mouse(which),
            Phase::Down,
            f64::from(x),
            f64::from(y),
        ),
        Event::MouseButtonUp {
            which,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if which != u32::MAX => (
            ContactId::Mouse(which),
            Phase::Up,
            f64::from(x),
            f64::from(y),
        ),
        Event::MouseMotion { which, x, y, .. } if which != u32::MAX => (
            ContactId::Mouse(which),
            Phase::Move,
            f64::from(x),
            f64::from(y),
        ),
        Event::FingerDown {
            touch_id,
            finger_id,
            x,
            y,
            ..
        } => (
            ContactId::Finger(touch_id, finger_id),
            Phase::Down,
            f64::from(x) * f64::from(layout.width),
            f64::from(y) * f64::from(layout.height),
        ),
        Event::FingerMotion {
            touch_id,
            finger_id,
            x,
            y,
            ..
        } => (
            ContactId::Finger(touch_id, finger_id),
            Phase::Move,
            f64::from(x) * f64::from(layout.width),
            f64::from(y) * f64::from(layout.height),
        ),
        Event::FingerUp {
            touch_id,
            finger_id,
            x,
            y,
            ..
        } => (
            ContactId::Finger(touch_id, finger_id),
            Phase::Up,
            f64::from(x) * f64::from(layout.width),
            f64::from(y) * f64::from(layout.height),
        ),
        _ => return None,
    };
    (x.is_finite() && y.is_finite()).then_some((id, phase, x, y))
}
impl Settings {
    fn row_count(&self) -> i32 {
        match self.page {
            Page::Timezones | Page::About => 5,
            Page::Wireless => i32::try_from(super::wireless::WIRELESS_ROWS).unwrap_or(4),
            Page::Applications => i32::try_from(super::preferences::APP_ROWS).unwrap_or(3),
            Page::DateTime => 2,
            _ => 4,
        }
    }
    fn pointer_targets<'a>(
        &self,
        geometry: &'a PanelLayout,
        rows: &'a [crate::layout::Rect],
        storage: &'a [crate::layout::Rect],
        tor: &'a [crate::layout::Rect],
        home: &'a [crate::layout::Rect; super::HOME_ROWS],
    ) -> &'a [crate::layout::Rect] {
        if self.confirmation.is_some() {
            return &geometry.confirmation;
        }
        match self.page {
            Page::Home => home,
            Page::About | Page::TorDetails => &[],
            Page::Tor => tor,
            Page::Storage => match self.storage_view {
                super::StorageView::Overview => storage,
                super::StorageView::Apps => &rows[..self.storage_rows()],
                _ => &[],
            },
            Page::Updates => &geometry.confirmation,
            Page::Display => &geometry.controls[..2],
            _ => rows,
        }
    }
    pub fn event(&mut self, event: &Event, layout: &Layout) -> Option<Request> {
        if !self.open {
            return None;
        }
        if let Event::KeyDown { .. } = event {
            self.clear_pointer();
            return crate::input::action(event, layout, 0).and_then(|action| self.input(action));
        }
        let (id, phase, x, y) = contact(event, layout)?;
        let geometry = PanelLayout::new(layout);
        let rows = PanelLayout::rows(layout, self.row_count());
        let storage_actions = PanelLayout::storage_actions(layout);
        let tor_controls = PanelLayout::tor_controls(layout);
        let home = PanelLayout::home(layout);
        let targets =
            self.pointer_targets(&geometry, &rows, &storage_actions, &tor_controls, &home);
        let hit = targets.iter().position(|r| r.contains(x, y)).or_else(|| {
            PanelLayout::footer(layout)
                .into_iter()
                .zip(self.footer_controls())
                .find_map(|(bounds, control)| {
                    control
                        .filter(|_| bounds.contains(x, y))
                        .map(|(index, _)| index)
                })
        });
        match phase {
            Phase::Down => {
                if self.contact.is_some() {
                    self.clear_pointer();
                    return None;
                }
                self.contact = hit.map(|index| (id, index));
                if let Some(index) = hit.filter(|&i| {
                    self.page == Page::Display
                        && i < 2
                        && self.confirmation.is_none()
                        && self.available(i)
                }) {
                    self.selected = index;
                    self.preview = slider_value(x, geometry.tracks[index])
                        .map(|value| self.normalize(index, value));
                    return self.preview.and_then(|value| self.adjust(index, value));
                }
            }
            Phase::Move | Phase::Up => {
                let (pressed, index) = self.contact?;
                if pressed != id {
                    self.clear_pointer();
                    return None;
                }
                if self.page == Page::Display
                    && index < 2
                    && self.confirmation.is_none()
                    && self.available(index)
                {
                    let track = geometry.tracks[index];
                    let position =
                        ((x - f64::from(track.x)) * 100. / f64::from(track.w)).clamp(0., 100.);
                    // A small dead band stops resistive-touch jitter flipping adjacent steps.
                    let value = if self
                        .preview
                        .is_some_and(|previous| (position - f64::from(previous.value())).abs() < 7.)
                    {
                        self.preview
                    } else {
                        slider_value(x, track).map(|value| self.normalize(index, value))
                    };
                    let changed = value != self.preview;
                    self.preview = value;
                    if matches!(phase, Phase::Up) {
                        self.clear_pointer();
                    }
                    if changed {
                        return value.and_then(|value| self.adjust(index, value));
                    }
                } else if matches!(phase, Phase::Up) {
                    self.clear_pointer();
                    if hit == Some(index) {
                        if self.page == Page::Device
                            && index == 0
                            && x < f64::from(layout.width) / 3.
                        {
                            self.selected = 0;
                            return self.timeout(false);
                        }
                        return self.input(Action::SelectAndActivate(index));
                    }
                }
            }
        }
        None
    }
}
fn slider_value(x: f64, track: crate::layout::Rect) -> Option<Percent> {
    let position = ((x - f64::from(track.x)) * 100. / f64::from(track.w)).clamp(0., 100.);
    // The small closed range avoids lossy float-to-integer casts for touch data.
    (0..=100)
        .step_by(10)
        .find(|&v| position <= f64::from(v) + 5.)
        .and_then(|v| Percent::new(v).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::system::{Control, Status};
    fn mouse(down: bool, x: i32, y: i32) -> Event {
        if down {
            Event::MouseButtonDown {
                timestamp: 0,
                window_id: 1,
                which: 0,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x,
                y,
            }
        } else {
            Event::MouseButtonUp {
                timestamp: 0,
                window_id: 1,
                which: 0,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x,
                y,
            }
        }
    }
    #[test]
    fn relaunch_uses_matched_mouse_and_touch_activation() -> Result<(), String> {
        for (width, height) in [(480, 272), (800, 480)] {
            let layout = Layout::home(width, height)?;
            let button = PanelLayout::new(&layout).confirmation[1];
            let (x, y) = (button.x + 5, button.y + 5);
            let mut settings = Settings::default();
            settings.show();
            settings.page(Page::Updates);
            settings.updater.state = crate::updater::State::Installed {
                version: semver::Version::new(1, 2, 3),
                durable: true,
                relaunch: crate::platform::update::Relaunch {
                    executable: "/installed/vitrallis".into(),
                    sha256: [0; 32],
                },
            };
            assert_eq!(settings.event(&mouse(false, x, y), &layout), None);
            assert_eq!(settings.event(&mouse(true, x, y), &layout), None);
            assert_eq!(
                settings.event(&mouse(false, x, y), &layout),
                Some(Request::RelaunchUpdate)
            );
            let x = f32::from(u16::try_from(x).map_err(|e| e.to_string())?) / f32::from(width);
            let y = f32::from(u16::try_from(y).map_err(|e| e.to_string())?) / f32::from(height);
            let down = Event::FingerDown {
                timestamp: 0,
                touch_id: 1,
                finger_id: 7,
                x,
                y,
                dx: 0.,
                dy: 0.,
                pressure: 1.,
            };
            let up = Event::FingerUp {
                timestamp: 0,
                touch_id: 1,
                finger_id: 7,
                x,
                y,
                dx: 0.,
                dy: 0.,
                pressure: 0.,
            };
            assert_eq!(settings.event(&up, &layout), None);
            assert_eq!(settings.event(&down, &layout), None);
            assert_eq!(settings.event(&up, &layout), Some(Request::RelaunchUpdate));
        }
        Ok(())
    }
    #[test]
    fn home_options_and_time_zone_rows_use_matching_releases() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut settings = Settings::default();
        settings.show();
        settings.status.screen_timeout = Some(60);
        settings.status.timezones = vec!["America/Vancouver".into(), "UTC".into()];
        // Home option 5 is Device.
        let option = PanelLayout::home(&layout)[5];
        for down in [true, false] {
            settings.event(&mouse(down, option.x + 10, option.y + 10), &layout);
        }
        assert_eq!(settings.page, Page::Device);
        assert_eq!(settings.event(&mouse(false, 40, 80), &layout), None);
        settings.event(&mouse(true, 40, 80), &layout);
        assert_eq!(
            settings.event(&mouse(false, 40, 80), &layout),
            Some(Request::Control(Control::ScreenTimeout(30)))
        );
        settings.input(Action::Back);
        // Home option 1 is Date & Time, which lists the time zone.
        let option = PanelLayout::home(&layout)[1];
        for down in [true, false] {
            settings.event(&mouse(down, option.x + 10, option.y + 10), &layout);
        }
        assert_eq!(settings.page, Page::DateTime);
        // The Time zone row opens the bounded zone list, and a zone applies.
        let row = PanelLayout::rows(&layout, 2)[1];
        for down in [true, false] {
            settings.event(&mouse(down, 200, row.y + 5), &layout);
        }
        assert_eq!(settings.page, Page::Timezones);
        let zone = PanelLayout::rows(&layout, 5)[0];
        settings.event(&mouse(true, 200, zone.y + 5), &layout);
        assert_eq!(
            settings.event(&mouse(false, 200, zone.y + 5), &layout),
            Some(Request::Control(Control::Timezone(0)))
        );
        assert_eq!(settings.page, Page::DateTime);
        Ok(())
    }
    #[test]
    fn update_buttons_require_matched_release_and_focus_cancels_touch() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let button = PanelLayout::new(&layout).confirmation[1];
        let (x, y) = (button.x + 5, button.y + 5);
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Updates);
        assert_eq!(settings.event(&mouse(false, x, y), &layout), None);
        settings.event(&mouse(true, x, y), &layout);
        settings.lost_focus();
        assert_eq!(settings.event(&mouse(false, x, y), &layout), None);
        settings.event(&mouse(true, x, y), &layout);
        assert_eq!(
            settings.event(&mouse(false, x, y), &layout),
            Some(Request::CheckUpdates)
        );
        settings.update_confirmation = Some((
            crate::settings::UpdateConfirmation::Install,
            std::time::Instant::now(),
        ));
        settings.event(&mouse(true, x, y), &layout);
        settings.input(Action::Back);
        assert_eq!(settings.event(&mouse(false, x, y), &layout), None);
        Ok(())
    }
    #[test]
    fn restore_footer_opens_confirmation_from_mouse_and_touch() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let footer = PanelLayout::footer(&layout)[1];
        let (x, y) = (footer.x + 5, footer.y + footer.h - 5);
        let mut settings = Settings::default();
        settings.updater.restore_available = true;
        settings.show();
        settings.page(Page::Updates);
        // A matched mouse press and release opens the confirmation with Cancel
        // selected; the action itself is never taken from the footer tap.
        assert_eq!(settings.event(&mouse(true, x, y), &layout), None);
        assert_eq!(settings.event(&mouse(false, x, y), &layout), None);
        assert!(settings.update_confirmation.is_some());
        assert_eq!(settings.selected, 0);
        settings.input(Action::Activate);
        assert!(settings.update_confirmation.is_none());
        // The same control releases for a finger.
        let nx = f32::from(u16::try_from(x).map_err(|e| e.to_string())?) / f32::from(layout.width);
        let ny = f32::from(u16::try_from(y).map_err(|e| e.to_string())?) / f32::from(layout.height);
        for up in [true, false, true] {
            let event = if up {
                Event::FingerUp {
                    timestamp: 0,
                    touch_id: 1,
                    finger_id: 9,
                    x: nx,
                    y: ny,
                    dx: 0.,
                    dy: 0.,
                    pressure: 0.,
                }
            } else {
                Event::FingerDown {
                    timestamp: 0,
                    touch_id: 1,
                    finger_id: 9,
                    x: nx,
                    y: ny,
                    dx: 0.,
                    dy: 0.,
                    pressure: 1.,
                }
            };
            assert_eq!(settings.event(&event, &layout), None);
        }
        assert!(settings.update_confirmation.is_some());
        // Only a second activation of the confirm button completes the action.
        settings.input(Action::Move(crate::navigation::Direction::Right));
        assert_eq!(
            settings.input(Action::Activate),
            Some(Request::RestorePrevious)
        );
        Ok(())
    }
    #[test]
    fn touch_steps_are_tens_and_jitter_does_not_reverse_a_step() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let track = PanelLayout::new(&layout).tracks[0];
        for offset in 0..=track.w {
            let value = slider_value(f64::from(track.x + offset), track).ok_or("slider value")?;
            assert_eq!(value.value() % 10, 0);
        }
        let mut settings = Settings {
            status: Status {
                brightness: Some(Percent::new(50)?),
                brightness_minimum: Some(Percent::new(10)?),
                ..Status::default()
            },
            ..Settings::default()
        };
        settings.show();
        settings.page(Page::Display);
        assert_eq!(
            settings.event(&mouse(true, track.x, track.y), &layout),
            Some(Request::Control(Control::Brightness(Percent::new(10)?)))
        );
        assert_eq!(settings.value(0), Some(Percent::new(10)?));
        settings.clear_pointer();
        settings.event(&mouse(true, track.x + track.w / 2, track.y), &layout);
        let mut motion = Event::MouseMotion {
            timestamp: 0,
            window_id: 1,
            which: 0,
            mousestate: sdl2::mouse::MouseState::from_sdl_state(1),
            x: track.x + track.w * 56 / 100,
            y: track.y,
            xrel: 0,
            yrel: 0,
        };
        assert_eq!(settings.event(&motion, &layout), None);
        if let Event::MouseMotion { x, .. } = &mut motion {
            *x = track.x + track.w * 60 / 100;
        }
        assert_eq!(
            settings.event(&motion, &layout),
            Some(Request::Control(Control::Brightness(Percent::new(60)?)))
        );
        if let Event::MouseMotion { x, .. } = &mut motion {
            *x = track.x + track.w * 54 / 100;
        }
        assert_eq!(settings.event(&motion, &layout), None);
        assert_eq!(settings.value(0), Some(Percent::new(60)?));
        Ok(())
    }
    #[test]
    fn finger_identity_and_synthesized_mouse_do_not_apply_accidental_controls() -> Result<(), String>
    {
        let layout = Layout::home(480, 272)?;
        let mut settings = Settings {
            status: Status {
                brightness: Some(Percent::new(50)?),
                ..Status::default()
            },
            ..Settings::default()
        };
        settings.show();
        settings.page(Page::Display);
        let down = Event::FingerDown {
            timestamp: 0,
            touch_id: 1,
            finger_id: 7,
            x: 0.5,
            y: 0.3,
            dx: 0.,
            dy: 0.,
            pressure: 1.,
        };
        let mut up = Event::FingerUp {
            timestamp: 0,
            touch_id: 1,
            finger_id: 8,
            x: 0.8,
            y: 0.3,
            dx: 0.,
            dy: 0.,
            pressure: 0.,
        };
        assert!(settings.event(&down, &layout).is_some());
        assert_eq!(settings.event(&up, &layout), None);
        assert!(settings.contact.is_none());
        let mut synthetic = mouse(true, 240, 80);
        if let Event::MouseButtonDown { which, .. } = &mut synthetic {
            *which = u32::MAX;
        }
        assert_eq!(settings.event(&synthetic, &layout), None);
        assert!(settings.contact.is_none());
        settings.event(&down, &layout);
        if let Event::FingerUp { finger_id, .. } = &mut up {
            *finger_id = 7;
        }
        assert!(matches!(
            settings.event(&up, &layout),
            Some(Request::Control(Control::Brightness(_)))
        ));
        Ok(())
    }
    #[test]
    fn dragging_applies_live_steps_and_stale_releases_do_nothing() -> Result<(), String> {
        for (w, h) in [(320, 200), (480, 272), (800, 480), (1280, 720)] {
            let layout = Layout::home(w, h)?;
            let geometry = PanelLayout::new(&layout);
            let track = geometry.tracks[0];
            let mut settings = Settings {
                status: Status {
                    brightness: Some(Percent::new(50)?),
                    ..Status::default()
                },
                ..Settings::default()
            };
            settings.show();
            settings.page(Page::Display);
            assert_eq!(
                settings.event(&mouse(false, track.x, track.y), &layout),
                None
            );
            assert_eq!(
                settings.event(&mouse(true, track.x, track.y), &layout),
                Some(Request::Control(Control::Brightness(Percent::new(0)?)))
            );
            assert_eq!(settings.value(0), Some(Percent::new(0)?));
            let motion = Event::MouseMotion {
                timestamp: 0,
                window_id: 1,
                which: 0,
                mousestate: sdl2::mouse::MouseState::from_sdl_state(1),
                x: track.x + track.w + 100,
                y: track.y,
                xrel: 0,
                yrel: 0,
            };
            assert_eq!(
                settings.event(&motion, &layout),
                Some(Request::Control(Control::Brightness(Percent::new(100)?)))
            );
            assert_eq!(settings.value(0), Some(Percent::new(100)?));
            assert_eq!(
                settings.event(&mouse(false, track.x + track.w + 100, track.y), &layout),
                None
            );
            assert_eq!(
                settings.event(&mouse(false, track.x, track.y), &layout),
                None
            );
            settings.event(&mouse(true, track.x, track.y), &layout);
            settings.cancel();
            settings.show();
            assert_eq!(
                settings.event(&mouse(false, track.x, track.y), &layout),
                None
            );
            assert!(track.w > 0);
        }
        Ok(())
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use crate::settings::StorageView;
    #[test]
    fn storage_cards_activate_only_after_matching_touch_release() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut settings = Settings::default();
        for (index, expected) in [(0, StorageView::Apps), (1, StorageView::Categories)] {
            settings.show();
            settings.page(Page::Storage);
            let bounds = PanelLayout::storage_actions(&layout)[index];
            let x = f32::from(u16::try_from(bounds.x + 4).map_err(|_| "x")?) / 480.;
            let y = f32::from(u16::try_from(bounds.y + 4).map_err(|_| "y")?) / 272.;
            let down = Event::FingerDown {
                timestamp: 0,
                touch_id: 1,
                finger_id: 1,
                x,
                y,
                dx: 0.,
                dy: 0.,
                pressure: 1.,
            };
            let up = Event::FingerUp {
                timestamp: 0,
                touch_id: 1,
                finger_id: 1,
                x,
                y,
                dx: 0.,
                dy: 0.,
                pressure: 0.,
            };
            settings.event(&up, &layout);
            assert_eq!(settings.storage_view, StorageView::Overview);
            settings.event(&down, &layout);
            settings.event(&up, &layout);
            assert_eq!(settings.storage_view, expected);
        }
        Ok(())
    }
}
