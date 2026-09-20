//! Native, bounded plain-text editor.
use sdl2::{keyboard::Keycode, rect::Rect};
use std::path::{Path, PathBuf};
use vitrallis_native::theme::{ACCENT, MUTED, TEXT};
use vitrallis_native::{
    browser,
    document::Document,
    ui::{self, Input, Options, Ui},
};
const BUTTONS: [&str; 6] = ["New", "Open", "Save", "Save as", "Find", "Close"];

/// Run Notepad's event-driven UI.
/// # Errors
/// Reports startup and unrecoverable SDL errors; file errors use recoverable dialogs.
pub fn run() -> Result<(), String> {
    let Some(options) = Options::parse("vitrallis-notepad")? else {
        return Ok(());
    };
    let session = vitrallis_native::ui::Session::new("Notepad", &options)?;
    let creator = session.canvas.texture_creator();
    let mut ui = Ui::new(session, &creator)?;
    let mut editor = Editor::default();
    let inbox = ui.inbox("notepad")?;
    if let Some(path) = &options.path {
        match Document::open(path) {
            Ok(doc) => editor.document = doc,
            Err(e) => ui.error(&format!("Open {}: {e}", path.display()))?,
        }
    }
    editor.render(&mut ui)?;
    if ui.finish_preview(&options)? {
        return Ok(());
    }
    loop {
        // Dialogs also consume SDL wake events. Check the bounded inbox before
        // waiting again so requests received during a dialog cannot get stranded.
        match inbox
            .as_ref()
            .map_or(Ok(None), vitrallis_native::ipc::Inbox::receive)
        {
            Ok(Some(path)) => {
                if let Err(error) = editor.open_requested(&mut ui, &path) {
                    ui.error(&error)?;
                }
                editor.render(&mut ui)?;
                ui.present();
                continue;
            }
            Err(error) => {
                ui.error(&error)?;
                continue;
            }
            Ok(None) => {}
        }
        let input = ui.wait()?;
        if matches!(input, Input::Ignore) {
            continue;
        }
        match editor.input(&mut ui, input) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => ui.error(&error)?,
        }
        editor.render(&mut ui)?;
        ui.present();
    }
}
#[derive(Default)]
struct Editor {
    document: Document,
    top: usize,
    left: usize,
    footer: Option<usize>,
    find: String,
}
impl Editor {
    fn open_requested(&mut self, ui: &mut Ui, path: &Path) -> Result<(), String> {
        let document = Document::open(path).map_err(|e| e.to_string())?;
        if self.may_discard(ui)? {
            self.document = document;
            self.top = 0;
            self.left = 0;
        }
        Ok(())
    }
    fn reveal(&mut self, ui: &Ui) {
        let rows = ui.rows(ui.header_height() + 4).saturating_sub(1).max(1);
        let row = self.document.row();
        if row < self.top {
            self.top = row;
        } else if row >= self.top + rows {
            self.top = row + 1 - rows;
        }
        let columns = usize::try_from((ui.width - 16) / ui.cell())
            .unwrap_or(1)
            .max(1);
        let col = self.document.column();
        if col < self.left {
            self.left = col;
        } else if col >= self.left + columns {
            self.left = col + 1 - columns;
        }
    }
    fn render(&mut self, ui: &mut Ui) -> Result<(), String> {
        self.reveal(ui);
        ui.clear();
        let title = format!("Notepad{}", if self.document.dirty { " *" } else { "" });
        let path = self
            .document
            .path
            .as_ref()
            .map_or_else(|| "Untitled".into(), |p| p.display().to_string());
        ui.header(&title, &path)?;
        let top = ui.header_height() + 4;
        let rows = ui.rows(top).saturating_sub(1);
        let selection = self.document.selection();
        for row in self.top..(self.top + rows).min(self.document.lines()) {
            let y = top + i32::try_from(row - self.top).unwrap_or(0) * ui.line();
            let line = self.document.line(row);
            for (column, (byte, ch)) in line
                .char_indices()
                .enumerate()
                .skip(self.left)
                .take(usize::try_from((ui.width - 16) / ui.cell()).unwrap_or(0))
            {
                let x = 8 + i32::try_from(column - self.left).unwrap_or(0) * ui.cell();
                if selection.contains(&(self.document.line_start(row).unwrap_or(0) + byte)) {
                    ui.fill(
                        Rect::new(x, y, ui.cell().unsigned_abs(), ui.line().unsigned_abs()),
                        vitrallis_native::theme::SELECTED,
                    )?;
                }
                ui.glyph(if ch == '\t' { '→' } else { ch }, x, y, TEXT)?;
            }
        }
        // Selection endpoints remain indicated without materializing the selected text.
        let row = self.document.row();
        let column = self.document.column();
        if row >= self.top && row < self.top + rows && column >= self.left {
            let x = 8 + i32::try_from(column - self.left).unwrap_or(0) * ui.cell();
            let y = top + i32::try_from(row - self.top).unwrap_or(0) * ui.line();
            ui.fill(Rect::new(x, y, 2, 8 * ui.scale.unsigned_abs()), ACCENT)?;
        }
        let status = format!(
            "Ln {}  Col {}   {} B{}",
            row + 1,
            column + 1,
            self.document.text().len(),
            if selection.is_empty() {
                ""
            } else {
                "  Selected"
            }
        );
        ui.text(
            &status,
            8,
            ui.height - ui.footer_height() - ui.line(),
            ui.width - 16,
            MUTED,
        )?;
        ui.buttons(&BUTTONS, self.footer)
    }
    fn input(&mut self, ui: &mut Ui, input: Input) -> Result<bool, String> {
        if let Some(action) = self.action(ui, &input) {
            return self.command(ui, action);
        }
        let result = match input {
            Input::Key(key, mods) => self.key(ui, key, mods),
            Input::Text(text) if self.footer.is_none() => {
                self.document.insert(&text).map_err(|e| e.to_string())
            }
            Input::Click(x, y) => {
                if y >= ui.header_height() && y < ui.height - ui.footer_height() - ui.line() {
                    let row = self.top
                        + usize::try_from((y - ui.header_height() - 4).max(0) / ui.line())
                            .unwrap_or(0);
                    let col = self.left + usize::try_from((x - 8).max(0) / ui.cell()).unwrap_or(0);
                    self.document.anchor = None;
                    self.document.place(row, col);
                    self.footer = None;
                }
                Ok(())
            }
            Input::Scroll(y) => {
                self.document
                    .vertical(-isize::try_from(y).unwrap_or(0) * 3, false);
                Ok(())
            }
            _ => Ok(()),
        };
        result?;
        Ok(false)
    }
    fn key(&mut self, ui: &Ui, key: Keycode, mods: sdl2::keyboard::Mod) -> Result<(), String> {
        let rows = isize::try_from(ui.rows(ui.header_height() + 4).saturating_sub(1)).unwrap_or(1);
        let select = ui::shift(mods);
        let result = match key {
            Keycode::Tab if self.footer.is_some() => {
                self.footer = match self.footer {
                    Some(5) => None,
                    Some(i) => Some(i + 1),
                    None => Some(0),
                };
                Ok(())
            }
            Keycode::F6 => {
                self.footer = if self.footer.is_some() { None } else { Some(0) };
                Ok(())
            }
            Keycode::Left | Keycode::Right if self.footer.is_some() => {
                let index = self.footer.unwrap_or(0);
                self.footer = Some((index + if key == Keycode::Right { 1 } else { 5 }) % 6);
                Ok(())
            }
            Keycode::Up | Keycode::Down if self.footer.is_some() => {
                self.footer = None;
                Ok(())
            }
            _ if self.footer.is_some() => Ok(()),
            Keycode::Left => {
                self.document.horizontal(false, select);
                Ok(())
            }
            Keycode::Right => {
                self.document.horizontal(true, select);
                Ok(())
            }
            Keycode::Up => {
                self.document.vertical(-1, select);
                Ok(())
            }
            Keycode::Down => {
                self.document.vertical(1, select);
                Ok(())
            }
            Keycode::PageUp => {
                self.document.vertical(-rows, select);
                Ok(())
            }
            Keycode::PageDown => {
                self.document.vertical(rows, select);
                Ok(())
            }
            Keycode::Home => {
                self.document.edge(false, select);
                Ok(())
            }
            Keycode::End => {
                self.document.edge(true, select);
                Ok(())
            }
            Keycode::Return | Keycode::KpEnter => self.document.newline(),
            Keycode::Backspace => self.document.backspace(),
            Keycode::Delete => self.document.delete(),
            Keycode::Tab => self.document.insert("\t"),
            Keycode::A if ui::ctrl(mods) => {
                self.document.select_all();
                Ok(())
            }
            Keycode::C | Keycode::X if ui::ctrl(mods) => {
                let range = self.document.selection();
                if !range.is_empty() {
                    ui.sdl
                        .video()?
                        .clipboard()
                        .set_clipboard_text(&self.document.text()[range.clone()])?;
                    if key == Keycode::X {
                        self.document
                            .replace(range, "")
                            .map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            }
            Keycode::V if ui::ctrl(mods) => {
                let text = ui.sdl.video()?.clipboard().clipboard_text()?;
                self.document.insert(&text)
            }
            _ => Ok(()),
        };
        result.map_err(|e| e.to_string())
    }
    fn action(&self, ui: &Ui, input: &Input) -> Option<usize> {
        match *input {
            Input::Close | Input::Key(Keycode::Escape, _) => Some(5),
            Input::Key(Keycode::N, m) if ui::ctrl(m) => Some(0),
            Input::Key(Keycode::O, m) if ui::ctrl(m) => Some(1),
            Input::Key(Keycode::S, m) if ui::ctrl(m) => Some(if ui::shift(m) { 3 } else { 2 }),
            Input::Key(Keycode::F, m) if ui::ctrl(m) => Some(4),
            Input::Key(Keycode::Return | Keycode::KpEnter, _) if self.footer.is_some() => {
                self.footer
            }
            Input::Click(x, y) => ui.button_at(x, y, BUTTONS.len()),
            _ => None,
        }
    }
    fn command(&mut self, ui: &mut Ui, action: usize) -> Result<bool, String> {
        match action {
            0 => {
                if self.may_discard(ui)? {
                    self.document = Document::default();
                    self.top = 0;
                    self.left = 0;
                }
            }
            1 => {
                if let Some(path) = browser::pick(ui, &self.directory())? {
                    // Validate the new file before asking to discard anything.
                    let document = Document::open(&path).map_err(|e| e.to_string())?;
                    if self.may_discard(ui)? {
                        self.document = document;
                        self.top = 0;
                        self.left = 0;
                    }
                }
            }
            2 | 3 => {
                self.save(ui, action == 3)?;
            }
            4 => {
                if let Some(needle) = ui.prompt("Find text", &self.find)? {
                    self.find = needle;
                    if !self.find.is_empty() {
                        let text = self.document.text();
                        let start = self.document.cursor.min(text.len());
                        let found = text[start..]
                            .find(&self.find)
                            .map(|i| start + i)
                            .or_else(|| text[..start].find(&self.find));
                        if let Some(index) = found {
                            self.document.cursor = index + self.find.len();
                            self.document.anchor = Some(index);
                        } else {
                            ui.error("Text was not found")?;
                        }
                    }
                }
            }
            5 => return self.may_discard(ui),
            _ => {}
        }
        Ok(false)
    }
    fn directory(&self) -> PathBuf {
        self.document
            .path
            .as_deref()
            .and_then(Path::parent)
            .map_or_else(vitrallis_native::home, Path::to_path_buf)
    }
    fn may_discard(&mut self, ui: &mut Ui) -> Result<bool, String> {
        if !self.document.dirty {
            return Ok(true);
        }
        match ui.choose(
            "Unsaved changes",
            "Save this document before continuing?",
            &["Cancel", "Save", "Discard"],
        )? {
            1 => self.save(ui, false),
            2 => Ok(true),
            _ => Ok(false),
        }
    }
    fn save(&mut self, ui: &mut Ui, save_as: bool) -> Result<bool, String> {
        let path = if save_as || self.document.path.is_none() {
            let initial = self
                .document
                .path
                .clone()
                .unwrap_or_else(|| self.directory().join("Untitled.txt"));
            let Some(path) = ui.prompt("Save as", &initial.to_string_lossy())? else {
                return Ok(false);
            };
            PathBuf::from(path)
        } else {
            self.document.path.clone().ok_or("Missing document path")?
        };
        let replacing =
            self.document.path.as_ref() != Some(&path) && path.symlink_metadata().is_ok();
        if replacing
            && ui.choose(
                "Replace file?",
                &path.display().to_string(),
                &["Cancel", "Replace"],
            )? != 1
        {
            return Ok(false);
        }
        self.document
            .save(&path, replacing)
            .map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl2::{event::Event, keyboard::Mod};
    fn keys(ui: &Ui, values: &[Keycode]) -> Result<(), String> {
        for &keycode in values {
            ui.sdl.event()?.push_event(Event::KeyDown {
                timestamp: 0,
                window_id: 0,
                keycode: Some(keycode),
                scancode: None,
                keymod: Mod::NOMOD,
                repeat: false,
            })?;
        }
        Ok(())
    }
    #[test]
    fn unsaved_actions_default_to_cancel_and_save_before_discard()
    -> Result<(), Box<dyn std::error::Error>> {
        sdl2::hint::set("SDL_VIDEODRIVER", "dummy");
        let session = vitrallis_native::ui::Session::new(
            "Notepad test",
            &Options {
                size: Some((480, 272)),
                ..Options::default()
            },
        )?;
        let creator = session.canvas.texture_creator();
        let mut ui = Ui::new(session, &creator)?;
        let scratch = vitrallis_native::files::Temporary::file(&std::env::temp_dir())?.0;
        std::fs::write(&scratch.path, "before\r\n")?;
        let mut editor = Editor {
            document: Document::open(&scratch.path)?,
            ..Editor::default()
        };
        editor.input(&mut ui, Input::Text("edit ".into()))?;
        for action in [0, 5] {
            keys(&ui, &[Keycode::Return])?;
            assert!(!editor.command(&mut ui, action)?);
            assert_eq!(editor.document.text(), "edit before\r\n");
            assert!(editor.document.dirty);
        }
        keys(&ui, &[Keycode::Tab, Keycode::Return])?;
        assert!(editor.command(&mut ui, 5)?);
        assert_eq!(std::fs::read_to_string(&scratch.path)?, "edit before\r\n");
        assert!(!editor.document.dirty);
        editor.input(&mut ui, Input::Text("more".into()))?;
        keys(&ui, &[Keycode::Tab, Keycode::Tab, Keycode::Return])?;
        assert!(!editor.command(&mut ui, 0)?);
        assert_eq!(editor.document.text(), "");
        editor.key(&ui, Keycode::Tab, Mod::NOMOD)?;
        assert_eq!(editor.document.text(), "\t");
        editor.key(&ui, Keycode::F6, Mod::NOMOD)?;
        for i in 0..6 {
            assert_eq!(editor.footer, Some(i));
            editor.key(&ui, Keycode::Delete, Mod::NOMOD)?;
            assert_eq!(editor.document.text(), "\t");
            editor.key(&ui, Keycode::Tab, Mod::NOMOD)?;
        }
        assert_eq!(editor.footer, None);
        editor.render(&mut ui)?;
        Ok(())
    }
}
