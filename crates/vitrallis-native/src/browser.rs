//! Shared, explicitly refreshed directory model and Notepad's compact picker.
use crate::{
    theme::{SELECTED, TEXT},
    ui::{Input, Ui},
};
use sdl2::{keyboard::Keycode, rect::Rect};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub const MAX_ENTRIES: usize = 20_000;
const MAX_LIST_BYTES: usize = 8 * 1024 * 1024;
#[derive(Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub label: String,
    pub directory: bool,
    pub symlink: bool,
    sort: String,
}
#[derive(Debug)]
pub struct Browser {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub hidden: bool,
    pub selected: usize,
    pub offset: usize,
}
impl Browser {
    /// # Errors
    /// Reports unreadable or excessively large directories, preserving the old listing on refresh errors.
    pub fn new(path: &Path) -> io::Result<Self> {
        Self::load(path, false)
    }
    fn load(path: &Path, hidden: bool) -> io::Result<Self> {
        let mut b = Self {
            path: path.to_path_buf(),
            entries: Vec::new(),
            hidden,
            selected: 0,
            offset: 0,
        };
        b.refresh()?;
        Ok(b)
    }
    /// # Errors
    /// Rejects non-directories and listings exceeding 20,000 entries.
    pub fn refresh(&mut self) -> io::Result<()> {
        let path = self.path.canonicalize()?;
        let mut entries = Vec::new();
        let mut seen = 0;
        let mut bytes: usize = 0;
        for entry in fs::read_dir(&path)? {
            seen += 1;
            if seen > MAX_ENTRIES {
                return Err(io::Error::other(
                    "Directory exceeds 20,000 entries; use a narrower directory",
                ));
            }
            let entry = entry?;
            let name = entry.file_name();
            if !self.hidden && name.as_encoded_bytes().first() == Some(&b'.') {
                continue;
            }
            let kind = entry.file_type()?;
            let label = name
                .to_string_lossy()
                .chars()
                .map(|c| if c.is_control() { '�' } else { c })
                .collect::<String>();
            let sort = label.to_lowercase();
            let entry_path = entry.path();
            bytes =
                bytes.saturating_add(label.capacity() + sort.capacity() + entry_path.capacity());
            if bytes > MAX_LIST_BYTES {
                return Err(io::Error::other(
                    "Directory name/path storage exceeds 8 MiB; use a narrower directory",
                ));
            }
            entries.push(Entry {
                path: entry_path,
                sort,
                label,
                directory: kind.is_dir(),
                symlink: kind.is_symlink(),
            });
        }
        entries.sort_unstable_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.sort.cmp(&b.sort))
                .then_with(|| a.path.cmp(&b.path))
        });
        self.path = path;
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len());
        self.offset = self.offset.min(self.selected);
        Ok(())
    }
    /// # Errors
    /// Leaves the previous listing intact when navigation fails.
    pub fn enter(&mut self, path: &Path) -> io::Result<()> {
        let next = Self::load(path, self.hidden)?;
        *self = next;
        Ok(())
    }
    /// # Errors
    /// Reports inaccessible parent directories.
    pub fn parent(&mut self) -> io::Result<()> {
        let parent = self.path.parent().unwrap_or(&self.path).to_path_buf();
        self.enter(&parent)
    }
    #[must_use]
    pub fn selected_entry(&self) -> Option<&Entry> {
        self.selected
            .checked_sub(1)
            .and_then(|i| self.entries.get(i))
    }
    pub fn move_by(&mut self, delta: isize, rows: usize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.entries.len());
        self.reveal(rows);
    }
    pub const fn reveal(&mut self, rows: usize) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + rows {
            self.offset = self.selected + 1 - rows;
        }
    }
    #[must_use]
    pub const fn row_height(ui: &Ui) -> i32 {
        20 * ui.scale
    }
    #[must_use]
    pub fn visible_rows(ui: &Ui, top: i32) -> usize {
        usize::try_from((ui.height - ui.footer_height() - top) / Self::row_height(ui))
            .unwrap_or(1)
            .max(1)
    }
    /// # Errors
    /// Reports rendering errors; only visible rows are visited, with no metadata I/O.
    pub fn render(&mut self, ui: &mut Ui, top: i32, focused: bool) -> Result<(), String> {
        let height = Self::row_height(ui);
        self.reveal(Self::visible_rows(ui, top));
        for row in 0..Self::visible_rows(ui, top) {
            let index = self.offset + row;
            if index > self.entries.len() {
                break;
            }
            let y = top + i32::try_from(row).unwrap_or(0) * height;
            if index == self.selected {
                ui.fill(
                    Rect::new(4, y, (ui.width - 8).unsigned_abs(), height.unsigned_abs()),
                    if focused {
                        SELECTED
                    } else {
                        crate::theme::PANEL
                    },
                )?;
            }
            // The label sits on the row's own vertical centre, so every row
            // shares one baseline whether or not it is highlighted.
            let text_y = y + (height - ui.cell()) / 2;
            if index == 0 {
                ui.text("[..] Parent directory", 8, text_y, ui.width - 16, TEXT)?;
            } else if let Some(entry) = self.entries.get(index - 1) {
                ui.text(
                    if entry.directory {
                        "[D]"
                    } else if entry.symlink {
                        "[L]"
                    } else {
                        "[F]"
                    },
                    8,
                    text_y,
                    32 * ui.scale,
                    TEXT,
                )?;
                ui.text(
                    &entry.label,
                    40 * ui.scale,
                    text_y,
                    ui.width - 48 * ui.scale,
                    TEXT,
                )?;
            }
        }
        Ok(())
    }
    /// # Errors
    /// Reports navigation errors; returns true when a file was activated.
    pub fn activate(&mut self) -> io::Result<bool> {
        let Some(entry) = self.selected_entry() else {
            self.parent()?;
            return Ok(false);
        };
        let path = entry.path.clone();
        if path.is_dir() {
            self.enter(&path)?;
            Ok(false)
        } else {
            Ok(true)
        }
    }
}

