use crate::navigation::Direction;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    System,
    Move(Direction),
    Activate,
    SelectAndActivate(usize),
    Back,
    Page(bool),
}

mod sdl {
    use super::Action;
    use crate::{layout::Layout, navigation::Direction};
    use sdl2::{event::Event, keyboard::Keycode, mouse::MouseButton};
    pub fn action(event: &Event, layout: &Layout, count: usize) -> Option<Action> {
        match *event {
            Event::KeyDown {
                keycode: Some(key),
                repeat: false,
                ..
            } => match key {
                Keycode::Power => Some(Action::System),
                Keycode::PageUp => Some(Action::Page(false)),
                Keycode::PageDown => Some(Action::Page(true)),
                Keycode::Left => Some(Action::Move(Direction::Left)),
                Keycode::Right => Some(Action::Move(Direction::Right)),
                Keycode::Up => Some(Action::Move(Direction::Up)),
                Keycode::Down => Some(Action::Move(Direction::Down)),
                Keycode::Return | Keycode::KpEnter => Some(Action::Activate),
                Keycode::Escape | Keycode::Home => Some(Action::Back),
                _ => None,
            },
            // Touch-generated mouse events are excluded; native finger events handle them.
            Event::MouseButtonUp {
                x,
                y,
                mouse_btn: MouseButton::Left,
                which,
                ..
            } if which != u32::MAX => pointer(layout, f64::from(x), f64::from(y), count),
            Event::FingerUp { x, y, .. } => pointer(
                layout,
                f64::from(x) * f64::from(layout.width),
                f64::from(y) * f64::from(layout.height),
                count,
            ),
            _ => None,
        }
    }

