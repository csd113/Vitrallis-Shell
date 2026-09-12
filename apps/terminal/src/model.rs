//! VT parser adapter with a bounded escape-string guard and input encoding.
use sdl2::keyboard::{Keycode, Mod};
use vitrallis_native::ui;
pub const SCROLLBACK: usize = 1000;
#[derive(Default, Clone, Copy)]
enum Sequence {
    #[default]
    Ground,
    Escape,
    Csi,
    String,
    StringEscape,
    Discard,
    DiscardEscape,
}
pub struct Terminal {
    pub parser: vt100::Parser<Replies>,
    sequence: Sequence,
    length: usize,
}
impl Terminal {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new_with_callbacks(
                rows.max(1),
                cols.max(1),
                SCROLLBACK,
                Replies::default(),
            ),
            sequence: Sequence::Ground,
            length: 0,
        }
    }
    pub fn process(&mut self, bytes: &[u8]) {
        // vte's std-enabled OSC buffer is a Vec. Cap every string before feeding
        // it to vt100, and discard its remainder through the actual terminator.
        for &byte in bytes {
            let previous = self.sequence;
            self.sequence = match (self.sequence, byte) {
                (Sequence::Ground, 27) => Sequence::Escape,
                (Sequence::Escape, b'[') => Sequence::Csi,
                (Sequence::Escape, b']' | b'P' | b'_' | b'^') => Sequence::String,
                (Sequence::Escape, _)
                | (Sequence::Csi, 0x40..=0x7e)
                | (Sequence::String | Sequence::Discard, 7)
                | (Sequence::StringEscape | Sequence::DiscardEscape, b'\\') => Sequence::Ground,
                (Sequence::String, 27) => Sequence::StringEscape,
                (Sequence::StringEscape, _) => Sequence::String,
                (Sequence::Discard, 27) => Sequence::DiscardEscape,
                (Sequence::DiscardEscape, _) => Sequence::Discard,
                (state, _) => state,
            };
            if matches!(previous, Sequence::Discard | Sequence::DiscardEscape) {
                continue;
            }
            if matches!(self.sequence, Sequence::Ground) {
                self.length = 0;
            } else {
                self.length += 1;
            }
            if self.length > 4096 {
                self.parser.process(b"\x1b\\\x18");
                self.sequence =
                    if matches!(self.sequence, Sequence::String | Sequence::StringEscape) {
                        Sequence::Discard
                    } else {
                        Sequence::Ground
                    };
                self.length = 0;
            } else {
                self.parser.process(&[byte]);
            }
        }
    }
}
#[derive(Default)]
pub struct Replies {
    pub bytes: Vec<u8>,
}
impl vt100::Callbacks for Replies {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        _i1: Option<u8>,
        _i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        if self.bytes.len() > 1024 {
            return;
        }
        if c == 'n' && params == [&[6][..]] {
            let (row, col) = screen.cursor_position();
            self.bytes
                .extend_from_slice(format!("\x1b[{};{}R", row + 1, col + 1).as_bytes());
        } else if c == 'n' && params == [&[5][..]] {
            self.bytes.extend_from_slice(b"\x1b[0n");
        } else if c == 'c' {
            self.bytes.extend_from_slice(b"\x1b[?1;2c");
        }
    }
}
pub fn geometry(width: i32, height: i32, scale: i32) -> (u16, u16) {
    (
        u16::try_from((height - 20 * scale) / (9 * scale))
            .unwrap_or(1)
            .clamp(1, 240),
        u16::try_from(width / (8 * scale))
            .unwrap_or(1)
            .clamp(1, 512),
    )
}
pub fn key(key: Keycode, mods: Mod, application: bool) -> Option<Vec<u8>> {
    let alt = mods.intersects(Mod::LALTMOD | Mod::RALTMOD);
    if ui::ctrl(mods) {
        let raw = i32::from(key);
        let byte = if (i32::from(b'a')..=i32::from(b'z')).contains(&raw) {
            u8::try_from(raw - i32::from(b'a') + 1).ok()
        } else {
            match key {
                Keycode::Space | Keycode::At => Some(0),
                Keycode::LeftBracket => Some(27),
                Keycode::Backslash => Some(28),
                Keycode::RightBracket => Some(29),
                Keycode::Caret => Some(30),
                Keycode::Underscore => Some(31),
                _ => None,
            }
        };
        if let Some(byte) = byte {
            return Some(if alt { vec![27, byte] } else { vec![byte] });
        }
    }
    let bytes: &[u8] = match key {
        Keycode::Return | Keycode::KpEnter => b"\r",
        Keycode::Backspace => b"\x7f",
        Keycode::Tab => {
            if ui::shift(mods) {
                b"\x1b[Z"
            } else {
                b"\t"
            }
        }
        Keycode::Escape => b"\x1b",
        Keycode::Up => {
            if application {
                b"\x1bOA"
            } else {
                b"\x1b[A"
            }
        }
        Keycode::Down => {
            if application {
                b"\x1bOB"
            } else {
                b"\x1b[B"
            }
        }
        Keycode::Right => {
            if application {
                b"\x1bOC"
            } else {
                b"\x1b[C"
            }
        }
        Keycode::Left => {
            if application {
                b"\x1bOD"
            } else {
                b"\x1b[D"
            }
        }
        Keycode::Home => {
            if application {
                b"\x1bOH"
            } else {
                b"\x1b[H"
            }
        }
        Keycode::End => {
            if application {
                b"\x1bOF"
            } else {
                b"\x1b[F"
            }
        }
        Keycode::PageUp => b"\x1b[5~",
        Keycode::PageDown => b"\x1b[6~",
        Keycode::Delete => b"\x1b[3~",
        Keycode::Insert => b"\x1b[2~",
        Keycode::F1 => b"\x1bOP",
        Keycode::F2 => b"\x1bOQ",
        Keycode::F3 => b"\x1bOR",
        Keycode::F4 => b"\x1bOS",
        Keycode::F5 => b"\x1b[15~",
        Keycode::F6 => b"\x1b[17~",
        Keycode::F7 => b"\x1b[18~",
        Keycode::F8 => b"\x1b[19~",
        Keycode::F9 => b"\x1b[20~",
        Keycode::F10 => b"\x1b[21~",
        Keycode::F11 => b"\x1b[23~",
        Keycode::F12 => b"\x1b[24~",
        _ => return None,
    };
    let mut output = Vec::with_capacity(bytes.len() + usize::from(alt));
    if alt {
        output.push(27);
    }
    output.extend_from_slice(bytes);
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ansi_cursor_colors_clear_and_wrap() {
        let mut terminal = Terminal::new(3, 4);
        terminal.process(b"abcdE");
        assert_eq!(terminal.parser.screen().cursor_position(), (1, 1));
        assert_eq!(terminal.parser.screen().contents(), "abcdE");
        terminal.process(b"\x1b[2;3H\x1b[31;44mZ");
        let cell = terminal.parser.screen().cell(1, 2).expect("in bounds");
        assert_eq!(cell.contents(), "Z");
        assert_eq!(cell.fgcolor(), vt100::Color::Idx(1));
        assert_eq!(cell.bgcolor(), vt100::Color::Idx(4));
        terminal.process(b"\x1b[2J");
        assert_eq!(terminal.parser.screen().contents(), "");
    }
    #[test]
    fn large_output_scrollback_and_escape_strings_are_bounded() {
        let mut terminal = Terminal::new(4, 20);
        for _ in 0..10_000 {
            terminal.process(b"line\r\n");
        }
        terminal.parser.screen_mut().set_scrollback(usize::MAX);
        assert_eq!(terminal.parser.screen().scrollback(), SCROLLBACK);
        terminal.process(b"\x1b]52;c;");
        for _ in 0..2000 {
            terminal.process(&[b'x'; 8192]);
        }
        terminal.process(b"\x07\x1b[HOK");
        assert!(terminal.length <= 4096);
        terminal.parser.screen_mut().set_scrollback(0);
        assert!(terminal.parser.screen().contents().starts_with("OK"));
    }
    #[test]
    fn geometry_resize_alternate_and_unicode() {
        assert_eq!(geometry(480, 272, 1), (28, 60));
        let mut terminal = Terminal::new(3, 10);
        terminal.process("é中🙂".as_bytes());
        assert_eq!(terminal.parser.screen().cursor_position(), (0, 5));
        terminal.parser.screen_mut().set_size(4, 12);
        assert_eq!(terminal.parser.screen().size(), (4, 12));
        terminal.process(b"\x1b[?1049h\x1b[Halternate");
        assert!(terminal.parser.screen().alternate_screen());
        terminal.process(b"\x1b[?1049l");
        assert!(!terminal.parser.screen().alternate_screen());
    }
    #[test]
    fn keyboard_control_meta_and_navigation_sequences() {
        assert_eq!(key(Keycode::C, Mod::LCTRLMOD, false), Some(vec![3]));
        assert_eq!(key(Keycode::D, Mod::LCTRLMOD, false), Some(vec![4]));
        assert_eq!(key(Keycode::Z, Mod::LCTRLMOD, false), Some(vec![26]));
        assert_eq!(key(Keycode::L, Mod::LCTRLMOD, false), Some(vec![12]));
        assert_eq!(
            key(Keycode::Up, Mod::NOMOD, false),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(key(Keycode::Up, Mod::NOMOD, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(
            key(Keycode::Backspace, Mod::LALTMOD, false),
            Some(vec![27, 127])
        );
        assert_eq!(
            key(Keycode::Delete, Mod::NOMOD, false),
            Some(b"\x1b[3~".to_vec())
        );
    }
}
