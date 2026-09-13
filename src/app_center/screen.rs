//! One target model for visible buttons, keyboard focus, touch and mouse activation.
use super::{Command, Row, Sources, Update, Worker};
use crate::layout::{Layout, Rect};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
    mouse::MouseButton,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Apps,
    Sources,
    Edit,
    Details,
    Changelog,
    Search,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Installed,
    Updates,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Check,
    Install,
    Sources,
    Home,
    Row(usize),
    Previous,
    Next,
    Add,
    Edit,
    Remove,
    Save,
    Cancel,
    CancelOperation,
    Character(char),
    Delete,
    Clear,
    Details,
    Uninstall,
    Approve,
    Search,
    Filter,
    Changelog,
    Open,
    Confirm(bool),
}
#[derive(Debug)]
enum Confirmation {
    Running(u64, String),
    Publisher(usize),
    Trust(usize),
    Remove(usize),
    Uninstall(usize),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readout {
    Status,
    Selection,
}
#[derive(Debug)]
pub struct Center {
    pub open: bool,
    pub message: String,
    pub busy: bool,
    pub launch: Option<String>,
    operation: Option<String>,
    errors: std::collections::BTreeMap<String, String>,
    search: String,
    filter: Filter,
    app_start: usize,
    download_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub refresh: bool,
    readout: Readout,
    pub selected: usize,
    pub text: String,
    page: Page,
    start: usize,
    row: usize,
    edit: Option<usize>,
    chosen: Option<String>,
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
            message: "Refresh to load available apps".into(),
            busy: false,
            launch: None,
            operation: None,
            errors: std::collections::BTreeMap::new(),
            search: String::new(),
            filter: Filter::All,
            app_start: 0,
            download_cancel: None,
            refresh: false,
            readout: Readout::Status,
            selected: 0,
            text: String::new(),
            page: Page::Apps,
            start: 0,
            row: 0,
            edit: None,
            chosen: None,
            sources: Sources::default(),
            rows: Vec::new(),
            worker: None,
            confirmation: None,
            contact: None,
        }
    }
}
impl Center {
    pub fn uninstall_entry(&mut self, app: &crate::app::AppEntry) -> Result<(), String> {
        if app.source != crate::app::AppSource::AppCenter {
            return Err("Only managed desktop entries can request uninstall".into());
        }
        if self.busy {
            return Err("Wait for the current App Center operation".into());
        }
        if self.worker.is_none() {
            self.worker = Some(Worker::start()?);
        }
        self.open = true;
        self.confirmation = None;
        self.send(Command::SelectInstalled(app.id.clone()));
        Ok(())
    }

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
        } else if !self.busy {
            self.send(Command::Scan);
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        loop {
            let Some(worker) = &self.worker else {
                break;
            };
            let update = match worker.receive.try_recv() {
                Ok(update) => update,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.worker = None;
                    self.busy = false;
                    self.download_cancel = None;
                    self.confirmation = None;
                    self.message = "App service stopped. Reopen App Center to reconnect; cached apps are kept.".into();
                    return true;
                }
            };
            changed = true;
            match update {
                Update::SelectedInstalled(key) => {
                    if self.open
                        && let Some(index) =
                            self.rows.iter().position(|row| row.package.key() == key)
                    {
                        self.chosen = Some(key);
                        self.row = index;
                        self.page(Page::Details);
                        self.confirm_uninstall();
                    }
                }
                Update::Sources(s) => self.sources = s,
                Update::Rows(rows) => {
                    self.rows = rows;
                    if !self
                        .rows
                        .iter()
                        .any(|row| self.chosen.as_ref() == Some(&row.package.key()))
                    {
                        self.chosen = None;
                    }
                    self.confirmation = None;
                    self.contact = None;
                    self.row = self.row.min(self.rows.len().saturating_sub(1));
                    if let Some(index) = self
                        .rows
                        .iter()
                        .position(|row| self.chosen.as_ref() == Some(&row.package.key()))
                    {
                        self.row = index;
                    }
                    if self.page == Page::Apps {
                        self.start = self.start.min(self.rows.len().saturating_sub(1));
                    }
                }
                Update::Row(row) => {
                    if let Some(old) = self
                        .rows
                        .iter_mut()
                        .find(|r| r.package.key() == row.package.key())
                    {
                        *old = *row;
                    }
                    if self.page == Page::Apps {
                        self.start = self.start.min(self.visible_rows().len().saturating_sub(1));
                    }
                }
                Update::Progress(s) => {
                    self.message = s;
                    self.readout = Readout::Status;
                }
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
                    if let Some(key) = self.operation.take() {
                        match &result {
                            Err(error) => {
                                self.errors.insert(key, error.clone());
                            }
                            Ok(_) => {
                                self.errors.remove(&key);
                            }
                        }
                    }
                    self.message = result.unwrap_or_else(|e| e);
                    self.readout = Readout::Status;
                }
            }
        }
        changed
    }
    fn send(&mut self, command: Command) {
        self.readout = Readout::Status;
        if let Some(w) = &self.worker {
            if matches!(command, Command::Install(_)) {
                self.download_cancel = Some(std::sync::Arc::clone(&w.cancelled));
            }
            self.operation = match &command {
                Command::Install(keys) => keys.first().cloned(),
                Command::Uninstall(key) => Some(key.clone()),
                Command::Answer(_, _) => self.operation.take(),
                _ => None,
            };
            if matches!(command, Command::Check | Command::Install(_)) {
                w.cancelled
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }
            match w.send.send(command) {
                Ok(()) => {
                    self.busy = true;
                    self.message = if self.operation.is_some() {
                        "Preparing app operation..."
                    } else {
                        "Loading repository information..."
                    }
                    .into();
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
        matches!(self.page, Page::Edit | Page::Search) && self.open
    }
    pub const fn title(&self) -> &str {
        match self.page {
            Page::Apps => "App Center",
            Page::Sources => "Repositories",
            Page::Edit => "Edit repositories",
            Page::Details => "App details",
            Page::Changelog => "What's New",
            Page::Search => "Find an app",
        }
    }
    pub fn details(&self) -> String {
        if let Some(c) = &self.confirmation {
            return match c {
            Confirmation::Running(_, s)=>s.clone(),
            Confirmation::Uninstall(i)=>self.rows.get(*i).map_or_else(String::new, |r| format!("Uninstall {}? App files and launchers will be removed and backed up. Other files will be kept. Close the app first.", r.package.name)),
            Confirmation::Publisher(i)=>self.rows.get(*i).map_or_else(String::new,|r|format!("Duplicate app ID: {}. Explicitly select publisher {}?",r.package.id,r.package.origin.as_str())),
            Confirmation::Trust(i)=>self.rows.get(*i).map_or_else(String::new,|r|format!("Trust {} to supply executable app files for catalog {}? Apps are not sandboxed. ",r.package.repository.as_str(),r.package.origin.as_str())),
            Confirmation::Remove(i)=>format!("Remove {}? Installed apps and saves remain.",self.sources.catalogs[*i].as_str()),
        };
        }
        if self.readout == Readout::Status && matches!(self.page, Page::Apps | Page::Sources) {
            return self.message.clone();
        }
        if self.page == Page::Edit {
            return format!(
                "owner/repo or HTTPS URL; use ; for batch entry\n{}",
                self.text
            );
        }
        if self.page == Page::Details {
            return self.app_details();
        }
        if self.page == Page::Search {
            return format!("Search names and descriptions\n{}", self.text);
        }
        if self.page == Page::Changelog {
            return self.chosen_row().map_or_else(String::new, |r| {
                format!(
                    "{} - available {}\n\n{}",
                    r.package.name,
                    r.package.version,
                    r.package
                        .changelog
                        .as_deref()
                        .unwrap_or("No release notes are available for this version.")
                )
            });
        }
        if self.page == Page::Sources {
            return "Default catalog is always included. Customs supplement it.\nRemoving a source never uninstalls apps.".into();
        }
        if let Some(row) = self.chosen_row() {
            return format!("Selected: {} | {}", row.package.name, row.status);
        }
        self.rows.get(self.row).map_or_else(
            || self.message.clone(),
            |r| format!("{} | {}", r.package.origin.as_str(), r.status),
        )
    }
    fn app_details(&self) -> String {
        use std::fmt::Write;
        self.chosen_row().map_or_else(
            || self.message.clone(),
            |r| {
                let summary: String = r.package.description.chars().take(120).collect();
                let mut detail = format!(
                    "{}\n{}\n\nInstalled: {}\nAvailable: {}{}\n{}",
                    r.package.name,
                    summary,
                    r.installed,
                    r.package.version,
                    if r.update_available() {
                        " - update available"
                    } else {
                        ""
                    },
                    r.status
                );
                if let Some(error) = self.errors.get(&r.package.key()) {
                    let _ = write!(detail, "\n\nLAST OPERATION FAILED\n{error}");
                }
                if summary != r.package.description {
                    let _ = write!(detail, "\n\nABOUT\n{}", r.package.description);
                }
                let _ = write!(
                    detail,
                    "\n\nREPOSITORY\n{}\n\nREQUIREMENTS\n{}\nDownload: {} KiB",
                    r.package.origin.as_str(),
                    r.package.notes,
                    r.download_size.div_ceil(1024)
                );
                if r.package.repository != r.package.origin {
                    let _ = write!(detail, "\nSource: {}", r.package.repository.as_str());
                }
                let requirements: Vec<_> = ["network", "audio", "storage"]
                    .into_iter()
                    .filter(|key| r.package.permissions[key].as_bool() == Some(true))
                    .collect();
                let _ = write!(
                    detail,
                    "\nUses: {}\nApps run with your user permissions.\n\nApp ID: {}",
                    if requirements.is_empty() {
                        "no declared services".into()
                    } else {
                        requirements.join(", ")
                    },
                    r.package.id
                );
                detail
            },
        )
    }
    fn primary(&self) -> (Target, &'static str) {
        if self.busy && self.download_cancel.is_some() {
            return (Target::CancelOperation, "Cancel");
        }
        self.chosen_row().map_or((Target::Install, "Install"), |r| {
            if r.update_available() {
                (Target::Install, "Update")
            } else if r.ready && r.can_uninstall() {
                (Target::Install, "Repair")
            } else if r.ready {
                (Target::Install, "Install")
            } else if r.can_uninstall() {
                (Target::Open, "Open")
            } else {
                (Target::Install, "Unavailable")
            }
        })
    }
    fn visible_rows(&self) -> Vec<usize> {
        let query = self.search.to_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| self.matches_query(r, &query))
            .map(|(i, _)| i)
            .collect()
    }
    fn matches_query(&self, row: &Row, query: &str) -> bool {
        (query.is_empty()
            || row.package.name.to_lowercase().contains(query)
            || row.package.description.to_lowercase().contains(query))
            && match self.filter {
                Filter::All => true,
                Filter::Installed => row.can_uninstall(),
                Filter::Updates => row.update_available(),
            }
    }
    pub fn empty_message(&self) -> Option<&'static str> {
        if self.page != Page::Apps || !self.visible_rows().is_empty() {
            return None;
        }
        Some(if !self.search.is_empty() || self.filter != Filter::All {
            "No matches. Clear Search or choose All."
        } else if self.busy {
            "Loading apps..."
        } else {
            "No apps yet. Refresh to load repositories."
        })
    }
    pub fn row_content(&self, target: &Target) -> Option<(&str, &str, String)> {
        if self.page == Page::Sources {
            let Target::Row(i) = target else {
                return None;
            };
            let origin = self.sources.catalogs.get(*i)?;
            let rows: Vec<_> = self
                .rows
                .iter()
                .filter(|r| r.package.origin == *origin)
                .collect();
            let status = rows
                .iter()
                .find(|r| r.package.entry.is_empty())
                .map_or_else(
                    || {
                        if rows.is_empty() {
                            "Refresh to load this repository".into()
                        } else {
                            format!("{} apps available", rows.len())
                        }
                    },
                    |r| r.status.clone(),
                );
            return Some((
                origin.as_str(),
                if *i == 0 {
                    "Default repository - always included"
                } else {
                    "Custom repository"
                },
                status,
            ));
        }
        if self.page != Page::Apps {
            return None;
        }
        let Target::Row(i) = target else {
            return None;
        };
        let r = self.rows.get(*i)?;
        let state = if self.operation.as_ref() == Some(&r.package.key()) {
            self.message
                .lines()
                .next()
                .unwrap_or("Preparing")
                .to_owned()
        } else if self.errors.contains_key(&r.package.key()) {
            "Failed - see Details".into()
        } else if r.update_available() {
            format!("Update: {} > {}", r.installed, r.package.version)
        } else if r.can_uninstall() {
            format!("Installed {}", r.installed)
        } else if r.ready {
            format!("Available {}", r.package.version)
        } else {
            "Unavailable - see Details".into()
        };
        Some((&r.package.name, &r.package.description, state))
    }
    pub fn row_icon(&self, target: &Target) -> Option<&[u8]> {
        if self.page != Page::Apps {
            return None;
        }
        let Target::Row(i) = target else {
            return None;
        };
        self.rows
            .get(*i)?
            .package
            .icon
            .as_deref()
            .map(Vec::as_slice)
    }
    pub fn detail_icon(&self) -> Option<&[u8]> {
        if self.page != Page::Details || self.confirmation.is_some() {
            return None;
        }
        self.chosen_row()?
            .package
            .icon
            .as_deref()
            .map(Vec::as_slice)
    }
    pub fn footer(&self) -> &str {
        if self.confirmation.is_some() {
            "Cancel is the safe default"
        } else if self.busy {
            &self.message
        } else if self.editing() {
            "Tab: next control | Enter: activate"
        } else {
            ""
        }
    }
    pub fn row_chosen(&self, target: &Target) -> bool {
        if self.page != Page::Apps {
            return false;
        }
        let Target::Row(i) = target else {
            return false;
        };
        self.rows
            .get(*i)
            .is_some_and(|r| self.chosen.as_ref() == Some(&r.package.key()))
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
                    } else if matches!(self.confirmation, Some(Confirmation::Uninstall(_))) {
                        "Uninstall"
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
                (Target::Check, "Refresh"),
                if self.busy && self.download_cancel.is_some() {
                    (Target::CancelOperation, "Cancel")
                } else {
                    self.primary()
                },
                (Target::Sources, "Sources"),
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
            Page::Details => vec![
                self.primary(),
                (Target::Changelog, "What's New"),
                (Target::Cancel, "Back"),
            ],
            Page::Changelog => vec![(Target::Cancel, "Back to details")],
            Page::Search => vec![
                (Target::Save, "Search"),
                (Target::Clear, "Clear"),
                (Target::Delete, "Delete"),
                (Target::Cancel, "Cancel"),
            ],
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
        if matches!(self.page, Page::Edit | Page::Search) {
            for (i, c) in (if self.page == Page::Search {
                "abcdefghijklmnopqrstuvwxyz0123456789-_/. "
            } else {
                "abcdefghijklmnopqrstuvwxyz0123456789-_/.:"
            })
            .chars()
            .enumerate()
            {
                out.push((
                    Target::Character(c),
                    if c == ' ' {
                        "Space".into()
                    } else {
                        c.to_string()
                    },
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
        self.list_targets(layout, out);
        for (target, label, x) in [
            (Target::Previous, "Previous", 8),
            (Target::Next, "Next", width - 96),
        ] {
            out.push((
                target,
                label.into(),
                Rect {
                    x,
                    y: height - 58,
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
                    y: height - 58,
                    w: 88,
                    h: 30,
                },
            ));
        }
        if self.page == Page::Details {
            let (target, label) = if self.enabled(&Target::Approve) {
                (Target::Approve, "Trust source")
            } else {
                (Target::Uninstall, "Remove")
            };
            out.push((
                target,
                label.into(),
                Rect {
                    x: width / 2 - 60,
                    y: height - 58,
                    w: 120,
                    h: 30,
                },
            ));
        }
    }
    fn list_targets(&self, layout: &Layout, out: &mut Vec<(Target, String, Rect)>) {
        let width = i32::from(layout.width);
        if matches!(self.page, Page::Apps | Page::Sources) {
            let indices = if self.page == Page::Apps {
                self.visible_rows()
            } else {
                (0..self.sources.catalogs.len()).collect()
            };
            if self.page == Page::Apps {
                for (target, label, x, w) in [
                    (
                        Target::Search,
                        if self.search.is_empty() {
                            "Search apps...".into()
                        } else {
                            format!("Search: {}", self.search)
                        },
                        8,
                        width * 2 / 3 - 12,
                    ),
                    (
                        Target::Filter,
                        format!("{:?} ({})", self.filter, indices.len()),
                        width * 2 / 3,
                        width / 3 - 8,
                    ),
                ] {
                    out.push((target, label, Rect { x, y: 66, w, h: 26 }));
                }
            }
            for (position, &i) in indices
                .iter()
                .enumerate()
                .skip(self.start)
                .take(Self::capacity(layout))
            {
                let label = if self.page == Page::Apps {
                    let row = &self.rows[i];
                    let selected = self.chosen.as_ref() == Some(&row.package.key());
                    format!(
                        "[{}] {}",
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
                        y: if self.page == Page::Apps { 98 } else { 68 }
                            + i32::try_from(position - self.start).unwrap_or(0) * 52,
                        w: width - 16,
                        h: 48,
                    },
                ));
            }
        }
    }
    fn capacity(layout: &Layout) -> usize {
        usize::from(layout.height.saturating_sub(160) / 52).max(1)
    }
    pub fn enabled(&self, target: &Target) -> bool {
        if *target == Target::Home {
            return true;
        }
        if self.confirmation.is_some() {
            return matches!(target, Target::Confirm(_));
        }
        if self.busy {
            if self.page == Page::Search
                && matches!(
                    target,
                    Target::Save | Target::Clear | Target::Delete | Target::Character(_)
                )
            {
                return true;
            }
            return matches!(
                target,
                Target::Row(_)
                    | Target::Previous
                    | Target::Next
                    | Target::Details
                    | Target::Changelog
                    | Target::Search
                    | Target::Filter
                    | Target::Cancel
            ) || *target == Target::CancelOperation && self.download_cancel.is_some();
        }
        match target {
            Target::Install => self.chosen_row().is_some_and(|row| row.ready),
            Target::Details | Target::Changelog => self.chosen_row().is_some(),
            Target::Open => self.chosen_row().is_some_and(|r| {
                r.can_uninstall()
                    && super::metadata::version(&r.installed).is_ok()
                    && !r.status.starts_with("incomplete")
            }),
            Target::CancelOperation => false,
            Target::Uninstall => {
                self.page == Page::Details && self.chosen_row().is_some_and(Row::can_uninstall)
            }
            Target::Edit | Target::Remove => self.row > 0 && self.row < self.sources.catalogs.len(),
            Target::Approve => self.chosen_row().is_some_and(|r| {
                !self
                    .sources
                    .trusted(&r.package.origin, &r.package.repository)
            }),
            _ => true,
        }
    }
    pub fn event(&mut self, event: &Event, layout: &Layout) {
        self.selected = self
            .selected
            .min(self.targets(layout).len().saturating_sub(1));
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
            && (!self.busy || self.page == Page::Search)
        {
            self.text.pop();
            return;
        }
        if let Event::TextInput { text, .. } = event {
            if self.editing() && (!self.busy || self.page == Page::Search) {
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
                Keycode::Up | Keycode::Kp8 => self.vertical(false, layout, targets),
                Keycode::Down | Keycode::Kp2 => self.vertical(true, layout, targets),
                Keycode::Left | Keycode::Kp4 => self.horizontal(false, targets),
                Keycode::Right | Keycode::Kp6 => self.horizontal(true, targets),
                Keycode::Tab => {
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
            if let Some((Target::Row(i), _, _)) = self.targets(layout).get(self.selected) {
                self.row = *i;
                self.readout = Readout::Selection;
            }
        }
    }
    fn horizontal(&mut self, forward: bool, targets: &[(Target, String, Rect)]) {
        // Button rows form a separate cycle, so paging controls remain reachable
        // without traversing the app list. Order follows the visible layout.
        let mut buttons: Vec<_> = targets
            .iter()
            .enumerate()
            .filter(|(_, (target, _, _))| !matches!(target, Target::Row(_)))
            .collect();
        buttons.sort_by_key(|(_, (_, _, r))| (r.y, r.x));
        if let Some(index) = buttons.iter().position(|(i, _)| *i == self.selected) {
            let next = if forward {
                (index + 1) % buttons.len()
            } else {
                (index + buttons.len() - 1) % buttons.len()
            };
            self.selected = buttons[next].0;
        }
    }
    fn vertical(&mut self, down: bool, layout: &Layout, targets: &[(Target, String, Rect)]) {
        let Some((target, _, current)) = targets.get(self.selected) else {
            return;
        };
        if let Target::Row(index) = target {
            let indices = if self.page == Page::Apps {
                self.visible_rows()
            } else {
                (0..self.sources.catalogs.len()).collect()
            };
            let position = indices.iter().position(|i| i == index).unwrap_or(0);
            let adjacent = if down {
                position.checked_add(1).filter(|i| *i < indices.len())
            } else {
                position.checked_sub(1)
            };
            if let Some(next) = adjacent {
                if next < self.start {
                    self.start = next;
                }
                if next >= self.start + Self::capacity(layout) {
                    self.start = next + 1 - Self::capacity(layout);
                }
                if let Some(i) = self
                    .targets(layout)
                    .iter()
                    .position(|(t, _, _)| *t == Target::Row(indices[next]))
                {
                    self.selected = i;
                }
                return;
            }
        }
        let next = targets
            .iter()
            .enumerate()
            .filter(|(_, (_, _, r))| {
                if down {
                    r.y > current.y
                } else {
                    r.y < current.y
                }
            })
            .min_by_key(|(_, (_, _, r))| ((r.y - current.y).abs(), (r.x - current.x).abs()));
        if let Some((index, _)) = next {
            self.selected = index;
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
        if matches!(self.page, Page::Apps) {
            self.app_start = self.start;
        }
        self.page = page;
        self.readout = Readout::Selection;
        self.start = if matches!(page, Page::Apps) {
            self.app_start
        } else {
            0
        };
        self.selected = 0;
        self.contact = None;
    }
    fn cancel_download(&mut self) {
        if let Some(cancelled) = &self.download_cancel {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.message = "Cancelling download; any commit already started will finish safely".into();
    }
    fn confirm_uninstall(&mut self) {
        self.confirmation = Some(Confirmation::Uninstall(self.row));
        self.selected = 0;
        self.contact = None;
    }
    #[cfg(test)]
    pub fn row_versions(&self, target: &Target) -> Option<(String, bool)> {
        if self.page != Page::Apps {
            return None;
        }
        let Target::Row(index) = target else {
            return None;
        };
        self.rows
            .get(*index)
            .map(|r| (r.package.version.to_string(), r.update_available()))
    }
    fn check_catalogs(&mut self) {
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
        if target == Target::Cancel && self.busy && self.page == Page::Apps {
            self.cancel_download();
            return;
        }
        match target {
            Target::Check => self.check_catalogs(),
            Target::Install => self.install_selected(),
            Target::CancelOperation => self.cancel_download(),
            Target::Open => {
                self.launch = self.chosen_row().map(|r| r.package.id.clone());
            }
            Target::Changelog => self.page(Page::Changelog),
            Target::Search => {
                self.text = self.search.clone();
                self.page(Page::Search);
            }
            Target::Filter => {
                self.filter = match self.filter {
                    Filter::All => Filter::Installed,
                    Filter::Installed => Filter::Updates,
                    Filter::Updates => Filter::All,
                };
                self.start = 0;
                self.selected = 0;
            }
            Target::Sources => {
                self.row = 0;
                self.page(Page::Sources);
            }
            Target::Home => {
                self.lost_focus();
                self.open = false;
            }
            Target::Cancel => match self.page {
                Page::Apps => self.open = false,
                Page::Edit => self.page(Page::Sources),
                Page::Changelog => self.page(Page::Details),
                _ => self.page(Page::Apps),
            },
            Target::Row(index) => {
                self.readout = Readout::Selection;
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
            Target::Save => self.save_text(),
            Target::Character(c) => self.append(&c.to_string()),
            Target::Delete => {
                self.text.pop();
            }
            Target::Clear => self.text.clear(),
            Target::Details => self.show_details(),
            Target::Uninstall => self.confirm_uninstall(),
            Target::Approve => {
                self.confirmation = Some(Confirmation::Trust(self.row));
                self.selected = 0;
            }
            Target::Confirm(yes) => self.answer(yes),
            Target::Previous => self.scroll(false, layout),
            Target::Next => self.scroll(true, layout),
        }
    }
    fn scroll(&mut self, forward: bool, layout: &Layout) {
        let count = match self.page {
            Page::Apps => self.visible_rows().len(),
            Page::Sources => self.sources.catalogs.len(),
            Page::Details | Page::Changelog => self.lines(usize::from(layout.width) / 8 - 2).len(),
            Page::Edit | Page::Search => 0,
        };
        let step = if matches!(self.page, Page::Details | Page::Changelog) {
            8
        } else {
            Self::capacity(layout)
        };
        if !forward {
            self.start = self.start.saturating_sub(step);
        } else if self.start + step < count {
            self.start += step;
        }
        self.selected = 0;
    }
    fn save_text(&mut self) {
        if self.page == Page::Search {
            self.search = self.text.trim().to_owned();
            self.app_start = 0;
            self.page(Page::Apps);
            return;
        }
        let mut next = self.sources.clone();
        match next.edit(self.edit, &self.text) {
            Ok(()) => {
                self.send(Command::Save(next));
                self.page(Page::Sources);
            }
            Err(e) => self.message = e,
        }
    }
    fn chosen_row(&self) -> Option<&Row> {
        self.rows
            .iter()
            .find(|row| self.chosen.as_ref() == Some(&row.package.key()))
            .filter(|row| {
                self.page != Page::Apps || self.matches_query(row, &self.search.to_lowercase())
            })
    }
    fn show_details(&mut self) {
        if let Some(index) = self
            .rows
            .iter()
            .position(|row| self.chosen.as_ref() == Some(&row.package.key()))
        {
            self.row = index;
            self.page(Page::Details);
        }
    }
    fn install_selected(&mut self) {
        if let Some(row) = self.chosen_row().filter(|row| row.ready) {
            self.send(Command::Install(vec![row.package.key()]));
        }
    }
    fn toggle(&mut self, index: usize) {
        let row = &self.rows[index];
        let key = row.package.key();
        if self.chosen.as_ref() == Some(&key) {
            self.chosen = None;
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
            self.chosen = Some(key);
        }
    }
    fn answer(&mut self, yes: bool) {
        if let Some(c) = self.confirmation.take() {
            match c {
                Confirmation::Running(id, _) => self.send(Command::Answer(id, yes)),
                Confirmation::Publisher(i) if yes => {
                    let row = &self.rows[i];
                    self.chosen = Some(row.package.key());
                }
                Confirmation::Trust(i) if yes => {
                    let r = &self.rows[i];
                    let mut next = self.sources.clone();
                    next.approvals
                        .insert((r.package.origin.clone(), r.package.repository.clone()));
                    self.send(Command::Save(next));
                }
                Confirmation::Uninstall(i) if yes => {
                    if let Some(row) = self.rows.get(i) {
                        self.send(Command::Uninstall(row.package.key()));
                    }
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
        if matches!(self.page, Page::Details | Page::Changelog) {
            self.start
        } else {
            0
        }
    }
    pub const fn full_details(&self) -> bool {
        self.confirmation.is_some() || matches!(self.page, Page::Details | Page::Changelog)
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
            ("search", Page::Search),
            ("changelog", Page::Changelog),
        ] {
            let mut center = Self::fixture()?;
            center.page(page);
            if matches!(page, Page::Details | Page::Changelog) {
                center.chosen = Some(center.rows[0].package.key());
                center.rows[0].package.changelog = Some(
                    "# Changelog\n\n## 1.1.0 - 2026-09-12\n\n- Clearer app controls.\n- Reliable updates.\n\n## 1.0.0 - 2026-09-01\n\n- First release.".into(),
                );
            }
            center.text = "example/catalog;https://github.com/my/catalog".into();
            out.push((name, center));
        }
        let mut center = Self::fixture()?;
        center.rows[0].installed = "1.0.0".into();
        center.rows[0].package.name = "Bitcoin Dashboard".into();
        out.push(("update-badge", center));
        let mut center = Self::fixture()?;
        center.rows[0].installed = "1.0.0".into();
        center.chosen = Some(center.rows[0].package.key());
        center.page(Page::Details);
        center.confirm_uninstall();
        out.push(("uninstall", center));
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
    fn focused(center: &Center, layout: &Layout) -> Target {
        center.targets(layout)[center.selected].0
    }
    #[test]
    fn directional_navigation_follows_rows_and_bypasses_long_lists() -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            let layout = Layout::home(w, h)?;
            for (up, down, left, right) in [
                (Keycode::Up, Keycode::Down, Keycode::Left, Keycode::Right),
                (Keycode::Kp8, Keycode::Kp2, Keycode::Kp4, Keycode::Kp6),
            ] {
                let mut center = center()?;
                let cycle = [
                    Target::Check,
                    Target::Install,
                    Target::Sources,
                    Target::Home,
                    Target::Search,
                    Target::Filter,
                    Target::Previous,
                    Target::Details,
                    Target::Next,
                ];
                for target in cycle.iter().cycle().skip(1).take(cycle.len()) {
                    center.event(&key(right), &layout);
                    assert_eq!(focused(&center, &layout), *target);
                }
                for target in cycle.iter().rev() {
                    center.event(&key(left), &layout);
                    assert_eq!(focused(&center, &layout), *target);
                }
                center.event(&key(down), &layout);
                assert_eq!(focused(&center, &layout), Target::Search);
                for index in 0..center.rows.len() {
                    center.event(&key(down), &layout);
                    assert_eq!(focused(&center, &layout), Target::Row(index));
                    assert_eq!(center.row, index);
                }
                assert!(center.start > 0);
                center.event(&key(down), &layout);
                assert_eq!(focused(&center, &layout), Target::Previous);
                for index in (0..center.rows.len()).rev() {
                    center.event(&key(up), &layout);
                    assert_eq!(focused(&center, &layout), Target::Row(index));
                }
                center.event(&key(up), &layout);
                assert_eq!(focused(&center, &layout), Target::Search);
                center.event(&key(up), &layout);
                assert_eq!(focused(&center, &layout), Target::Check);
                assert!(center.chosen.is_none());
                center.rows.clear();
                center.event(&key(down), &layout);
                center.event(&key(down), &layout);
                assert_eq!(focused(&center, &layout), Target::Previous);
            }
        }
        Ok(())
    }
    #[test]
    fn single_readout_switches_between_row_status_and_operation_results() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut center = center()?;
        assert_eq!(center.details(), center.message);
        center.event(&key(Keycode::Down), &layout);
        center.event(&key(Keycode::Down), &layout);
        assert_eq!(
            center.details(),
            format!("{} | ready", super::super::sources::DEFAULT)
        );
        let (send, _commands) = std::sync::mpsc::channel();
        let (updates, receive) = std::sync::mpsc::channel();
        center.worker = Some(Worker {
            send,
            receive,
            cancelled: std::sync::Arc::default(),
        });
        updates
            .send(Update::Progress("Downloading 5 / 10 bytes (50%)".into()))
            .map_err(|e| e.to_string())?;
        center.poll();
        assert_eq!(center.details(), "Downloading 5 / 10 bytes (50%)");
        updates
            .send(Update::Done(Err("Download failed".into()), false))
            .map_err(|e| e.to_string())?;
        center.poll();
        assert_eq!(center.details(), "Download failed");
        center.event(&key(Keycode::Down), &layout);
        assert!(center.details().ends_with(" | ready"));
        updates
            .send(Update::Done(Ok("App uninstalled".into()), true))
            .map_err(|e| e.to_string())?;
        center.poll();
        assert_eq!(center.details(), "App uninstalled");
        Ok(())
    }
    fn tap(center: &mut Center, target: Target, layout: &Layout) -> Result<(), String> {
        let bounds = center
            .targets(layout)
            .into_iter()
            .find(|(t, _, _)| *t == target)
            .ok_or("target")?
            .2;
        let x = f32::from(u16::try_from(bounds.x + bounds.w / 2).map_err(|e| e.to_string())?)
            / f32::from(layout.width);
        let y = f32::from(u16::try_from(bounds.y + bounds.h / 2).map_err(|e| e.to_string())?)
            / f32::from(layout.height);
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
            layout,
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
            layout,
        );
        Ok(())
    }
    #[test]
    fn details_and_uninstall_use_the_single_selection_across_pages_and_focus_changes()
    -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            let layout = Layout::home(w, h)?;
            let mut center = center()?;
            center.rows[0].ready = false; // Unsupported/uninstalled apps still have readable information.
            center.rows[8].installed = center.rows[8].package.version.to_string();
            center.rows[8].ready = false;
            let chosen = center.rows[8].package.key();
            let (send, commands) = std::sync::mpsc::channel();
            let (_updates, receive) = std::sync::mpsc::channel();
            center.worker = Some(Worker {
                send,
                receive,
                cancelled: std::sync::Arc::default(),
            });
            assert!(!center.enabled(&Target::Details));
            center.event(&key(Keycode::Down), &layout);
            center.event(&key(Keycode::Down), &layout);
            center.event(&key(Keycode::Return), &layout);
            tap(&mut center, Target::Details, &layout)?;
            assert!(center.details().starts_with("App 0\n"));
            assert!(!center.enabled(&Target::Uninstall));
            tap(&mut center, Target::Cancel, &layout)?;
            while center.start + Center::capacity(&layout) <= 8 {
                center.event(&key(Keycode::PageDown), &layout);
            }
            tap(&mut center, Target::Row(8), &layout)?;
            assert_eq!(center.chosen.as_ref(), Some(&chosen));
            assert!(center.details().starts_with("Selected: App 8 |"));
            while center.start > 0 {
                center.event(&key(Keycode::PageUp), &layout);
            }
            assert!(
                center
                    .targets(&layout)
                    .iter()
                    .all(|(_, label, _)| !label.starts_with("[X]"))
            );
            // Traverse another row without changing the explicit selection.
            center.event(&key(Keycode::Down), &layout);
            center.event(&key(Keycode::Down), &layout);
            assert_eq!(center.row, 0);
            center.event(&key(Keycode::Up), &layout);
            center.event(&key(Keycode::Up), &layout);
            for _ in 0..7 {
                center.event(&key(Keycode::Right), &layout);
            }
            assert_eq!(focused(&center, &layout), Target::Details);
            center.event(&key(Keycode::Return), &layout);
            assert!(center.details().starts_with("App 8\n"));
            assert!(!center.enabled(&Target::Install));
            assert!(center.enabled(&Target::Uninstall));
            tap(&mut center, Target::Uninstall, &layout)?;
            assert!(center.details().starts_with("Uninstall App 8?"));
            assert_eq!(focused(&center, &layout), Target::Confirm(false));
            center.event(&key(Keycode::Return), &layout);
            assert!(commands.try_recv().is_err());
            tap(&mut center, Target::Uninstall, &layout)?;
            tap(&mut center, Target::Confirm(true), &layout)?;
            assert!(matches!(commands.try_recv(), Ok(Command::Uninstall(key)) if key == chosen));
            assert!(commands.try_recv().is_err());
        }
        Ok(())
    }
    #[test]
    fn catalog_refresh_preserves_selection_and_invalidates_old_confirmation() -> Result<(), String>
    {
        let layout = Layout::home(480, 272)?;
        let mut center = center()?;
        center.rows[1].installed = "1.0.0".into();
        center.activate(Target::Row(1), &layout);
        center.activate(Target::Row(1), &layout);
        assert!(center.chosen.is_none());
        assert!(!center.enabled(&Target::Details));
        center.activate(Target::Details, &layout);
        assert_eq!(center.page, Page::Apps);
        center.activate(Target::Row(1), &layout);
        center.activate(Target::Sources, &layout);
        center.activate(Target::Cancel, &layout);
        center.activate(Target::Details, &layout);
        assert!(center.details().starts_with("App 1\n"));
        center.activate(Target::Uninstall, &layout);
        let (send, commands) = std::sync::mpsc::channel();
        let (updates, receive) = std::sync::mpsc::channel();
        center.worker = Some(Worker {
            send,
            receive,
            cancelled: std::sync::Arc::default(),
        });
        let mut replacement = Center::fixture()?.rows;
        replacement.reverse();
        updates
            .send(Update::Rows(replacement))
            .map_err(|e| e.to_string())?;
        center.poll();
        assert_eq!(center.chosen, Some(center.rows[7].package.key()));
        assert!(center.confirmation.is_none());
        assert!(center.enabled(&Target::Details));
        assert!(!center.enabled(&Target::Uninstall));
        center.activate(Target::Confirm(true), &layout);
        assert!(commands.try_recv().is_err());
        Ok(())
    }
    #[test]
    fn current_apps_can_be_selected_without_entering_the_install_queue() -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            for touch in [false, true] {
                let layout = Layout::home(w, h)?;
                let mut center = center()?;
                center.rows[0].installed = center.rows[0].package.version.to_string();
                center.rows[0].ready = false;
                let current = center.rows[0].package.key();
                let available = center.rows[1].package.key();
                let (send, commands) = std::sync::mpsc::channel();
                let (_updates, receive) = std::sync::mpsc::channel();
                center.worker = Some(Worker {
                    send,
                    receive,
                    cancelled: std::sync::Arc::default(),
                });
                if touch {
                    let bounds = center
                        .targets(&layout)
                        .into_iter()
                        .find(|(t, _, _)| *t == Target::Row(0))
                        .ok_or("row")?
                        .2;
                    for down in [true, false] {
                        let event = if down {
                            Event::MouseButtonDown {
                                timestamp: 0,
                                window_id: 1,
                                which: 0,
                                mouse_btn: MouseButton::Left,
                                clicks: 1,
                                x: bounds.x + 1,
                                y: bounds.y + 1,
                            }
                        } else {
                            Event::MouseButtonUp {
                                timestamp: 0,
                                window_id: 1,
                                which: 0,
                                mouse_btn: MouseButton::Left,
                                clicks: 1,
                                x: bounds.x + 1,
                                y: bounds.y + 1,
                            }
                        };
                        center.event(&event, &layout);
                    }
                } else {
                    center.event(&key(Keycode::Down), &layout);
                    center.event(&key(Keycode::Down), &layout);
                    center.event(&key(Keycode::Space), &layout);
                }
                assert!(center.chosen.as_ref() == Some(&current));
                assert!(
                    center
                        .targets(&layout)
                        .iter()
                        .any(|(t, label, _)| *t == Target::Row(0) && label.starts_with("[X]"))
                );
                assert!(!center.enabled(&Target::Install));
                center.event(&key(Keycode::I), &layout);
                assert!(commands.try_recv().is_err());
                center.activate(Target::Details, &layout);
                assert!(center.enabled(&Target::Uninstall));
                center.activate(Target::Cancel, &layout);
                center.toggle(1);
                assert!(center.enabled(&Target::Install));
                center.activate(Target::Install, &layout);
                assert!(
                    matches!(commands.try_recv(), Ok(Command::Install(keys)) if keys == vec![available.clone()])
                );
                assert_eq!(center.chosen, Some(available));
            }
        }
        Ok(())
    }
    #[test]
    fn uninstall_details_confirmation_is_safe_and_dispatches_once() -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            let layout = Layout::home(w, h)?;
            let mut center = center()?;
            let (send, commands) = std::sync::mpsc::channel();
            let (_updates, receive) = std::sync::mpsc::channel();
            center.worker = Some(Worker {
                send,
                receive,
                cancelled: std::sync::Arc::default(),
            });
            center.rows[0].installed = "1.0.0".into();
            center.rows[0].ready = false; // Current apps are still uninstallable.
            let key_value = center.rows[0].package.key();
            center.activate(Target::Row(0), &layout);
            center.activate(Target::Details, &layout);
            assert!(center.enabled(&Target::Uninstall));
            assert!(
                center
                    .targets(&layout)
                    .iter()
                    .any(|(t, label, _)| *t == Target::Uninstall && label == "Remove")
            );
            center.selected = center
                .targets(&layout)
                .iter()
                .position(|(target, _, _)| *target == Target::Uninstall)
                .ok_or("Remove control")?;
            center.event(&key(Keycode::Return), &layout);
            assert!(matches!(
                center.confirmation,
                Some(Confirmation::Uninstall(0))
            ));
            center.event(&key(Keycode::Return), &layout);
            assert!(center.confirmation.is_none());
            assert!(commands.try_recv().is_err());
            center.activate(Target::Uninstall, &layout);
            center.lost_focus();
            assert!(center.confirmation.is_none());
            center.activate(Target::Uninstall, &layout);
            center.event(&key(Keycode::Right), &layout);
            center.event(&key(Keycode::Return), &layout);
            assert!(matches!(commands.try_recv(), Ok(Command::Uninstall(key)) if key == key_value));
            assert!(center.busy);
            center.event(&key(Keycode::Return), &layout);
            assert!(commands.try_recv().is_err());
        }
        Ok(())
    }
    #[test]
    fn uninstall_touch_uses_the_same_confirmation_and_new_versions_have_badges()
    -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480)] {
            let layout = Layout::home(w, h)?;
            let mut center = center()?;
            center.rows[0].package.version = super::super::metadata::version("1.10.0")?;
            center.rows[0].package.installable = true;
            for (installed, update) in [
                ("not installed", false),
                ("local / unknown", false),
                ("1.9.0", true),
                ("1.10.0", false),
                ("2.0.0", false),
            ] {
                center.rows[0].installed = installed.into();
                let (version, badge) = center.row_versions(&Target::Row(0)).ok_or("row")?;
                assert_eq!(version, "1.10.0");
                assert_eq!(badge, update);
            }
            center.rows[0].installed = "1.9.0".into();
            center.chosen = Some(center.rows[0].package.key());
            center.page(Page::Details);
            assert!(
                center
                    .details()
                    .contains("Available: 1.10.0 - update available")
            );
            let bounds = center
                .targets(&layout)
                .into_iter()
                .find(|(t, _, _)| *t == Target::Uninstall)
                .ok_or("Uninstall")?
                .2;
            let x = f32::from(u16::try_from(bounds.x + bounds.w / 2).map_err(|e| e.to_string())?)
                / f32::from(w);
            let y = f32::from(u16::try_from(bounds.y + bounds.h / 2).map_err(|e| e.to_string())?)
                / f32::from(h);
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
            center.event(&up, &layout);
            assert!(center.confirmation.is_none());
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
            center.event(&up, &layout);
            assert!(matches!(
                center.confirmation,
                Some(Confirmation::Uninstall(0))
            ));
            assert_eq!(center.selected, 0);
            center.answer(false);
            center.rows[0].installed = "not installed".into();
            assert!(!center.enabled(&Target::Uninstall));
        }
        Ok(())
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
                    .find(|(_, (t, _, _))| *t == Target::CancelOperation)
                    .ok_or("Cancel target")?;
                assert_eq!(label, "Cancel");
                assert!(center.enabled(&Target::CancelOperation));
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
        for page in [
            Page::Apps,
            Page::Sources,
            Page::Edit,
            Page::Details,
            Page::Search,
            Page::Changelog,
        ] {
            for busy in [false, true] {
                let mut center = center()?;
                center.page(page);
                center.busy = busy;
                center.confirmation = Some(Confirmation::Publisher(0));
                center.event(&key(Keycode::Home), &layout);
                assert!(!center.open);
                assert!(center.confirmation.is_none());
                assert!(center.chosen.is_none());
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
            .send(Update::Done(Ok("Done".into()), true))
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
            for page in [
                Page::Apps,
                Page::Sources,
                Page::Edit,
                Page::Details,
                Page::Search,
                Page::Changelog,
            ] {
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
            assert!(center.chosen.is_none());
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
            assert!(center.chosen.is_none());
            center.confirmation = Some(Confirmation::Publisher(0));
            center.event(&key(Keycode::Kp6), &layout);
            center.event(&key(Keycode::KpEnter), &layout);
            assert_eq!(center.chosen.iter().count(), 1);
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
        assert!(center.chosen.is_none());
        assert!(matches!(
            center.confirmation,
            Some(Confirmation::Publisher(0))
        ));
        center.answer(true);
        assert_eq!(center.chosen.iter().count(), 1);
        center.toggle(1);
        center.answer(false);
        assert!(center.chosen.as_ref() == Some(&center.rows[0].package.key()));
        center.toggle(1);
        center.answer(true);
        assert_eq!(center.chosen.iter().count(), 1);
        assert!(center.chosen.as_ref() == Some(&center.rows[1].package.key()));
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

#[cfg(test)]
mod browsing_tests {
    use super::*;
    #[test]
    fn mutation_preserves_search_filter_selection_and_details_scroll() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut center = Center::fixture()?;
        center.search = "App".into();
        center.filter = Filter::Installed;
        center.rows[6].installed = "1.0.0".into();
        center.chosen = Some(center.rows[6].package.key());
        center.page(Page::Details);
        center.start = 8;
        let (send, _commands) = std::sync::mpsc::channel();
        let (updates, receive) = std::sync::mpsc::channel();
        center.worker = Some(Worker {
            send,
            receive,
            cancelled: std::sync::Arc::default(),
        });
        let mut replacement = Center::fixture()?.rows.remove(6);
        replacement.installed = "1.1.0".into();
        updates
            .send(Update::Row(Box::new(replacement)))
            .map_err(|e| e.to_string())?;
        center.poll();
        assert_eq!(center.start, 8);
        assert_eq!(center.search, "App");
        assert_eq!(center.filter, Filter::Installed);
        assert_eq!(center.chosen_row().ok_or("selection")?.installed, "1.1.0");
        center.busy = true;
        assert!(center.enabled(&Target::Changelog));
        assert!(!center.enabled(&Target::Install));
        center.activate(Target::Changelog, &layout);
        assert!(center.details().contains("No release notes"));
        center.activate(Target::Cancel, &layout);
        assert_eq!(center.page, Page::Details);
        Ok(())
    }
    #[test]
    fn filters_and_touch_search_share_actions_and_empty_state_is_explained() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut center = Center::fixture()?;
        center.rows[2].installed = "1.0.0".into();
        center.chosen = Some(center.rows[2].package.key());
        center.activate(Target::Filter, &layout);
        assert_eq!(center.visible_rows(), [2]);
        center.activate(Target::Search, &layout);
        center.append("missing app");
        center.activate(Target::Save, &layout);
        assert!(
            center
                .empty_message()
                .is_some_and(|s| s.contains("Clear Search"))
        );
        assert!(!center.enabled(&Target::Install));
        assert!(!center.enabled(&Target::Open));
        assert!(!center.enabled(&Target::Details));
        center.activate(Target::Search, &layout);
        center.activate(Target::Clear, &layout);
        center.activate(Target::Save, &layout);
        assert_eq!(center.visible_rows(), [2]);
        assert_eq!(
            center.chosen_row().ok_or("preserved selection")?.installed,
            "1.0.0"
        );
        Ok(())
    }
}
