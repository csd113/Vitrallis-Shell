//! One target model for visible buttons, keyboard focus, touch and mouse activation.
use super::{Command, Row, Sources, Update, Worker};
use crate::layout::{Layout, Rect};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
    mouse::MouseButton,
};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Apps,
    Sources,
    Edit,
    Details,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Check,
    Install,
    Sources,
    Shell,
    Home,
    Row(usize),
    Previous,
    Next,
    Add,
    Edit,
    Remove,
    Save,
    Cancel,
    Character(char),
    Delete,
    Clear,
    Details,
    Approve,
    Confirm(bool),
}
#[derive(Debug)]
enum Confirmation {
    Running(u64, String),
    Publisher(usize),
    Trust(usize),
    Remove(usize),
}
#[derive(Debug)]
pub enum Destination {
    ShellUpdates,
}
#[derive(Debug)]
pub struct Center {
    pub open: bool,
    pub message: String,
    pub busy: bool,
    download_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub refresh: bool,
    pub request: Option<Destination>,
    pub selected: usize,
    pub text: String,
    page: Page,
    start: usize,
    row: usize,
    edit: Option<usize>,
    checked: BTreeSet<String>,
    sources: Sources,
    rows: Vec<Row>,
    worker: Option<Worker>,
    confirmation: Option<Confirmation>,
    contact: Option<(i64, i64, Target)>,
}
impl Default for Center {
    fn default() -> Self {
        Self {
            open: false,
            message: "Check for updates to load catalogs".into(),
            busy: false,
            download_cancel: None,
            refresh: false,
            request: None,
            selected: 0,
            text: String::new(),
            page: Page::Apps,
            start: 0,
            row: 0,
            edit: None,
            checked: BTreeSet::new(),
            sources: Sources::default(),
            rows: Vec::new(),
            worker: None,
            confirmation: None,
            contact: None,
        }
    }
}
impl Center {
    pub fn show(&mut self) {
        self.open = true;
        if self.worker.is_none() {
            match Worker::start() {
                Ok(w) => {
                    self.worker = Some(w);
                    self.busy = true;
                }
                Err(e) => self.message = e,
            }
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Some(update) = self.worker.as_ref().and_then(|w| w.receive.try_recv().ok()) {
            changed = true;
            match update {
                Update::Sources(s) => self.sources = s,
                Update::Rows(rows) => {
                    self.refresh = true;
                    self.rows = rows;
                    self.checked.clear();
                    self.start = 0;
                    self.row = self.row.min(self.rows.len().saturating_sub(1));
                    self.selected = 0;
                }
                Update::Progress(s) => self.message = s,
                Update::Confirm(id, s) if !self.open => {
                    self.send(Command::Answer(id, false));
                    self.message = s;
                }
                Update::Confirm(id, s) => {
                    self.confirmation = Some(Confirmation::Running(id, s));
                    self.selected = 0;
                    self.contact = None;
                }
                Update::Done(result, changed) => {
                    self.busy = false;
                    self.download_cancel = None;
                    self.confirmation = None;
                    self.refresh |= changed;
                    self.message = result.map_or_else(
                        |e| e,
                        |()| {
                            if changed {
                                "Installed; apps remain closed. Check again for current status"
                                    .into()
                            } else {
                                "Ready. Selections are unchecked by default".into()
                            }
                        },
                    );
                }
            }
        }
        changed
    }
    fn send(&mut self, command: Command) {
        if let Some(w) = &self.worker {
            if matches!(command, Command::Install(_)) {
                self.download_cancel = Some(std::sync::Arc::clone(&w.cancelled));
            }
            if matches!(command, Command::Check | Command::Install(_)) {
                w.cancelled
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }
            match w.send.send(command) {
                Ok(()) => {
                    self.busy = true;
                    self.message = "Working...".into();
                }
                Err(e) => self.message = e.to_string(),
            }
        }
    }
    pub fn lost_focus(&mut self) {
        self.contact = None;
        if let Some(confirmation) = self.confirmation.take() {
            if let Confirmation::Running(id, _) = confirmation {
                self.send(Command::Answer(id, false));
            }
            self.selected = 0;
        }
    }
    pub const fn editing(&self) -> bool {
        matches!(self.page, Page::Edit) && self.open
    }
    pub const fn title(&self) -> &str {
        match self.page {
            Page::Apps => "APP CENTER",
            Page::Sources => "GITHUB CATALOGS",
            Page::Edit => "EDIT REPOSITORIES",
            Page::Details => "APP / SOURCE DETAILS",
        }
    }
    pub fn details(&self) -> String {
        if let Some(c) = &self.confirmation {
            return match c {
            Confirmation::Running(_, s)=>s.clone(),
            Confirmation::Publisher(i)=>self.rows.get(*i).map_or_else(String::new,|r|format!("Duplicate app ID: {}. Explicitly select publisher {}?",r.package.id,r.package.origin.as_str())),
            Confirmation::Trust(i)=>self.rows.get(*i).map_or_else(String::new,|r|format!("Trust {} to supply executable app files for catalog {}? Apps are not sandboxed. Check again after approval.",r.package.repository.as_str(),r.package.origin.as_str())),
            Confirmation::Remove(i)=>format!("Remove {}? Installed apps and saves remain.",self.sources.catalogs[*i].as_str()),
        };
        }
        if self.busy {
            return self.message.clone();
        }
        if self.page == Page::Edit {
            return format!(
                "owner/repo or HTTPS URL; use ; for batch entry\n{}",
                self.text
            );
        }
        if self.page == Page::Details {
            return self.rows.get(self.row).map_or_else(||self.message.clone(),|r|format!("{}\nID: {}\nCatalog: {}\nSource: {}\nInstalled: {} Latest: {}\nDownload: {} bytes\n{}\n{}\nRequirements: {}. Apps are not sandboxed.",r.package.name,r.package.id,r.package.origin.as_str(),r.package.repository.as_str(),r.installed,r.package.version,r.download_size,r.status,r.package.notes,r.package.permissions));
        }
        if self.page == Page::Sources {
            return "Default catalog is always included. Customs supplement it.\nRemoving a source never uninstalls apps.".into();
        }
        self.rows.get(self.row).map_or_else(
            || self.message.clone(),
            |r| format!("{} | {}", r.package.origin.as_str(), r.status),
        )
    }
    pub fn lines(&self, width: usize) -> Vec<String> {
        let width = width.max(1);
        let mut lines = Vec::new();
        for line in self.details().lines() {
            let mut current = String::new();
            for word in line.split_whitespace() {
                if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width
                {
                    lines.push(std::mem::take(&mut current));
                }
                for c in word.chars() {
                    if current.chars().count() == width {
                        lines.push(std::mem::take(&mut current));
                    }
                    current.push(c);
                }
                if current.chars().count() < width {
                    current.push(' ');
                }
            }
            lines.push(current.trim_end().into());
        }
        // Keep the insertion end visible while entering a long batch.
        if self.editing() && lines.len() > 3 {
            lines.drain(1..lines.len() - 2);
        }
        lines
    }
    pub fn targets(&self, layout: &Layout) -> Vec<(Target, String, Rect)> {
        let width = i32::from(layout.width);
        let height = i32::from(layout.height);
        let mut out = Vec::new();
        if self.confirmation.is_some() {
            for (i, (target, label)) in [
                (Target::Confirm(false), "Cancel"),
                (
                    Target::Confirm(true),
                    if matches!(self.confirmation, Some(Confirmation::Running(_, _))) {
                        "Close and update"
                    } else {
                        "Confirm"
                    },
                ),
            ]
            .into_iter()
            .enumerate()
            {
                out.push((
                    target,
                    label.into(),
                    Rect {
                        x: 12 + i32::try_from(i).unwrap_or(0) * (width / 2),
                        y: height - 48,
                        w: width / 2 - 24,
                        h: 36,
                    },
                ));
            }
            return out;
        }
        let menu = match self.page {
            Page::Apps => vec![
                (Target::Check, "Check"),
                if self.busy && self.download_cancel.is_some() {
                    (Target::Cancel, "Cancel")
                } else {
                    (Target::Install, "Install")
                },
                (Target::Sources, "Sources"),
                (Target::Shell, "Shell"),
                (Target::Home, "Home"),
            ],
            Page::Sources => vec![
                (Target::Add, "Add"),
                (Target::Edit, "Edit"),
                (Target::Remove, "Remove"),
                (Target::Cancel, "Back"),
            ],
            Page::Edit => vec![
                (Target::Save, "Save"),
                (Target::Clear, "Clear"),
                (Target::Delete, "Delete"),
                (Target::Character(';'), "; batch"),
                (Target::Cancel, "Cancel"),
            ],
            Page::Details => vec![(Target::Approve, "Trust source"), (Target::Cancel, "Back")],
        };
        let count = i32::try_from(menu.len()).unwrap_or(1);
        for (i, (target, label)) in menu.into_iter().enumerate() {
            out.push((
                target,
                label.into(),
                Rect {
                    x: 4 + i32::try_from(i).unwrap_or(0) * (width / count),
                    y: 30,
                    w: width / count - 8,
                    h: 30,
                },
            ));
        }
        self.content_targets(layout, &mut out);
        out
    }
    fn content_targets(&self, layout: &Layout, out: &mut Vec<(Target, String, Rect)>) {
        let width = i32::from(layout.width);
        let height = i32::from(layout.height);
        if self.page == Page::Edit {
            for (i, c) in "abcdefghijklmnopqrstuvwxyz0123456789-_/.:"
                .chars()
                .enumerate()
            {
                out.push((
                    Target::Character(c),
                    c.to_string(),
                    Rect {
                        x: 8 + i32::try_from(i % 10).unwrap_or(0) * ((width - 16) / 10),
                        y: 108 + i32::try_from(i / 10).unwrap_or(0) * 28,
                        w: (width - 16) / 10 - 3,
                        h: 25,
                    },
                ));
            }
            return;
        }
        if self.page != Page::Details {
            let count = if self.page == Page::Apps {
                self.rows.len()
            } else {
                self.sources.catalogs.len()
            };
            for i in self.start..(self.start + Self::capacity(layout)).min(count) {
                let label = if self.page == Page::Apps {
                    let row = &self.rows[i];
                    let selected = self.checked.contains(&row.package.key());
                    format!(
                        "[{}] {} | {} > {}",
                        if selected {
                            "X"
                        } else if self
                            .rows
                            .iter()
                            .filter(|r| r.package.id == row.package.id)
                            .count()
                            > 1
                        {
                            "!"
                        } else {
                            " "
                        },
                        row.package.name,
                        row.installed,
                        row.package.version
                    )
                } else {
                    format!(
                        "{}{}",
                        if i == 0 { "* " } else { "" },
                        self.sources.catalogs[i].as_str()
                    )
                };
                out.push((
                    Target::Row(i),
                    label,
                    Rect {
                        x: 8,
                        y: 68 + i32::try_from(i - self.start).unwrap_or(0) * 38,
                        w: width - 16,
                        h: 34,
                    },
                ));
            }
        }
        for (target, label, x) in [
            (Target::Previous, "Previous", 8),
            (Target::Next, "Next", width - 96),
        ] {
            out.push((
                target,
                label.into(),
                Rect {
                    x,
                    y: height - 44,
                    w: 88,
                    h: 30,
                },
            ));
        }
        if self.page == Page::Apps {
            out.push((
                Target::Details,
                "Details".into(),
                Rect {
                    x: width / 2 - 44,
                    y: height - 44,
                    w: 88,
                    h: 30,
                },
            ));
        }
    }
    fn capacity(layout: &Layout) -> usize {
        usize::from(layout.height.saturating_sub(150) / 38).max(1)
    }
    pub fn enabled(&self, target: &Target) -> bool {
        if *target == Target::Home {
            return true;
        }
        if self.confirmation.is_some() {
            return matches!(target, Target::Confirm(_));
        }
        if self.busy {
            return *target == Target::Cancel && self.download_cancel.is_some();
        }
        match target {
            Target::Install => !self.checked.is_empty(),
            Target::Edit | Target::Remove => self.row > 0 && self.row < self.sources.catalogs.len(),
            Target::Approve => self.rows.get(self.row).is_some_and(|r| {
                !self
                    .sources
                    .trusted(&r.package.origin, &r.package.repository)
            }),
            _ => true,
        }
    }
    pub fn event(&mut self, event: &Event, layout: &Layout) {
        if let Event::Window {
            win_event: WindowEvent::FocusLost,
            ..
        } = event
        {
            self.lost_focus();
            return;
        }
        if matches!(
            event,
            Event::KeyDown {
                keycode: Some(Keycode::Backspace),
                repeat: true,
                ..
            }
        ) && self.editing()
            && !self.busy
        {
            self.text.pop();
            return;
        }
        if let Event::TextInput { text, .. } = event {
            if self.editing() && !self.busy {
                self.append(text);
            }
            return;
        }
        let targets = self.targets(layout);
        if matches!(event, Event::KeyDown { .. }) {
            self.keyboard(event, layout, &targets);
            return;
        }
        if let Event::MouseWheel { y, .. } = event {
            if *y != 0 {
                self.activate(
                    if *y > 0 {
                        Target::Previous
                    } else {
                        Target::Next
                    },
                    layout,
                );
            }
            return;
        }
        self.pointer_event(event, layout, &targets);
    }
    fn pointer_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        targets: &[(Target, String, Rect)],
    ) {
        let contact = match *event {
            Event::MouseButtonDown {
                which,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if which != u32::MAX => {
                Some((i64::from(which), -1, true, f64::from(x), f64::from(y)))
            }
            Event::MouseButtonUp {
                which,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if which != u32::MAX => {
                Some((i64::from(which), -1, false, f64::from(x), f64::from(y)))
            }
            Event::FingerDown {
                touch_id,
                finger_id,
                x,
                y,
                ..
            } => Some((
                touch_id,
                finger_id,
                true,
                f64::from(x) * f64::from(layout.width),
                f64::from(y) * f64::from(layout.height),
            )),
            Event::FingerUp {
                touch_id,
                finger_id,
                x,
                y,
                ..
            } => Some((
                touch_id,
                finger_id,
                false,
                f64::from(x) * f64::from(layout.width),
                f64::from(y) * f64::from(layout.height),
            )),
            _ => None,
        };
        if let Some((id, finger, down, x, y)) = contact {
            let hit = targets.iter().position(|(_, _, r)| r.contains(x, y));
            if down {
                self.contact = if self.contact.is_none() {
                    hit.map(|i| (id, finger, targets[i].0))
                } else {
                    None
                };
            } else {
                let old = self.contact.take();
                if let Some(i) = hit
                    && old == Some((id, finger, targets[i].0))
                {
                    self.selected = i;
                    self.activate(targets[i].0, layout);
                }
            }
        }
    }
    fn keyboard(&mut self, event: &Event, layout: &Layout, targets: &[(Target, String, Rect)]) {
        if let Event::KeyDown {
            keycode: Some(key),
            repeat: false,
            ..
        } = event
        {
            self.contact = None;
            match *key {
                Keycode::Home => self.activate(Target::Home, layout),
                Keycode::Escape => self.activate(Target::Cancel, layout),
                Keycode::Up | Keycode::Kp8 | Keycode::Left | Keycode::Kp4 => {
                    self.selected = self
                        .selected
                        .checked_sub(1)
                        .unwrap_or_else(|| targets.len().saturating_sub(1));
                }
                Keycode::Down | Keycode::Kp2 | Keycode::Right | Keycode::Kp6 | Keycode::Tab => {
                    self.selected = (self.selected + 1) % targets.len().max(1);
                }
                Keycode::Space if self.editing() => (),
                Keycode::Return | Keycode::KpEnter | Keycode::Space => {
                    if let Some((t, _, _)) = targets.get(self.selected) {
                        self.activate(*t, layout);
                    }
                }
                Keycode::PageDown => self.activate(Target::Next, layout),
                Keycode::PageUp => self.activate(Target::Previous, layout),
                Keycode::Backspace if self.editing() => {
                    self.text.pop();
                }
                Keycode::C if !self.editing() => self.activate(Target::Check, layout),
                Keycode::I if !self.editing() => self.activate(Target::Install, layout),
                _ => (),
            }
            if let Some((Target::Row(i), _, _)) = targets.get(self.selected) {
                self.row = *i;
            }
        }
    }
    fn append(&mut self, text: &str) {
        if self.text.len() + text.len() <= 8192
            && text
                .chars()
                .all(|c| !c.is_control() || c == '\n' || c == '\t')
        {
            self.text.push_str(text);
        }
    }
    const fn page(&mut self, page: Page) {
        self.page = page;
        self.start = 0;
        self.selected = 0;
        self.contact = None;
    }
    fn cancel_download(&mut self) {
        if let Some(cancelled) = &self.download_cancel {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.message = "Cancelling download; any commit already started will finish safely".into();
    }
    fn check_catalogs(&mut self) {
        self.checked.clear();
        self.rows.clear();
        self.send(Command::Check);
    }
    fn activate(&mut self, target: Target, layout: &Layout) {
        if target == Target::Cancel && self.confirmation.is_some() {
            self.answer(false);
            return;
        }
        if !self.enabled(&target) {
            return;
        }
        if target == Target::Cancel && self.busy {
            self.cancel_download();
            return;
        }
        match target {
            Target::Check => self.check_catalogs(),
            Target::Install => self.send(Command::Install(self.checked.iter().cloned().collect())),
            Target::Sources => {
                self.row = 0;
                self.page(Page::Sources);
            }
            Target::Shell => {
                self.open = false;
                self.request = Some(Destination::ShellUpdates);
            }
            Target::Home => {
                self.lost_focus();
                self.open = false;
            }
            Target::Cancel => match self.page {
                Page::Apps => self.open = false,
                Page::Edit => self.page(Page::Sources),
                _ => self.page(Page::Apps),
            },
            Target::Row(index) => {
                self.row = index;
                if self.page == Page::Apps {
                    self.toggle(index);
                }
            }
            Target::Add => {
                self.edit = None;
                self.text.clear();
                self.page(Page::Edit);
            }
            Target::Edit => {
                self.edit = Some(self.row);
                self.text = self.sources.catalogs[self.row].as_str().into();
                self.page(Page::Edit);
            }
            Target::Remove => {
                self.confirmation = Some(Confirmation::Remove(self.row));
                self.selected = 0;
            }
            Target::Save => {
                let mut next = self.sources.clone();
                match next.edit(self.edit, &self.text) {
                    Ok(()) => {
                        self.send(Command::Save(next));
                        self.page(Page::Sources);
                    }
                    Err(e) => self.message = e,
                }
            }
            Target::Character(c) => self.append(&c.to_string()),
            Target::Delete => {
                self.text.pop();
            }
            Target::Clear => self.text.clear(),
            Target::Details => self.page(Page::Details),
            Target::Approve => {
                self.confirmation = Some(Confirmation::Trust(self.row));
                self.selected = 0;
            }
            Target::Confirm(yes) => self.answer(yes),
            Target::Previous => {
                self.start = self.start.saturating_sub(if self.page == Page::Details {
                    8
                } else {
                    Self::capacity(layout)
                });
                self.selected = 0;
            }
            Target::Next => {
                let count = match self.page {
                    Page::Apps => self.rows.len(),
                    Page::Sources => self.sources.catalogs.len(),
                    Page::Details => self.lines(usize::from(layout.width) / 8 - 2).len(),
                    Page::Edit => 0,
                };
                let step = if self.page == Page::Details {
                    8
                } else {
                    Self::capacity(layout)
                };
                if self.start + step < count {
                    self.start += step;
                }
                self.selected = 0;
            }
        }
    }
    fn toggle(&mut self, index: usize) {
        let row = &self.rows[index];
        let key = row.package.key();
        if self.checked.remove(&key) {
            return;
        }
        if !row.ready {
            self.message = row.status.clone();
            return;
        }
        if self
            .rows
            .iter()
            .filter(|r| r.package.id == row.package.id)
            .count()
            > 1
        {
            self.confirmation = Some(Confirmation::Publisher(index));
            self.selected = 0;
        } else {
            self.checked.insert(key);
        }
    }
    fn answer(&mut self, yes: bool) {
        if let Some(c) = self.confirmation.take() {
            match c {
                Confirmation::Running(id, _) => self.send(Command::Answer(id, yes)),
                Confirmation::Publisher(i) if yes => {
                    let row = &self.rows[i];
                    for other in &self.rows {
                        if other.package.id == row.package.id {
                            self.checked.remove(&other.package.key());
                        }
                    }
                    self.checked.insert(row.package.key());
                }
                Confirmation::Trust(i) if yes => {
                    let r = &self.rows[i];
                    let mut next = self.sources.clone();
                    next.approvals
                        .insert((r.package.origin.clone(), r.package.repository.clone()));
                    self.send(Command::Save(next));
                }
                Confirmation::Remove(i) if yes => {
                    let mut next = self.sources.clone();
                    next.remove(i);
                    self.row = 0;
                    self.send(Command::Save(next));
                }
                _ => (),
            }
        }
        self.selected = 0;
    }
    pub const fn detail_start(&self) -> usize {
        if matches!(self.page, Page::Details) {
            self.start
        } else {
            0
        }
    }
    pub const fn full_details(&self) -> bool {
        self.confirmation.is_some() || matches!(self.page, Page::Details)
    }
}

#[cfg(test)]
impl Center {
    fn fixture() -> Result<Self, String> {
        let origin = super::sources::Repository::parse(super::sources::DEFAULT)?;
        let bytes = include_bytes!("../../tests/fixtures/app-center/catalog.json");
        let p = super::metadata::catalog(&origin, bytes)?.remove(0);
        let mut center = Self {
            open: true,
            ..Self::default()
        };
        for i in 0..9 {
            let mut package = p.clone();
            package.name = format!("App {i}");
            package.id = format!("org.example.app{i}");
            center.rows.push(Row {
                package,
                installed: "not installed".into(),
                status: "ready".into(),
                ready: true,
                download_size: 12345,
            });
        }
        Ok(center)
    }

    pub fn qa_samples() -> Result<Vec<(&'static str, Self)>, String> {
        let mut out = Vec::new();
        for (name, page) in [
            ("apps", Page::Apps),
            ("sources", Page::Sources),
            ("editor", Page::Edit),
            ("details", Page::Details),
        ] {
            let mut center = Self::fixture()?;
            center.page(page);
            center.text = "example/catalog;https://github.com/my/catalog".into();
            out.push((name, center));
        }
        let mut center = Self::fixture()?;
        center.confirmation = Some(Confirmation::Running(
            1,
            "Close and update? Unsaved work may be lost. App: Example app".into(),
        ));
        out.push(("confirm", center));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(key: Keycode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 1,
            keycode: Some(key),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        }
    }
    fn center() -> Result<Center, String> {
        Center::fixture()
    }
    #[test]
    fn download_cancel_is_visible_and_shared_by_keyboard_and_touch() -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            let layout = Layout::home(w, h)?;
            for touch in [false, true] {
                let mut center = center()?;
                let (send, _commands) = std::sync::mpsc::channel();
                let (_updates, receive) = std::sync::mpsc::channel();
                let cancelled = std::sync::Arc::default();
                center.worker = Some(Worker {
                    send,
                    receive,
                    cancelled: std::sync::Arc::clone(&cancelled),
                });
                center.send(Command::Install(vec![center.rows[0].package.key()]));
                center.message = "Downloading: 16384 / 40000 bytes (40%)\nSelected app".into();
                assert!(center.details().starts_with("Downloading:"));
                let (index, (_, label, bounds)) = center
                    .targets(&layout)
                    .into_iter()
                    .enumerate()
                    .find(|(_, (t, _, _))| *t == Target::Cancel)
                    .ok_or("Cancel target")?;
                assert_eq!(label, "Cancel");
                assert!(center.enabled(&Target::Cancel));
                for _ in 0..index {
                    center.event(&key(Keycode::Tab), &layout);
                }
                assert_eq!(center.selected, index);
                if touch {
                    let x = f32::from(
                        u16::try_from(bounds.x + bounds.w / 2).map_err(|e| e.to_string())?,
                    ) / f32::from(w);
                    let y = f32::from(
                        u16::try_from(bounds.y + bounds.h / 2).map_err(|e| e.to_string())?,
                    ) / f32::from(h);
                    center.event(
                        &Event::FingerDown {
                            timestamp: 0,
                            touch_id: 1,
                            finger_id: 1,
                            x,
                            y,
                            dx: 0.,
                            dy: 0.,
                            pressure: 1.,
                        },
                        &layout,
                    );
                    center.event(
                        &Event::FingerUp {
                            timestamp: 0,
                            touch_id: 1,
                            finger_id: 1,
                            x,
                            y,
                            dx: 0.,
                            dy: 0.,
                            pressure: 0.,
                        },
                        &layout,
                    );
                } else {
                    center.event(&key(Keycode::Return), &layout);
                }
                assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
                assert!(center.busy);
                assert!(center.open);
            }
        }
        Ok(())
    }

    #[test]
    fn home_leaves_every_page_even_when_busy_and_cancels_confirmation() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        for page in [Page::Apps, Page::Sources, Page::Edit, Page::Details] {
            for busy in [false, true] {
                let mut center = center()?;
                center.page(page);
                center.busy = busy;
                center.confirmation = Some(Confirmation::Publisher(0));
                center.event(&key(Keycode::Home), &layout);
                assert!(!center.open);
                assert!(center.confirmation.is_none());
                assert!(center.checked.is_empty());
                assert_eq!(center.busy, busy);
            }
        }
        let mut center = center()?;
        center.busy = true;
        assert!(center.enabled(&Target::Home));
        center.activate(Target::Home, &layout);
        assert!(!center.open);
        assert!(center.busy);
        Ok(())
    }

