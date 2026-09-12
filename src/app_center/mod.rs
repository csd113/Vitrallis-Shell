//! Native App Center services and screen state, independent of device adapters.
mod discovery;
mod install;
mod metadata;
mod network;
mod running;
mod runtime;
mod screen;
mod sources;
mod storage;
#[cfg(test)]
mod tests;
mod transaction;
mod uninstall;
pub use discovery::integrate;
pub use screen::Center;
pub const TILE_ID: &str = "vitrallis-app-center";
use install::Checked;
use sources::Sources;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};
use storage::Locations;
#[derive(Debug)]
enum Command {
    Check,
    Save(Sources),
    Install(Vec<String>),
    Uninstall(String),
    Answer(u64, bool),
}
#[derive(Debug)]
enum Update {
    Sources(Sources),
    Progress(String),
    Rows(Vec<Row>),
    Confirm(u64, String),
    Done(Result<String, String>, bool),
}
#[derive(Debug)]
struct Worker {
    send: Sender<Command>,
    receive: Receiver<Update>,
    cancelled: Arc<AtomicBool>,
}
impl Worker {
    fn start() -> Result<Self, String> {
        let loc = Locations::current()?;
        let (send, commands) = mpsc::channel();
        let (updates, receive) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        thread::Builder::new()
            .name("app-center".into())
            .spawn(move || service(&loc, &commands, &updates, &worker_cancelled))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            send,
            receive,
            cancelled,
        })
    }
}
fn service(
    loc: &Locations,
    commands: &Receiver<Command>,
    updates: &Sender<Update>,
    cancelled: &AtomicBool,
) {
    let mut rows = Vec::new();
    let initial = Sources::load(&loc.sources);
    let mut expected_sources = initial.as_ref().ok().cloned();
    if let Ok(sources) = &initial {
        let _ = updates.send(Update::Sources(sources.clone()));
    }
    let _ = updates.send(Update::Done(
        initial.map(|_| "Ready. Selections are unchecked by default".into()),
        false,
    ));
    while let Ok(command) = commands.recv() {
        let mut changed = false;
        let success = match &command {
            Command::Uninstall(_) => "App uninstalled. Other data kept; removed files backed up.",
            Command::Install(_) => "Installed; apps remain closed. Check again for current status",
            _ => "Ready. Selections are unchecked by default",
        };
        let result = (|| {
            let _lock = storage::Lock::take(&loc.state)?;
            match command {
                Command::Check => {
                    rows.clear();
                    let sources = Sources::load(&loc.sources)?;
                    expected_sources = Some(sources.clone());
                    let _ = updates.send(Update::Sources(sources.clone()));
                    rows = check_all(loc, &sources, &network::Curl, |s| {
                        let _ = updates.send(Update::Progress(s));
                    });
                    let _ = updates.send(Update::Rows(rows.iter().map(Row::from).collect()));
                }
                Command::Save(sources) => {
                    if Some(Sources::load(&loc.sources)?) != expected_sources {
                        return Err("Source settings changed; Check again before editing".into());
                    }
                    sources.save(&loc.sources)?;
                    expected_sources = Some(sources.clone());
                    rows.clear();
                    let _ = updates.send(Update::Sources(sources));
                    let _ = updates.send(Update::Rows(Vec::new()));
                }
                Command::Install(keys) => {
                    let sources = Sources::load(&loc.sources)?;
                    let mut errors = Vec::new();
                    for key in keys {
                        let Some(row) = rows.iter().find(|r| r.package.key() == key) else {
                            errors.push("Check is no longer valid".into());
                            continue;
                        };
                        let result = install_one(
                            loc,
                            &sources,
                            row,
                            commands,
                            updates,
                            cancelled,
                            &network::Curl,
                        );
                        match result {
                            Ok(()) => changed = true,
                            Err(e) => errors.push(format!("{}: {e}", row.package.name)),
                        }
                    }
                    rows.clear();
                    let _ = updates.send(Update::Rows(Vec::new()));
                    if !errors.is_empty() {
                        return Err(errors.join("; "));
                    }
                }
                Command::Uninstall(key) => {
                    let row = rows
                        .iter_mut()
                        .find(|r| r.package.key() == key)
                        .ok_or("Check is no longer valid")?;
                    let _ = updates.send(Update::Progress(format!(
                        "Uninstalling {}",
                        row.package.name
                    )));
                    changed = true;
                    uninstall::uninstall(loc, &row.package)?;
                    row.installed = "not installed".into();
                    row.status = "not installed; other data retained".into();
                    row.ready = row.package.installable;
                    let _ = updates.send(Update::Rows(rows.iter().map(Row::from).collect()));
                }
                Command::Answer(_, _) => {
                    return Err("No running-app confirmation is pending".into());
                }
            }
            Ok(())
        })();
        let _ = updates.send(Update::Done(result.map(|()| success.into()), changed));
    }
}
fn install_one(
    loc: &Locations,
    sources: &Sources,
    row: &Checked,
    commands: &Receiver<Command>,
    updates: &Sender<Update>,
    cancelled: &AtomicBool,
    fetch: &impl network::Fetch,
) -> Result<(), String> {
    use running::Processes;
    cancellation(cancelled)?;
    if !sources.catalogs.contains(&row.package.origin)
        || !sources.trusted(&row.package.origin, &row.package.repository)
    {
        return Err("Source removed or approval revoked; check again".into());
    }
    if !row.ready {
        return Err("No available update".into());
    }
    let entry = loc.root(&row.package).join(&row.package.entry);
    let processes = running::Native.list(&entry)?;
    if !processes.is_empty() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static PROMPT: AtomicU64 = AtomicU64::new(1);
        let id = PROMPT.fetch_add(1, Ordering::Relaxed);
        updates
            .send(Update::Confirm(
                id,
                format!(
                    "Close and update? Unsaved work may be lost. App: {}",
                    row.package.name
                ),
            ))
            .map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            match commands
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            {
                Ok(Command::Answer(token, true)) if token == id => {
                    running::close(
                        &running::Native,
                        &entry,
                        &processes,
                        std::time::Duration::from_secs(8),
                    )?;
                    break;
                }
                Ok(Command::Answer(token, _)) if token != id => (),
                _ => return Err("Cancelled; app left running".into()),
            }
        }
    }
    if !running::Native.list(&entry)?.is_empty() {
        return Err("App started again; update skipped".into());
    }
    let planned = acquire(loc, row, fetch, |s| {
        cancellation(cancelled)?;
        updates.send(Update::Progress(s)).map_err(|e| e.to_string())
    })?;
    if !running::Native.list(&entry)?.is_empty() {
        return Err("App started again; update skipped".into());
    }
    cancellation(cancelled)?;
    // Once commit begins, let the transaction finish or roll back without interruption.
    let _ = updates.send(Update::Progress(format!(
        "Installing {}: {}",
        row.package.name, planned.status
    )));
    install::install(loc, &planned)
}
fn cancellation(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        Err("Cancelled before installation".into())
    } else {
        Ok(())
    }
}

