//! Normalize the built-in `PocketCHIP` Fn layer before dispatching SDL events.
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::{Keycode, Mod},
};

/// Device keyboard translation shared by the launcher and native applications.
pub struct Keyboard {
    pocketchip: bool,
    text_modifiers: Mod,
}

impl Keyboard {
    /// Detect the device once. Resolution and session flags do not identify a keyboard.
    #[must_use]
    pub fn new(video: &sdl2::VideoSubsystem) -> Self {
        let compatible = if cfg!(target_os = "linux") && video.current_video_driver() == "x11" {
            std::fs::read("/proc/device-tree/compatible").unwrap_or_default()
        } else {
            Vec::new()
        };
        Self::from_compatible(&compatible)
    }

    fn from_compatible(compatible: &[u8]) -> Self {
        Self {
            pocketchip: compatible
                .split(|byte| *byte == 0)
                .any(|name| name == b"nextthing,pocketchip"),
            text_modifiers: Mod::NOMOD,
        }
    }

    /// Translate key events in place, preserving timestamps, scancodes and repeats.
    /// Text stays supplied by the active X11 keymap, including Fn punctuation.
    pub fn event(&mut self, event: &mut Event) {
        match event {
            Event::KeyDown {
                keycode, keymod, ..
            } => {
                self.translate(keycode, keymod);
                self.text_modifiers = *keymod;
            }
            Event::KeyUp {
                keycode, keymod, ..
            } => self.translate(keycode, keymod),
            Event::Window {
                win_event: WindowEvent::FocusLost,
                ..
            } => self.text_modifiers = Mod::NOMOD,
            _ => {}
        }
    }

    /// Modifiers from the key event producing text, rather than SDL's later global state.
    #[must_use]
    pub const fn text_modifiers(&self) -> Mod {
        self.text_modifiers
    }

    fn translate(&self, key: &mut Option<Keycode>, modifiers: &mut Mod) {
        // SDL2 exposes ISO_Level3_Shift as Right Alt, and Mode_switch as Mode.
        // Neither is a terminal Meta modifier on the built-in keyboard.
        let fn_modifiers = Mod::RALTMOD | Mod::MODEMOD;
        if !self.pocketchip || !modifiers.intersects(fn_modifiers) {
            return;
        }
        *key = key.map(|key| match key {
            Keycode::Num1 => Keycode::F1,
            Keycode::Num2 => Keycode::F2,
            Keycode::Num3 => Keycode::F3,
            Keycode::Num4 => Keycode::F4,
            Keycode::Num5 => Keycode::F5,
            Keycode::Num6 => Keycode::F6,
            Keycode::Num7 => Keycode::F7,
            Keycode::Num8 => Keycode::F8,
            Keycode::Num9 => Keycode::F9,
            Keycode::Num0 => Keycode::F10,
            Keycode::Minus => Keycode::F11,
            Keycode::Equals => Keycode::F12,
            _ => key,
        });
        modifiers.remove(fn_modifiers);
    }
}

#[cfg(test)]
mod tests;
