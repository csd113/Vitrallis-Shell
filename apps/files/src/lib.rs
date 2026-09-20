//! Native list-based file manager with shared navigation and streamed operations.
use sdl2::keyboard::Keycode;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use vitrallis_native::{
    browser::Browser,
    files,
    ui::{Input, Options, Ui},
};
const BUTTONS: [&str; 4] = ["Open", "Menu", "Refresh", "Close"];

/// # Errors
/// Reports startup/SDL failures. Operational errors remain inside the usable UI.
pub fn run() -> Result<(), String> {
    let Some(options) = Options::parse("vitrallis-files")? else {
        return Ok(());
    };
    let session = vitrallis_native::ui::Session::new("Files", &options)?;
    let creator = session.canvas.texture_creator();
    let mut ui = Ui::new(session, &creator)?;
    let inbox = ui.inbox("files")?;
    let start = options.path.clone().unwrap_or_else(vitrallis_native::home);
    let browser = match Browser::new(&start) {
        Ok(browser) => browser,
        Err(e) => {
            ui.error(&e.to_string())?;
            Browser::new(&vitrallis_native::home()).map_err(|e| e.to_string())?
        }
    };
    let mut manager = Manager {
        browser,
        footer: None,
    };
    manager.render(&mut ui)?;
    if ui.finish_preview(&options)? {
        return Ok(());
    }
    loop {
        let input = ui.wait()?;
        if inbox
            .as_ref()
            .is_some_and(vitrallis_native::ipc::Inbox::take_close_request)
            && ui.idle_for_auto_close(&input)
        {
            return Ok(());
        }
        if matches!(input, Input::Ignore | Input::Wake) {
            continue;
        }
        match manager.input(&mut ui, &input) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => ui.error(&e)?,
        }
        manager.render(&mut ui)?;
        ui.present();
    }
}
struct Manager {
    browser: Browser,
    footer: Option<usize>,
}
impl Manager {
    fn render(&mut self, ui: &mut Ui) -> Result<(), String> {
        ui.clear();
        let title = format!(
            "Files   {} / {}{}",
            self.browser.selected,
            self.browser.entries.len(),
            if self.browser.hidden { "   Hidden" } else { "" }
        );
        ui.header(&title, &self.browser.path.to_string_lossy())?;
        self.browser
            .render(ui, ui.header_height() + 4, self.footer.is_none())?;
        ui.buttons(&BUTTONS, self.footer)
    }
    fn input(&mut self, ui: &mut Ui, input: &Input) -> Result<bool, String> {
        let rows = Browser::visible_rows(ui, ui.header_height() + 4);
        let action = match *input {
            Input::Close => Some(3),
            Input::Key(Keycode::Escape, _) => {
                self.browser.parent().map_err(|e| e.to_string())?;
                self.footer = None;
                None
            }
            Input::Key(Keycode::Tab | Keycode::F6, _) => {
                self.footer = match self.footer {
                    None => Some(0),
                    Some(3) => None,
                    Some(i) => Some(i + 1),
                };
                None
            }
            Input::Key(Keycode::Left | Keycode::Right, _) if self.footer.is_some() => {
                self.footer = Some((self.footer.unwrap_or(0) + 1) % 4);
                None
            }
            Input::Key(Keycode::Up, _) => {
                self.browser.move_by(-1, rows);
                self.footer = None;
                None
            }
            Input::Key(Keycode::Down, _) => {
                self.browser.move_by(1, rows);
                self.footer = None;
                None
            }
            Input::Key(Keycode::PageUp, _) => {
                self.browser
                    .move_by(-isize::try_from(rows).unwrap_or(1), rows);
                None
            }
            Input::Key(Keycode::PageDown, _) => {
                self.browser
                    .move_by(isize::try_from(rows).unwrap_or(1), rows);
                None
            }
            Input::Key(Keycode::Home, _) => {
                self.browser.selected = 0;
                None
            }
            Input::Key(Keycode::End, _) => {
                self.browser.selected = self.browser.entries.len();
                None
            }
            Input::Key(Keycode::Left | Keycode::Backspace, _) => {
                self.browser.parent().map_err(|e| e.to_string())?;
                None
            }
            Input::Key(Keycode::Return | Keycode::KpEnter, _) => Some(self.footer.unwrap_or(0)),
            Input::Key(Keycode::F5, _) => Some(2),
            Input::Key(Keycode::H, m) if vitrallis_native::ui::ctrl(m) => {
                self.browser.hidden = !self.browser.hidden;
                Some(2)
            }
            Input::Key(Keycode::Delete, _) => {
                self.operation(ui, 4)?;
                None
            }
            Input::Key(Keycode::F2, _) => {
                self.operation(ui, 3)?;
                None
            }
            Input::Key(Keycode::M, _) => Some(1),
            Input::Scroll(y) => {
                self.browser
                    .move_by(-isize::try_from(y).unwrap_or(0) * 3, rows);
                None
            }
            Input::Click(x, y) => self.click(ui, x, y),
            _ => None,
        };
        match action {
            Some(0) => {
                if self.browser.activate().map_err(|e| e.to_string())? {
                    self.open(ui)?;
                }
            }
            Some(1) => self.menu(ui)?,
            Some(2) => self.browser.refresh().map_err(|e| e.to_string())?,
            Some(3) => return Ok(true),
            _ => {}
        }
        Ok(false)
    }
    fn click(&mut self, ui: &Ui, x: i32, y: i32) -> Option<usize> {
        if let Some(button) = ui.button_at(x, y, 4) {
            return Some(button);
        }
        let row = ui.row_at(
            y,
            ui.header_height() + 4,
            Browser::row_height(ui),
            Browser::visible_rows(ui, ui.header_height() + 4),
        )?;
        let index = self.browser.offset + row;
        if index > self.browser.entries.len() {
            return None;
        }
        if self.browser.selected == index {
            return Some(0);
        }
        self.browser.selected = index;
        self.footer = None;
        None
    }
    fn path(&self) -> Result<PathBuf, String> {
        self.browser
            .selected_entry()
            .map(|e| e.path.clone())
            .ok_or_else(|| "Select a file or directory first".into())
    }
    fn open(&self, ui: &mut Ui) -> Result<(), String> {
        let path = self.path()?;
        match files::handler(&path).map_err(|e| e.to_string())? {
            files::Handler::Text => {
                if !vitrallis_native::ipc::request_notepad(&path).map_err(|e| e.to_string())? {
                    // Standalone Files waits for its single editor child and reaps it.
                    // Inside Vitrallis all requests use the launcher's supervisor instead.
                    let status = std::process::Command::new(
                        vitrallis_native::companion("vitrallis-notepad")
                            .map_err(|e| e.to_string())?,
                    )
                    .arg("--")
                    .arg(&path)
                    .status()
                    .map_err(|e| e.to_string())?;
                    if !status.success() {
                        return Err(format!("Notepad exited: {status}"));
                    }
                }
            }
            files::Handler::Properties => {
                let info = files::properties(&path).map_err(|e| e.to_string())?;
                ui.choose("File properties", &info, &["Back"])?;
            }
        }
        Ok(())
    }
    fn menu(&mut self, ui: &mut Ui) -> Result<(), String> {
        let labels = [
            "Back",
            "Properties",
            "New folder",
            "Rename",
            "Delete",
            "Copy to",
            "Move to",
            "Hidden",
            "Go to path",
        ];
        let mut selected: usize = 0;
        let mut last = 0;
        let mut offset = 0;
        loop {
            let top = ui.header_height() + 6;
            let rows = Browser::visible_rows(ui, top);
            let height = Browser::row_height(ui);
            if selected < labels.len() {
                last = selected;
                if selected < offset {
                    offset = selected;
                } else if selected >= offset + rows {
                    offset = selected + 1 - rows;
                }
            }
            ui.clear();
            ui.header("Files menu", "Up / Down / Tab select   Enter activates")?;
            for (i, label) in labels.iter().enumerate().skip(offset).take(rows) {
                let y = top + i32::try_from(i - offset).unwrap_or(0) * height;
                if i == selected {
                    ui.fill(
                        sdl2::rect::Rect::new(
                            4,
                            y - 2,
                            (ui.width - 8).unsigned_abs(),
                            height.unsigned_abs(),
                        ),
                        vitrallis_native::theme::SELECTED,
                    )?;
                }
                ui.text(label, 12, y, ui.width - 24, vitrallis_native::theme::TEXT)?;
            }
            ui.buttons(&["Back", "Choose"], selected.checked_sub(labels.len()))?;
            ui.present();
            let action = match ui.wait()? {
                Input::Close | Input::Key(Keycode::Escape, _) => Some(0),
                Input::Key(Keycode::Up | Keycode::Left, _) => {
                    selected = selected.saturating_sub(1);
                    None
                }
                Input::Key(Keycode::Down | Keycode::Right | Keycode::Tab, _) => {
                    selected = (selected + 1) % (labels.len() + 2);
                    None
                }
                Input::Key(Keycode::Return | Keycode::KpEnter, _) => {
                    Some(match selected.cmp(&labels.len()) {
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => last,
                        std::cmp::Ordering::Less => selected,
                    })
                }
                Input::Click(x, y) => ui
                    .button_at(x, y, 2)
                    .map(|i| if i == 0 { 0 } else { last })
                    .or_else(|| {
                        ui.row_at(y, top, height, rows)
                            .map(|row| row + offset)
                            .filter(|row| *row < labels.len())
                    }),
                _ => None,
            };
            if let Some(action) = action {
                return self.operation(ui, action);
            }
        }
    }
    fn operation(&mut self, ui: &mut Ui, action: usize) -> Result<(), String> {
        match action {
            0 => return Ok(()),
            1 => {
                ui.choose(
                    "Properties",
                    &files::properties(&self.path()?).map_err(|e| e.to_string())?,
                    &["Back"],
                )?;
            }
            2 => {
                if let Some(name) = ui.prompt("New folder", "")? {
                    files::create_folder(&self.browser.path, &name).map_err(|e| e.to_string())?;
                }
            }
            3 => {
                let path = self.path()?;
                let initial=path.file_name().ok_or("Missing name")?.to_str().ok_or("This name is not UTF-8; use the terminal to rename it without changing its bytes")?;
                if let Some(name) = ui.prompt("Rename", initial)? {
                    let target =
                        files::named(&self.browser.path, &name).map_err(|e| e.to_string())?;
                    files::move_entry(&path, &target).map_err(|e| e.to_string())?;
                }
            }
            4 => {
                let path = self.path()?;
                if ui.choose(
                    "Delete permanently?",
                    &format!("{}\nNonempty folders are protected.", path.display()),
                    &["Cancel", "Delete"],
                )? == 1
                {
                    files::delete(&path, true).map_err(|e| e.to_string())?;
                }
            }
            5 | 6 => {
                let path = self.path()?;
                if let Some(target) = ui.prompt(
                    if action == 5 {
                        "Copy to full destination path"
                    } else {
                        "Move to full destination path"
                    },
                    &self.browser.path.to_string_lossy(),
                )? {
                    let destination = PathBuf::from(target);
                    if action == 5 {
                        copy(ui, path, destination)?;
                    } else {
                        files::move_entry(&path, &destination).map_err(|e| {
                            format!("{e}\nAcross filesystems: Copy, verify, then Delete.")
                        })?;
                    }
                }
            }
            7 => self.browser.hidden = !self.browser.hidden,
            8 => {
                if let Some(path) =
                    ui.prompt("Go to directory", &self.browser.path.to_string_lossy())?
                {
                    self.browser
                        .enter(&PathBuf::from(path))
                        .map_err(|e| e.to_string())?;
                }
            }
            _ => {}
        }
        self.browser.refresh().map_err(|e| e.to_string())
    }
}
#[derive(Default)]
struct Progress {
    bytes: u64,
    changed: bool,
    result: Option<Result<(), String>>,
}
fn copy(ui: &mut Ui, source: PathBuf, destination: PathBuf) -> Result<(), String> {
    let state = Arc::new(Mutex::new(Progress::default()));
    let output = Arc::clone(&state);
    let sender = ui.sdl.event()?.event_sender();
    let worker = std::thread::Builder::new()
        .name("files-copy".into())
        .spawn(move || {
            let mut last = Instant::now();
            let result = files::copy_entry(&source, &destination, &mut |bytes| {
                if last.elapsed() >= Duration::from_millis(100) {
                    last = Instant::now();
                    if let Ok(mut state) = output.lock() {
                        state.bytes = bytes;
                        if !state.changed {
                            state.changed = true;
                            let _ = vitrallis_native::ui::wake(&sender);
                        }
                    }
                }
            })
            .map_err(|e| e.to_string());
            if let Ok(mut state) = output.lock() {
                state.result = Some(result);
                let _ = vitrallis_native::ui::wake(&sender);
            }
        })
        .map_err(|e| e.to_string())?;
    loop {
        let mut state = state.lock().map_err(|_| "Copy worker state unavailable")?;
        if let Some(result) = state.result.take() {
            drop(state);
            worker.join().map_err(|_| "Copy worker stopped")?;
            return result;
        }
        let bytes = state.bytes;
        state.changed = false;
        drop(state);
        ui.clear();
        ui.header("Copying", &format!("{bytes} bytes copied"))?;
        ui.text(
            "Publishing only when complete",
            8,
            ui.header_height() + 16,
            ui.width - 16,
            vitrallis_native::theme::TEXT,
        )?;
        ui.text(
            "Please wait before closing Files",
            8,
            ui.header_height() + ui.line() + 16,
            ui.width - 16,
            vitrallis_native::theme::MUTED,
        )?;
        ui.present();
        ui.wait()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl2::{event::Event, keyboard::Mod};
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn keyboard_controls_and_cancel_default_delete_preserve_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        sdl2::hint::set("SDL_VIDEODRIVER", "dummy");
        let session = vitrallis_native::ui::Session::new(
            "Files test",
            &Options {
                size: Some((480, 272)),
                ..Options::default()
            },
        )?;
        let creator = session.canvas.texture_creator();
        let mut ui = Ui::new(session, &creator)?;
        let directory =
            std::env::temp_dir().join(format!("vitrallis-files-ui-{}", std::process::id()));
        std::fs::create_dir(&directory)?;
        let scratch = Scratch(directory.canonicalize()?);
        let path = scratch.0.join("note.txt");
        std::fs::write(&path, "keep")?;
        let mut manager = Manager {
            browser: Browser::new(&scratch.0)?,
            footer: None,
        };
        manager.browser.selected = 1;
        ui.sdl.event()?.push_event(Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Return),
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        })?;
        manager.operation(&mut ui, 4)?;
        assert!(path.exists());
        for i in 0..4 {
            manager.input(&mut ui, &Input::Key(Keycode::Tab, Mod::NOMOD))?;
            assert_eq!(manager.footer, Some(i));
        }
        assert!(manager.input(&mut ui, &Input::Key(Keycode::Return, Mod::NOMOD))?);
        manager.footer = None;
        manager.input(&mut ui, &Input::Key(Keycode::End, Mod::NOMOD))?;
        assert_eq!(manager.browser.selected, manager.browser.entries.len());
        manager.input(&mut ui, &Input::Key(Keycode::Home, Mod::NOMOD))?;
        assert_eq!(manager.browser.selected, 0);
        manager.render(&mut ui)?;
        assert!(Browser::row_height(&ui) >= 20);
        ui.sdl.event()?.push_event(Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Escape),
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        })?;
        manager.menu(&mut ui)?;
        assert_eq!(manager.browser.path, scratch.0); // Modal consumes Escape first.
        assert!(!manager.input(&mut ui, &Input::Key(Keycode::Escape, Mod::NOMOD))?);
        assert_eq!(manager.browser.path, scratch.0.parent().ok_or("parent")?);
        manager.browser = Browser::new(std::path::Path::new("/"))?;
        assert!(!manager.input(&mut ui, &Input::Key(Keycode::Escape, Mod::NOMOD))?);
        assert_eq!(manager.browser.path, std::path::Path::new("/"));
        assert!(manager.input(&mut ui, &Input::Close)?);
        Ok(())
    }
}
