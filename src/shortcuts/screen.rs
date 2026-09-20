//! Native desktop menu, scrollable editor, file picker and touch text entry.
use super::{Draft, Store, command::Mode};
use crate::{
    app::{AppEntry, AppSource},
    layout::{Layout, Rect},
};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::{Keycode, Mod},
    mouse::MouseButton,
};
use std::path::PathBuf;

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Command,
    Cwd,
    FolderName,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toolbar {
    Actions,
    Back,
}
impl Toolbar {
    pub const fn bounds(self, layout: &Layout) -> Rect {
        match self {
            Self::Actions => layout.desktop_menu,
            Self::Back => layout.folder_back,
        }
    }
}
pub const ACTIONS_LABEL: &str = "Manage [F10]";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Menu,
    Editor,
    Text(Field),
    Browse(bool),
    Remove,
    FolderMove,
    FolderDelete,
}
#[derive(Debug, Clone, Copy, Default)]
enum KeyboardPage {
    #[default]
    Lower,
    Upper,
    Symbols,
}
impl KeyboardPage {
    const fn keys(self) -> &'static str {
        match self {
            Self::Lower => "abcdefghijklmnopqrstuvwxyz0123456789 /._-",
            Self::Upper => "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 /._-",
            Self::Symbols => "`~!@#$%^&*()-_=+[{]}\\|;:'\",<.>/? 0123456789",
        }
    }
    const fn next(self) -> Self {
        match self {
            Self::Lower => Self::Upper,
            Self::Upper => Self::Symbols,
            Self::Symbols => Self::Lower,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::Lower => "ABC",
            Self::Upper => "#+=",
            Self::Symbols => "abc",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    MoveEarlier,
    MoveLater,
    CreateFolder,
    RenameFolder,
    DeleteFolder,
    MoveApp,
    Destination(usize),
    Unfile,
    Add,
    Edit,
    Remove,
    Uninstall,
    Cancel,
    Save,
    Field(Field),
    Icon,
    DefaultIcon,
    Terminal,
    Shell,
    Previous,
    Next,
    Item(usize),
    Parent,
    Character(char),
    Delete,
    Clear,
    Shift,
    Done,
    Confirm,
}
#[derive(Debug)]
pub enum Request {
    Reorder(bool),
    Folder(crate::folders::Change),
    Save,
    Remove,
    Uninstall,
}

#[derive(Debug)]
pub struct Desktop {
    pub toolbar: Option<Toolbar>,
    pub in_folder: bool,
    pub folders: crate::folders::Folders,
    pub folder_context: Option<String>,
    folder_edit: Option<String>,
    pub open: bool,
    pub draft: Draft,
    pub entry: Option<AppEntry>,
    pub error: String,
    pub selected: usize,
    page: Page,
    scroll: usize,
    text: String,
    keyboard_page: KeyboardPage,
    directory: PathBuf,
    files: Vec<(PathBuf, bool)>,
    contact: Option<(i64, i64, Target)>,
}
impl Default for Desktop {
    fn default() -> Self {
        Self {
            toolbar: None,
            in_folder: false,
            folders: crate::folders::Folders::default(),
            folder_context: None,
            folder_edit: None,
            open: false,
            draft: Draft::default(),
            entry: None,
            error: String::new(),
            selected: 0,
            page: Page::Menu,
            scroll: 0,
            text: String::new(),
            keyboard_page: KeyboardPage::default(),
            directory: PathBuf::new(),
            files: Vec::new(),
            contact: None,
        }
    }
}

impl Desktop {
    #[cfg(test)]
    pub fn qa_samples(layout: &Layout) -> Vec<(&'static str, Self)> {
        [
            ("actions", Page::Menu),
            ("editor", Page::Editor),
            ("command", Page::Text(Field::Command)),
            ("remove", Page::Remove),
            ("picker", Page::Browse(true)),
        ]
        .into_iter()
        .map(|(name, page)| {
            let mut desktop = Self::default();
            desktop.add();
            desktop.page = page;
            if page == Page::Menu {
                desktop.entry =
                    crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"))
                        .into_iter()
                        .next();
            }
            desktop.draft.name = "My Linux program".into();
            desktop.draft.command = "'/home/alex/My Tools/program' 'quoted argument'".into();
            desktop.draft.cwd = "/home/alex/My Documents".into();
            desktop.text.clone_from(&desktop.draft.command);
            desktop.directory = "/home/alex/Pictures".into();
            desktop.files = (0..10)
                .map(|i| {
                    (
                        PathBuf::from(format!("/home/alex/Pictures/Chosen icon {i}.png")),
                        false,
                    )
                })
                .collect();
            if desktop.editing() {
                desktop.selected = desktop.targets(layout).len().saturating_sub(1);
            }
            (name, desktop)
        })
        .collect()
    }

    pub fn toolbar_event(&mut self, event: &Event) -> Option<crate::input::DesktopAction> {
        use crate::input::DesktopAction;
        if self.open {
            return None;
        }
        let Event::KeyDown {
            keycode: Some(key),
            keymod,
            repeat: false,
            ..
        } = event
        else {
            return None;
        };
        match *key {
            Keycode::Tab => {
                let previous = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
                self.toolbar = match (self.toolbar, previous, self.in_folder) {
                    (None, false, true) | (Some(Toolbar::Actions), true, true) => {
                        Some(Toolbar::Back)
                    }
                    (None, _, _) | (Some(Toolbar::Back), false, _) => Some(Toolbar::Actions),
                    _ => None,
                };
                Some(DesktopAction::Focus)
            }
            Keycode::Escape | Keycode::Up if self.toolbar.is_some() => {
                self.toolbar = None;
                Some(DesktopAction::Focus)
            }
            Keycode::Left | Keycode::Right if self.toolbar.is_some() => {
                self.toolbar = Some(
                    if self.in_folder && self.toolbar == Some(Toolbar::Actions) {
                        Toolbar::Back
                    } else {
                        Toolbar::Actions
                    },
                );
                Some(DesktopAction::Focus)
            }
            Keycode::Return | Keycode::KpEnter | Keycode::Space if self.toolbar.is_some() => {
                match self.toolbar.take()? {
                    Toolbar::Back => Some(DesktopAction::Back),
                    Toolbar::Actions => Some(DesktopAction::Menu(None)),
                }
            }
            _ => None,
        }
    }
    pub fn menu(&mut self, entry: Option<AppEntry>) {
        self.toolbar = None;
        self.entry = entry;
        self.open = true;
        self.error.clear();
        self.page(Page::Menu);
    }
    pub fn add(&mut self) {
        self.toolbar = None;
        self.entry = None;
        self.draft = Draft::default();
        self.open = true;
        self.page(Page::Editor);
    }
    fn page(&mut self, page: Page) {
        self.page = page;
        self.selected = 0;
        self.scroll = 0;
        self.contact = None;
        self.error.clear();
    }
    pub const fn editing(&self) -> bool {
        self.open && matches!(self.page, Page::Text(_))
    }
    pub const fn title(&self) -> &'static str {
        match self.page {
            Page::Menu => "Desktop actions",
            Page::Editor if self.entry.is_some() => "Edit shortcut",
            Page::Editor => "Add shortcut",
            Page::Text(Field::Name) => "Name",
            Page::Text(Field::Command) => "Command",
            Page::Text(Field::Cwd) => "Working directory (optional)",
            Page::Text(Field::FolderName) => "Folder name",
            Page::FolderMove => "Move app to folder",
            Page::FolderDelete => "Delete folder?",
            Page::Browse(true) => "Choose icon: PNG / BMP, up to 512x512",
            Page::Browse(false) => "Choose executable or script",
            Page::Remove => "Remove shortcut?",
        }
    }
    pub fn description(&self) -> String {
        if !self.error.is_empty() {
            return self.error.clone();
        }
        match self.page {
            Page::Menu => self.entry.as_ref().map_or_else(
                || "Add a Linux program or script".into(),
                |app| app.name.clone(),
            ),
            Page::Editor => "Tab/arrows: select | Enter/tap: edit | PgUp/PgDn: scroll".into(),
            Page::Text(_) => format!("{}|", self.text),
            Page::Browse(_) => self.directory.display().to_string(),
            Page::FolderMove => "Choose a destination; apps stay installed".into(),
            Page::FolderDelete => "Apps return to Apps. No app or data will be deleted.".into(),
            Page::Remove => {
                "Only the shortcut is removed. The program and its data are kept.".into()
            }
        }
    }
    pub fn preview(&self) -> Option<&[u8]> {
        matches!(self.page, Page::Editor)
            .then_some(self.draft.icon.as_deref())
            .flatten()
    }
    fn menu_rows(&self) -> Vec<(Target, String)> {
        let mut rows = vec![
            (Target::Add, "Add shortcut".into()),
            (Target::CreateFolder, "Create folder".into()),
        ];
        if self.folder_context.is_some() {
            rows.extend([
                (Target::RenameFolder, "Rename folder".into()),
                (Target::DeleteFolder, "Delete folder".into()),
            ]);
        }
        if let Some(app) = &self.entry {
            rows.extend([
                (Target::MoveEarlier, "Move earlier".into()),
                (Target::MoveLater, "Move later".into()),
            ]);
            if !matches!(app.source, AppSource::System | AppSource::Folder) {
                rows.push((Target::MoveApp, "Move app to folder / Apps".into()));
            }
            match app.source {
                AppSource::Custom => rows.extend([
                    (Target::Edit, "Edit shortcut".into()),
                    (Target::Remove, "Remove shortcut".into()),
                ]),
                AppSource::AppCenter => {
                    rows.push((Target::Uninstall, "Uninstall app".into()));
                }
                _ if super::hidden_key(app).is_some() => {
                    rows.push((Target::Remove, "Remove shortcut".into()));
                }
                _ => (),
            }
        }
        rows
    }
    fn rows(&self) -> Vec<(Target, String)> {
        match self.page {
            Page::Menu => self.menu_rows(),
            Page::FolderMove => std::iter::once((Target::Unfile, "Apps (unfiled)".into()))
                .chain(
                    self.folders
                        .names
                        .values()
                        .enumerate()
                        .map(|(index, name)| (Target::Destination(index), name.clone())),
                )
                .collect(),
            Page::Editor => vec![
                (
                    Target::Field(Field::Name),
                    format!("Name: {}", self.draft.name),
                ),
                (
                    Target::Field(Field::Command),
                    format!("Command: {}", self.draft.command),
                ),
                (Target::Item(usize::MAX), "Browse executable...".into()),
                (
                    Target::Field(Field::Cwd),
                    format!(
                        "Working directory: {}",
                        if self.draft.cwd.is_empty() {
                            "Home"
                        } else {
                            &self.draft.cwd
                        }
                    ),
                ),
                (Target::Icon, "Choose icon...".into()),
                (Target::DefaultIcon, "Use default icon".into()),
                (
                    Target::Terminal,
                    format!(
                        "[{}] Run in terminal",
                        if self.draft.terminal { "x" } else { " " }
                    ),
                ),
                (
                    Target::Shell,
                    format!(
                        "[{}] Shell mode (/bin/sh -c)",
                        if self.draft.mode == Mode::Shell {
                            "x"
                        } else {
                            " "
                        }
                    ),
                ),
            ],
            Page::Browse(_) => self
                .files
                .iter()
                .enumerate()
                .map(|(i, (path, directory))| {
                    (
                        Target::Item(i),
                        format!(
                            "{}{}",
                            if *directory { "[DIR] " } else { "" },
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                    )
                })
                .collect(),
            Page::Text(_) | Page::Remove | Page::FolderDelete => Vec::new(),
        }
    }
    fn capacity(layout: &Layout) -> usize {
        usize::from(layout.height.saturating_sub(100) / 32).max(1)
    }
    pub fn targets(&self, layout: &Layout) -> Vec<(Target, String, Rect)> {
        let width = i32::from(layout.width);
        let height = i32::from(layout.height);
        let mut targets = Vec::new();
        if matches!(self.page, Page::Text(_)) {
            let keys = self.keyboard_page.keys();
            for (i, key) in keys.chars().enumerate() {
                let i = i32::try_from(i).unwrap_or(0);
                targets.push((
                    Target::Character(key),
                    if key == ' ' {
                        "Space".into()
                    } else {
                        key.to_string()
                    },
                    Rect {
                        x: 8 + (i % 10) * ((width - 16) / 10),
                        y: 72 + (i / 10) * 28,
                        w: (width - 16) / 10 - 2,
                        h: 26,
                    },
                ));
            }
        } else {
            for (i, (target, label)) in self
                .rows()
                .into_iter()
                .skip(self.scroll)
                .take(Self::capacity(layout))
                .enumerate()
            {
                targets.push((
                    target,
                    label,
                    Rect {
                        x: 8,
                        y: 64 + i32::try_from(i).unwrap_or(0) * 32,
                        w: width - if self.page == Page::Menu { 16 } else { 64 },
                        h: 30,
                    },
                ));
            }
            if self.page != Page::Menu && self.rows().len() > Self::capacity(layout) {
                targets.extend([
                    (
                        Target::Previous,
                        "Up".into(),
                        Rect {
                            x: width - 52,
                            y: 64,
                            w: 44,
                            h: 44,
                        },
                    ),
                    (
                        Target::Next,
                        "Down".into(),
                        Rect {
                            x: width - 52,
                            y: 112,
                            w: 44,
                            h: 44,
                        },
                    ),
                ]);
            }
        }
        let mut footer = self.footer();
        if self.page == Page::Menu && self.rows().len() > Self::capacity(layout) {
            footer.extend([(Target::Previous, "Previous"), (Target::Next, "Next")]);
        }
        let cell = (width - 16) / i32::try_from(footer.len()).unwrap_or(1);
        for (i, (target, label)) in footer.into_iter().enumerate() {
            targets.push((
                target,
                label.into(),
                Rect {
                    x: 8 + i32::try_from(i).unwrap_or(0) * cell,
                    y: height - 34,
                    w: cell - 4,
                    h: 30,
                },
            ));
        }
        targets
    }
    fn footer(&self) -> Vec<(Target, &'static str)> {
        match self.page {
            Page::Editor => vec![(Target::Cancel, "Cancel"), (Target::Save, "Save")],
            Page::Menu | Page::FolderMove => vec![(Target::Cancel, "Cancel")],
            Page::FolderDelete => vec![
                (Target::Cancel, "Cancel"),
                (Target::Confirm, "Delete folder"),
            ],
            Page::Browse(_) => vec![
                (Target::Cancel, "Cancel"),
                (Target::Parent, "Parent folder"),
            ],
            Page::Text(_) => vec![
                (Target::Cancel, "Cancel"),
                (Target::Clear, "Clear"),
                (Target::Delete, "Backspace"),
                (Target::Shift, self.keyboard_page.label()),
                (Target::Done, "Done"),
            ],
            Page::Remove => vec![
                (Target::Cancel, "Cancel"),
                (Target::Confirm, "Remove shortcut"),
            ],
        }
    }
    fn browse(&mut self, icons: bool) -> Result<(), String> {
        if self.directory.as_os_str().is_empty() {
            self.directory = super::user_home()?;
        }
        self.read_directory()?;
        self.page(Page::Browse(icons));
        Ok(())
    }
    fn read_directory(&mut self) -> Result<(), String> {
        let entries = std::fs::read_dir(&self.directory).map_err(|e| e.to_string())?;
        let mut files = Vec::new();
        for entry in entries.take(1000) {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() || path.is_file() {
                files.push((path.clone(), path.is_dir()));
            }
        }
        files.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        self.files = files;
        self.scroll = 0;
        self.selected = 0;
        Ok(())
    }
    fn folder_action(
        &mut self,
        target: Target,
        layout: &Layout,
    ) -> Result<Option<Request>, String> {
        match target {
            Target::MoveEarlier | Target::MoveLater => {
                return Ok(Some(Request::Reorder(target == Target::MoveLater)));
            }
            Target::CreateFolder | Target::RenameFolder => {
                self.folder_edit = if target == Target::RenameFolder {
                    self.folder_context.clone()
                } else {
                    None
                };
                self.text = self
                    .folder_edit
                    .as_ref()
                    .and_then(|id| self.folders.names.get(id))
                    .cloned()
                    .unwrap_or_default();
                self.page(Page::Text(Field::FolderName));
                self.selected = self.targets(layout).len().saturating_sub(1);
            }
            Target::DeleteFolder => self.page(Page::FolderDelete),
            Target::MoveApp => self.page(Page::FolderMove),
            Target::Unfile | Target::Destination(_) => {
                let app = self.entry.as_ref().ok_or("No app selected")?;
                let folder = if let Target::Destination(index) = target {
                    Some(
                        self.folders
                            .names
                            .keys()
                            .nth(index)
                            .ok_or("Folder no longer exists")?
                            .clone(),
                    )
                } else {
                    None
                };
                return Ok(Some(Request::Folder(crate::folders::Change::Move(
                    app.id.clone(),
                    folder,
                ))));
            }
            _ => return Err("Invalid folder action".into()),
        }
        Ok(None)
    }
    fn activate(&mut self, target: Target, layout: &Layout) -> Result<Option<Request>, String> {
        match target {
            Target::MoveEarlier | Target::MoveLater => {
                return Ok(Some(Request::Reorder(target == Target::MoveLater)));
            }
            Target::CreateFolder
            | Target::RenameFolder
            | Target::DeleteFolder
            | Target::MoveApp
            | Target::Unfile
            | Target::Destination(_) => return self.folder_action(target, layout),
            Target::Add => self.add(),
            Target::Edit => {
                self.draft = Store::current()?
                    .load(&self.entry.as_ref().ok_or("No shortcut selected")?.id)?;
                self.page(Page::Editor);
            }
            Target::Remove => self.page(Page::Remove),
            Target::Uninstall => return Ok(Some(Request::Uninstall)),
            Target::Save => return Ok(Some(Request::Save)),
            Target::Confirm if self.page == Page::FolderDelete => {
                return Ok(Some(Request::Folder(crate::folders::Change::Delete(
                    self.folder_context.clone().ok_or("No folder selected")?,
                ))));
            }
            Target::Confirm => return Ok(Some(Request::Remove)),
            Target::Cancel => match self.page {
                Page::Text(Field::FolderName) | Page::FolderMove | Page::FolderDelete => {
                    self.page(Page::Menu);
                }
                Page::Text(_) | Page::Browse(_) => self.page(Page::Editor),
                Page::Remove => self.page(Page::Menu),
                _ => {
                    self.open = false;
                    self.contact = None;
                }
            },
            Target::Field(field) => {
                self.text = match field {
                    Field::Name => &self.draft.name,
                    Field::Command => &self.draft.command,
                    Field::Cwd => &self.draft.cwd,
                    Field::FolderName => return Err("Use folder actions to edit names".into()),
                }
                .clone();
                self.page(Page::Text(field));
                self.selected = self.targets(layout).len().saturating_sub(1);
            }
            Target::Done => return self.finish_text(),
            Target::Character(c) => self.insert(&c.to_string()),
            Target::Delete => {
                self.text.pop();
            }
            Target::Clear => self.text.clear(),
            Target::Shift => self.keyboard_page = self.keyboard_page.next(),
            Target::Terminal => self.draft.terminal = !self.draft.terminal,
            Target::Shell => {
                self.draft.mode = if self.draft.mode == Mode::Shell {
                    Mode::Direct
                } else {
                    Mode::Shell
                }
            }
            Target::DefaultIcon => self.draft.icon = None,
            Target::Icon => self.browse(true)?,
            Target::Previous => self.scroll = self.scroll.saturating_sub(Self::capacity(layout)),
            Target::Next => {
                self.scroll =
                    (self.scroll + Self::capacity(layout)).min(self.rows().len().saturating_sub(1));
            }
            Target::Parent => {
                if let Some(parent) = self.directory.parent() {
                    self.directory = parent.into();
                    self.read_directory()?;
                }
            }
            Target::Item(index) => {
                if self.page == Page::Editor {
                    self.browse(false)?;
                } else if let Some((path, directory)) = self.files.get(index).cloned() {
                    if directory {
                        self.directory = path;
                        self.read_directory()?;
                    } else {
                        if self.page == Page::Browse(true) {
                            self.draft.choose_icon(&path)?;
                        } else {
                            self.draft.command = super::command::quote(&path)?;
                        }
                        self.page(Page::Editor);
                    }
                }
            }
        }
        Ok(None)
    }
    fn finish_text(&mut self) -> Result<Option<Request>, String> {
        if let Page::Text(field) = self.page {
            if field == Field::FolderName {
                return Ok(Some(Request::Folder(match self.folder_edit.clone() {
                    Some(id) => crate::folders::Change::Rename(id, self.text.clone()),
                    None => crate::folders::Change::Create(self.text.clone()),
                })));
            }
            let text = std::mem::take(&mut self.text);
            match field {
                Field::Name => self.draft.name = text,
                Field::Command => self.draft.command = text,
                Field::Cwd => self.draft.cwd = text,
                Field::FolderName => return Err("Folder field already handled".into()),
            }
            self.page(Page::Editor);
        }
        Ok(None)
    }
    fn insert(&mut self, text: &str) {
        let limit = match self.page {
            Page::Text(Field::Name | Field::FolderName) => 100,
            Page::Text(Field::Cwd) => 4096,
            _ => 8192,
        };
        if self.text.len() + text.len() <= limit && !text.chars().any(char::is_control) {
            self.text.push_str(text);
        }
    }
    fn keyboard(
        &mut self,
        key: Keycode,
        keymod: Mod,
        targets: &[(Target, String, Rect)],
    ) -> Option<Target> {
        let mut activate = None;
        self.contact = None;
        match key {
            Keycode::Escape | Keycode::Home => activate = Some(Target::Cancel),
            Keycode::Space if self.editing() => (),
            Keycode::Return | Keycode::KpEnter | Keycode::Space => {
                activate = targets.get(self.selected).map(|(t, _, _)| *t);
            }
            Keycode::Tab if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) => {
                self.selected = self
                    .selected
                    .checked_sub(1)
                    .unwrap_or_else(|| targets.len().saturating_sub(1));
            }
            Keycode::Tab | Keycode::Down | Keycode::Right => {
                self.selected = (self.selected + 1) % targets.len().max(1);
            }
            Keycode::Up | Keycode::Left => {
                self.selected = self
                    .selected
                    .checked_sub(1)
                    .unwrap_or_else(|| targets.len().saturating_sub(1));
            }
            Keycode::PageUp => activate = Some(Target::Previous),
            Keycode::PageDown => activate = Some(Target::Next),
            Keycode::Backspace if self.editing() => activate = Some(Target::Delete),
            Keycode::A if self.editing() && keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                activate = Some(Target::Clear);
            }
            _ => (),
        }
        activate
    }
    pub fn event(&mut self, event: &Event, layout: &Layout) -> Option<Request> {
        let targets = self.targets(layout);
        self.selected = self.selected.min(targets.len().saturating_sub(1));
        let hit = |x, y| {
            targets
                .iter()
                .find(|(_, _, rect)| rect.contains(x, y))
                .map(|(target, _, _)| *target)
        };
        let mut activate = None;
        match event {
            Event::KeyDown {
                keycode: Some(key),
                keymod,
                repeat: false,
                ..
            } => {
                activate = self.keyboard(*key, *keymod, &targets);
            }
            Event::TextInput { text, .. } if self.editing() => self.insert(text),
            Event::MouseWheel { y, .. } if *y != 0 => {
                activate = Some(if *y > 0 {
                    Target::Previous
                } else {
                    Target::Next
                });
            }
            Event::MouseButtonDown {
                which,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if *which != u32::MAX => {
                self.contact = hit(f64::from(*x), f64::from(*y)).map(|t| (i64::from(*which), 0, t));
            }
            Event::MouseButtonUp {
                which,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if *which != u32::MAX => {
                let target = hit(f64::from(*x), f64::from(*y));
                if self
                    .contact
                    .take()
                    .is_some_and(|(id, _, t)| id == i64::from(*which) && target == Some(t))
                {
                    activate = target;
                }
            }
            Event::FingerDown {
                touch_id,
                finger_id,
                x,
                y,
                ..
            } => {
                self.contact = if self.contact.is_some() {
                    None
                } else {
                    hit(
                        f64::from(*x) * f64::from(layout.width),
                        f64::from(*y) * f64::from(layout.height),
                    )
                    .map(|t| (*touch_id, *finger_id, t))
                };
            }
            Event::FingerUp {
                touch_id,
                finger_id,
                x,
                y,
                ..
            } => {
                let target = hit(
                    f64::from(*x) * f64::from(layout.width),
                    f64::from(*y) * f64::from(layout.height),
                );
                if self.contact.take().is_some_and(|(id, finger, t)| {
                    id == *touch_id && finger == *finger_id && target == Some(t)
                }) {
                    activate = target;
                }
            }
            Event::Window {
                win_event: WindowEvent::FocusLost,
                ..
            } => {
                self.contact = None;
                if matches!(self.page, Page::Remove | Page::FolderDelete) {
                    self.page(Page::Menu);
                }
            }
            _ => (),
        }
        let pointer = !matches!(event, Event::KeyDown { .. });
        activate.and_then(|target| self.dispatch(target, layout, pointer))
    }
    fn dispatch(&mut self, target: Target, layout: &Layout, pointer: bool) -> Option<Request> {
        let targets = self.targets(layout);

        if pointer && let Some(index) = targets.iter().position(|(t, _, _)| *t == target) {
            self.selected = index;
        }
        match self.activate(target, layout) {
            Ok(request) => {
                // Variable-length pages must not move focus from Down to Save,
                // or from the keyboard page switch onto a character key.
                if matches!(
                    target,
                    Target::Previous | Target::Next | Target::Shift | Target::Parent
                ) && let Some(index) = self
                    .targets(layout)
                    .iter()
                    .position(|(item, _, _)| *item == target)
                {
                    self.selected = index;
                }
                return request;
            }
            Err(error) => self.error = error,
        }
        None
    }
}
