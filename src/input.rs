use crate::navigation::Direction;
#[derive(Debug, Clone, Copy)]
pub enum Action {
    Move(Direction),
    Activate,
    SelectAndActivate(usize),
    Back,
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
            } if which != u32::MAX => layout
                .hit(f64::from(x), f64::from(y), count)
                .map(Action::SelectAndActivate),
            Event::FingerUp { x, y, .. } => {
                layout.touch(x, y, count).map(Action::SelectAndActivate)
            }
            _ => None,
        }
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