/// Open picker shared with Files' listing model; no filesystem worker or watcher.
/// # Errors
/// Reports SDL errors; filesystem errors remain recoverable in the picker.
pub fn pick(ui: &mut Ui, start: &Path) -> Result<Option<PathBuf>, String> {
    let mut browser = Browser::new(start).map_err(|e| e.to_string())?;
    let mut footer = None;
    loop {
        ui.clear();
        ui.header_path("Open text file", &browser.path.to_string_lossy())?;
        browser.render(ui, ui.header_height() + 4, footer.is_none())?;
        ui.buttons(&["Cancel", "Open", "Path", "Hidden"], footer)?;
        ui.present();
        let input = ui.wait()?;
        let action = match input {
            Input::Close | Input::Key(Keycode::Escape, _) => Some(0),
            Input::Key(Keycode::Tab, _) => {
                footer = match footer {
                    None => Some(0),
                    Some(3) => None,
                    Some(i) => Some(i + 1),
                };
                None
            }
            Input::Key(Keycode::Return | Keycode::KpEnter, _) => Some(footer.unwrap_or(1)),
            Input::Key(Keycode::Up, _) => {
                browser.move_by(-1, Browser::visible_rows(ui, ui.header_height() + 4));
                footer = None;
                None
            }
            Input::Key(Keycode::Down, _) => {
                browser.move_by(1, Browser::visible_rows(ui, ui.header_height() + 4));
                footer = None;
                None
            }
            Input::Key(Keycode::PageUp, _) => {
                browser.move_by(
                    -isize::try_from(Browser::visible_rows(ui, ui.header_height() + 4))
                        .unwrap_or(1),
                    Browser::visible_rows(ui, ui.header_height() + 4),
                );
                None
            }
            Input::Key(Keycode::PageDown, _) => {
                browser.move_by(
                    isize::try_from(Browser::visible_rows(ui, ui.header_height() + 4)).unwrap_or(1),
                    Browser::visible_rows(ui, ui.header_height() + 4),
                );
                None
            }
            Input::Click(x, y) => {
                if let Some(i) = ui.button_at(x, y, 4) {
                    Some(i)
                } else {
                    let row = ui.row_at(
                        y,
                        ui.header_height() + 4,
                        Browser::row_height(ui),
                        Browser::visible_rows(ui, ui.header_height() + 4),
                    );
                    let index = row.map_or(usize::MAX, |row| browser.offset + row);
                    if index <= browser.entries.len() {
                        if browser.selected == index {
                            Some(1)
                        } else {
                            browser.selected = index;
                            footer = None;
                            None
                        }
                    } else {
                        None
                    }
                }
            }
            _ => None,
        };
        let result = match action {
            Some(0) => return Ok(None),
            Some(1) => match browser.activate() {
                Ok(true) => return Ok(browser.selected_entry().map(|e| e.path.clone())),
                Ok(false) => Ok(()),
                Err(e) => Err(e),
            },
            Some(2) => {
                if let Some(path) = ui.prompt("Open path", &browser.path.to_string_lossy())? {
                    let path = PathBuf::from(path);
                    if path.is_file() {
                        return Ok(Some(path));
                    }
                    browser.enter(&path)
                } else {
                    Ok(())
                }
            }
            Some(3) => {
                browser.hidden = !browser.hidden;
                browser.refresh()
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            ui.error(&error.to_string())?;
        }
    }
}