    fn pointer(layout: &Layout, x: f64, y: f64, count: usize) -> Option<Action> {
        if layout.footer.contains(x, y) {
            return Some(Action::System);
        }
        if layout.previous.contains(x, y) {
            return Some(Action::Page(false));
        }
        if layout.next.contains(x, y) {
            return Some(Action::Page(true));
        }
        layout.hit(x, y, count).map(Action::SelectAndActivate)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn keyboard_touch_and_mouse_are_translated_without_repeat() -> Result<(), String> {
            let layout = Layout::new(480, 272, 3, 2)?;
            let mut key = Event::KeyDown {
                timestamp: 0,
                window_id: 1,
                keycode: Some(Keycode::Return),
                scancode: None,
                keymod: sdl2::keyboard::Mod::NOMOD,
                repeat: false,
            };
            assert!(matches!(action(&key, &layout, 6), Some(Action::Activate)));
            if let Event::KeyDown { repeat, .. } = &mut key {
                *repeat = true;
            }
            assert!(action(&key, &layout, 6).is_none());
            if let Event::KeyDown {
                keycode, repeat, ..
            } = &mut key
            {
                *repeat = false;
                *keycode = Some(Keycode::F1);
            }
            assert_eq!(action(&key, &layout, 6), None);
            let touch = Event::FingerUp {
                timestamp: 0,
                touch_id: 1,
                finger_id: 1,
                x: 0.5,
                y: 0.3,
                dx: 0.,
                dy: 0.,
                pressure: 0.,
            };
            assert!(matches!(
                action(&touch, &layout, 6),
                Some(Action::SelectAndActivate(1))
            ));
            let mouse = Event::MouseButtonUp {
                timestamp: 0,
                window_id: 1,
                which: u32::MAX,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x: 240,
                y: 80,
            };
            assert!(action(&mouse, &layout, 6).is_none());
            Ok(())
        }
    }
}
pub use sdl::action;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopAction {
    Add,
    Menu(Option<usize>),
    Settings,
    Focus,
}
#[derive(Debug, Default)]
pub struct DesktopInput {
    contact: Option<(i64, i64, u32, DesktopAction)>,
}
impl DesktopInput {
    pub const fn clear(&mut self) {
        self.contact = None;
    }
    pub fn event(
        &mut self,
        event: &sdl2::event::Event,
        layout: &crate::layout::Layout,
        count: usize,
    ) -> Option<DesktopAction> {
        use sdl2::{
            event::{Event, WindowEvent},
            keyboard::Keycode,
            mouse::MouseButton,
        };
        let target = |x, y| desktop_target(layout, count, x, y);
        let (id, finger, timestamp, down, hit, held) = match *event {
            Event::KeyDown {
                keycode: Some(key),
                repeat: false,
                ..
            } => {
                self.clear();
                return match key {
                    Keycode::F2 => Some(DesktopAction::Add),
                    Keycode::F10 | Keycode::Application => Some(DesktopAction::Menu(None)),
                    _ => None,
                };
            }
            Event::MouseButtonDown {
                which,
                mouse_btn,
                x,
                y,
                timestamp,
                ..
            }
            | Event::MouseButtonUp {
                which,
                mouse_btn,
                x,
                y,
                timestamp,
                ..
            } if which != u32::MAX
                && matches!(mouse_btn, MouseButton::Left | MouseButton::Right) =>
            {
                (
                    i64::from(which),
                    0,
                    timestamp,
                    matches!(event, Event::MouseButtonDown { .. }),
                    target(f64::from(x), f64::from(y)),
                    mouse_btn == MouseButton::Right,
                )
            }
            Event::FingerDown {
                touch_id,
                finger_id,
                timestamp,
                x,
                y,
                ..
            }
            | Event::FingerUp {
                touch_id,
                finger_id,
                timestamp,
                x,
                y,
                ..
            } => (
                touch_id,
                finger_id,
                timestamp,
                matches!(event, Event::FingerDown { .. }),
                target(
                    f64::from(x) * f64::from(layout.width),
                    f64::from(y) * f64::from(layout.height),
                ),
                matches!(event, Event::FingerDown { .. }),
            ),
            Event::FingerMotion {
                touch_id,
                finger_id,
                x,
                y,
                ..
            } => {
                let hit = target(
                    f64::from(x) * f64::from(layout.width),
                    f64::from(y) * f64::from(layout.height),
                );
                if self.contact.is_some_and(|(id, finger, _, t)| {
                    id != touch_id || finger != finger_id || hit != Some(t)
                }) {
                    self.clear();
                }
                return None;
            }
            Event::Window {
                win_event: WindowEvent::FocusLost,
                ..
            } => {
                self.clear();
                return None;
            }
            _ => return None,
        };
        self.finish(id, finger, timestamp, down, hit, held)
    }
    fn finish(
        &mut self,
        id: i64,
        finger: i64,
        timestamp: u32,
        down: bool,
        hit: Option<DesktopAction>,
        held: bool,
    ) -> Option<DesktopAction> {
        if down {
            self.contact = if self.contact.is_some() {
                None
            } else {
                hit.filter(|t| held || !matches!(t, DesktopAction::Menu(Some(_))))
                    .map(|t| (id, finger, timestamp, t))
            };
            None
        } else {
            let (previous_id, previous_finger, start, previous) = self.contact.take()?;
            hit.filter(|t| {
                previous_id == id
                    && previous_finger == finger
                    && previous == *t
                    && (!matches!(t, DesktopAction::Menu(Some(_)))
                        || held
                        || timestamp.wrapping_sub(start) >= 600)
            })
        }
    }
}

fn desktop_target(
    layout: &crate::layout::Layout,
    count: usize,
    x: f64,
    y: f64,
) -> Option<DesktopAction> {
    if layout.add_shortcut.contains(x, y) {
        Some(DesktopAction::Add)
    } else if layout.desktop_menu.contains(x, y) {
        Some(DesktopAction::Menu(None))
    } else {
        layout
            .hit(x, y, count)
            .map(|index| DesktopAction::Menu(Some(index)))
    }
}