    #[test]
    fn leaving_declines_running_app_prompts_including_late_worker_requests() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let (send, commands) = std::sync::mpsc::channel();
        let (updates, receive) = std::sync::mpsc::channel();
        let mut center = center()?;
        center.worker = Some(Worker {
            send,
            receive,
            cancelled: std::sync::Arc::default(),
        });
        center.busy = true;
        center.confirmation = Some(Confirmation::Running(1, "Close app?".into()));
        center.event(&key(Keycode::Home), &layout);
        assert!(matches!(commands.try_recv(), Ok(Command::Answer(1, false))));
        updates
            .send(Update::Confirm(2, "Close another app?".into()))
            .map_err(|e| e.to_string())?;
        assert!(center.poll());
        assert!(matches!(commands.try_recv(), Ok(Command::Answer(2, false))));
        assert!(!center.open);
        assert!(center.confirmation.is_none());
        updates
            .send(Update::Done(Ok(()), true))
            .map_err(|e| e.to_string())?;
        assert!(center.poll());
        assert!(!center.busy);
        assert!(center.refresh);
        Ok(())
    }

    #[test]
    fn every_visible_control_has_keyboard_focus_at_both_sizes() -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            let layout = Layout::home(w, h)?;
            let mut center = center()?;
            for page in [Page::Apps, Page::Sources, Page::Edit, Page::Details] {
                center.page(page);
                let targets = center.targets(&layout);
                for (i, (_, _, r)) in targets.iter().enumerate() {
                    assert!(
                        r.x >= 0
                            && r.y >= 0
                            && r.x + r.w <= i32::from(w)
                            && r.y + r.h <= i32::from(h)
                    );
                    assert_eq!(center.selected, i);
                    center.event(&key(Keycode::Tab), &layout);
                }
                assert_eq!(center.selected, 0);
            }
            center.page(Page::Apps);
            center.event(&key(Keycode::PageDown), &layout);
            assert!(center.start > 0);
            center.event(&key(Keycode::PageUp), &layout);
            assert_eq!(center.start, 0);
            assert!(center.checked.is_empty());
        }
        Ok(())
    }
    #[test]
    fn touch_keyboard_text_entry_and_cancel_defaults_share_actions() -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            let layout = Layout::home(w, h)?;
            let mut center = center()?;
            center.page(Page::Edit);
            center.event(
                &Event::TextInput {
                    timestamp: 0,
                    window_id: 1,
                    text: "example/one;example/two".into(),
                },
                &layout,
            );
            assert_eq!(Sources::batch(&center.text)?.len(), 2);
            center.event(&key(Keycode::Backspace), &layout);
            assert!(center.text.ends_with("tw"));
            let (i, (_, _, r)) = center
                .targets(&layout)
                .into_iter()
                .enumerate()
                .find(|(_, t)| t.0 == Target::Character('o'))
                .ok_or("key")?;
            let x =
                f32::from(u16::try_from(r.x + r.w / 2).map_err(|e| e.to_string())?) / f32::from(w);
            let y =
                f32::from(u16::try_from(r.y + r.h / 2).map_err(|e| e.to_string())?) / f32::from(h);
            let up = Event::FingerUp {
                timestamp: 0,
                touch_id: 1,
                finger_id: 2,
                x,
                y,
                dx: 0.,
                dy: 0.,
                pressure: 0.,
            };
            center.event(&up, &layout);
            assert!(center.text.ends_with("tw"));
            center.event(
                &Event::FingerDown {
                    timestamp: 0,
                    touch_id: 1,
                    finger_id: 2,
                    x,
                    y,
                    dx: 0.,
                    dy: 0.,
                    pressure: 1.,
                },
                &layout,
            );
            center.event(&up, &layout);
            assert!(center.text.ends_with("two"));
            assert_eq!(center.selected, i);
            center.page(Page::Apps);
            center.confirmation = Some(Confirmation::Publisher(0));
            center.event(&key(Keycode::KpEnter), &layout);
            assert!(center.checked.is_empty());
            center.confirmation = Some(Confirmation::Publisher(0));
            center.event(&key(Keycode::Kp6), &layout);
            center.event(&key(Keycode::KpEnter), &layout);
            assert_eq!(center.checked.len(), 1);
            center.busy = true;
            center.event(&key(Keycode::I), &layout);
            center.event(&key(Keycode::Home), &layout);
            assert!(!center.open);
            assert!(center.busy);
        }
        Ok(())
    }
    #[test]
    fn conflicts_require_explicit_publisher_selection_and_busy_commands_are_guarded()
    -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut center = center()?;
        center.rows[1].package.id = center.rows[0].package.id.clone();
        center.rows[1].package.origin = super::super::sources::Repository::parse("other/catalog")?;
        center.toggle(0);
        assert!(center.checked.is_empty());
        assert!(matches!(
            center.confirmation,
            Some(Confirmation::Publisher(0))
        ));
        center.answer(true);
        assert_eq!(center.checked.len(), 1);
        center.toggle(1);
        center.answer(false);
        assert!(center.checked.contains(&center.rows[0].package.key()));
        center.toggle(1);
        center.answer(true);
        assert_eq!(center.checked.len(), 1);
        assert!(center.checked.contains(&center.rows[1].package.key()));
        let (send, receive) = std::sync::mpsc::channel();
        let (_updates, queue) = std::sync::mpsc::channel();
        center.worker = Some(Worker {
            send,
            receive: queue,
            cancelled: std::sync::Arc::default(),
        });
        center.activate(Target::Install, &layout);
        assert!(
            matches!(receive.try_recv(),Ok(Command::Install(keys)) if keys==vec![center.rows[1].package.key()])
        );
        center.activate(Target::Check, &layout);
        assert!(receive.try_recv().is_err());
        center.confirmation = Some(Confirmation::Running(7, "Close?".into()));
        center.selected = 0;
        center.event(&key(Keycode::Return), &layout);
        assert!(matches!(receive.try_recv(), Ok(Command::Answer(7, false))));
        Ok(())
    }
}