fn acquire(
    loc: &Locations,
    row: &Checked,
    fetch: &impl network::Fetch,
    mut progress: impl FnMut(String) -> Result<(), String>,
) -> Result<install::Planned, String> {
    if !row.ready {
        return Err("No available update".into());
    }
    let files = network::bundle(fetch, &row.package, &mut progress)?;
    progress(format!("Verifying {}", row.package.name))?;
    install::prepare(loc, row.package.clone(), files)
}

fn check_all(
    loc: &Locations,
    sources: &Sources,
    fetch: &impl network::Fetch,
    mut progress: impl FnMut(String),
) -> Vec<Checked> {
    let mut rows = Vec::new();
    for origin in &sources.catalogs {
        progress(format!("Checking {}", origin.as_str()));
        match network::catalog(fetch, origin) {
            Err(error) => {
                // Keep a non-installable source diagnostic row alongside successful origins.
                rows.push(source_error(origin, &error));
            }
            Ok(packages) => {
                for package in packages {
                    let installed = install::label(loc, &package).unwrap_or_else(|e| e);
                    let result = (|| {
                        if !package.installable {
                            return Err(format!("Disabled: {}", package.notes));
                        }
                        if !sources.trusted(origin, &package.repository) {
                            return Err(format!(
                                "Approval required for source {}",
                                package.repository.as_str()
                            ));
                        }
                        install::check(loc, package.clone())
                    })();
                    rows.push(result.unwrap_or_else(|status| Checked {
                        package,
                        installed,
                        status,
                        ready: false,
                    }));
                }
            }
        }
    }
    rows
}
fn source_error(origin: &sources::Repository, error: &str) -> Checked {
    Checked {
        package: metadata::Package {
            origin: origin.clone(),
            repository: origin.clone(),
            id: "io.vitrallis.sourceerror".into(),
            name: format!("Source: {}", origin.as_str()),
            version: metadata::Version::zero(),
            entry: String::new(),
            permissions: serde_json::Value::Null,
            installable: false,
            notes: String::new(),
            commit: String::new(),
            directory: String::new(),
            files: vec![],
        },
        installed: "unavailable".into(),
        status: error.into(),
        ready: false,
    }
}

#[derive(Debug)]
struct Row {
    package: metadata::Package,
    installed: String,
    status: String,
    ready: bool,
    download_size: usize,
}
impl From<&Checked> for Row {
    fn from(c: &Checked) -> Self {
        let mut package = c.package.clone();
        package.files.clear();
        Self {
            package,
            installed: c.installed.clone(),
            status: c.status.clone(),
            ready: c.ready,
            download_size: c.package.files.iter().map(|f| f.size).sum(),
        }
    }
}

impl Row {
    fn can_uninstall(&self) -> bool {
        !self.package.entry.is_empty()
            && !matches!(self.installed.as_str(), "not installed" | "unavailable")
    }
    fn update_available(&self) -> bool {
        self.package.installable
            && metadata::version(&self.installed)
                .is_ok_and(|installed| self.package.version > installed)
    }
}