/// Require press and release on the same target and pointer. Clear this when
/// focus changes or an app exits so stale releases cannot launch another app.
#[derive(Debug, Default)]
pub struct PointerInput {
    pressed: Option<(Pointer, Action)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pointer {
    Mouse(u32),
    Finger(i64, i64),
}
impl PointerInput {
    pub const fn clear(&mut self) {
        self.pressed = None;
    }
    pub fn action(
        &mut self,
        event: &sdl2::event::Event,
        layout: &crate::layout::Layout,
        count: usize,
    ) -> Option<Action> {
        use sdl2::{
            event::{Event, WindowEvent},
            mouse::MouseButton,
        };
        let (pointer, down, mut translated) = match *event {
            Event::MouseButtonDown {
                which,
                mouse_btn: MouseButton::Left,
                ..
            } if which != u32::MAX => {
                let mut release = event.clone();
                if let Event::MouseButtonDown {
                    timestamp,
                    window_id,
                    which,
                    mouse_btn,
                    clicks,
                    x,
                    y,
                } = release
                {
                    release = Event::MouseButtonUp {
                        timestamp,
                        window_id,
                        which,
                        mouse_btn,
                        clicks,
                        x,
                        y,
                    };
                }
                (Pointer::Mouse(which), true, action(&release, layout, count))
            }
            Event::MouseButtonUp {
                which,
                mouse_btn: MouseButton::Left,
                ..
            } if which != u32::MAX => (Pointer::Mouse(which), false, action(event, layout, count)),
            Event::FingerDown {
                timestamp,
                touch_id,
                finger_id,
                x,
                y,
                dx,
                dy,
                pressure,
            } => {
                let release = Event::FingerUp {
                    timestamp,
                    touch_id,
                    finger_id,
                    x,
                    y,
                    dx,
                    dy,
                    pressure,
                };
                (
                    Pointer::Finger(touch_id, finger_id),
                    true,
                    action(&release, layout, count),
                )
            }
            Event::FingerUp {
                touch_id,
                finger_id,
                ..
            } => (
                Pointer::Finger(touch_id, finger_id),
                false,
                action(event, layout, count),
            ),
            Event::Window {
                win_event: WindowEvent::FocusLost,
                ..
            } => {
                self.clear();
                return None;
            }
            Event::KeyDown { .. } => {
                self.clear();
                return action(event, layout, count);
            }
            _ => return action(event, layout, count),
        };
        if down {
            // A second touch cancels the gesture, rather than switching fingers.
            if self.pressed.is_some() {
                translated = None;
            }
            self.pressed = translated.map(|target| (pointer, target));
            None
        } else {
            let pressed = self.pressed.take();
            translated.filter(|target| pressed == Some((pointer, *target)))
        }
    }
}

#[cfg(test)]
mod gesture_tests {
    use super::*;
    use sdl2::event::Event;
    #[test]
    fn finger_requires_matching_press_release_and_page_buttons_work() -> Result<(), String> {
        let layout = crate::layout::Layout::home(480, 272)?;
        let mut pointer = PointerInput::default();
        let down = Event::FingerDown {
            timestamp: 0,
            touch_id: 1,
            finger_id: 1,
            x: 0.5,
            y: 0.3,
            dx: 0.,
            dy: 0.,
            pressure: 1.,
        };
        let mut up = Event::FingerUp {
            timestamp: 0,
            touch_id: 1,
            finger_id: 1,
            x: 0.5,
            y: 0.3,
            dx: 0.,
            dy: 0.,
            pressure: 0.,
        };
        assert!(pointer.action(&up, &layout, 6).is_none());
        assert!(pointer.action(&down, &layout, 6).is_none());
        assert_eq!(
            pointer.action(&up, &layout, 6),
            Some(Action::SelectAndActivate(1))
        );
        pointer.action(&down, &layout, 6);
        if let Event::FingerUp { finger_id, .. } = &mut up {
            *finger_id = 2;
        }
        assert!(pointer.action(&up, &layout, 6).is_none());
        pointer.action(&down, &layout, 6);
        pointer.clear();
        assert!(pointer.action(&up, &layout, 6).is_none());
        for (tile, forward) in [(layout.previous, false), (layout.next, true)] {
            let x = f32::from(u16::try_from(tile.x + tile.w / 2).map_err(|_| "x")?) / 480.;
            let y = f32::from(u16::try_from(tile.y + tile.h / 2).map_err(|_| "y")?) / 272.;
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
            assert!(pointer.action(&down, &layout, 6).is_none());
            assert_eq!(pointer.action(&up, &layout, 6), Some(Action::Page(forward)));
        }
        Ok(())
    }
}
