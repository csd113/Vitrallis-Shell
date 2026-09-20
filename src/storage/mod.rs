//! On-demand snapshots: one cancellable worker, cached across Settings visits.
mod disk;
pub mod scan;
#[cfg(test)]
mod tests;
use crate::app_center::{
    accounting::{self, AppUsage},
    storage::Locations,
};
pub use disk::{Disk, LowSpace, format_bytes};
use scan::{Scanner, Size};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};
const CACHE_AGE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub apps: Vec<AppUsage>,
    pub app_total: Size,
    pub categories: Vec<(&'static str, Size)>,
    pub issue: Option<String>,
    pub root_bytes: u64,
}
fn report(cancel: &AtomicBool) -> Result<Report, String> {
    let loc = Locations::current()?;
    let mut scanner = Scanner::new(cancel);
    let (apps, issue) = accounting::installed(&loc, &mut scanner)?;
    let mut app_total = accounting::aggregate(apps.iter().map(|app| &app.total));
    if let Some(issue) = &issue {
        app_total.fail(issue);
    }
    let mut categories = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        categories.push(("Shell executable", scanner.measure(&executable, false)));
    }
    // Scanned in ownership order; Scanner excludes already-counted roots and hard links.
    categories.push(("App Center cache", {
        let mut size = scanner.measure(&loc.state.join("catalogs"), true);
        size.add(&scanner.measure(&loc.state.join("presentation"), true));
        size
    }));
    categories.push((
        "Retained app data",
        scanner.measure(&loc.data.join("vitrallis/apps"), true),
    ));
    categories.push(("Manager state / backups", scanner.measure(&loc.state, true)));
    categories.push((
        "Shell data",
        scanner.measure(&loc.data.join("vitrallis"), true),
    ));
    if let Some(config) = loc.sources.parent() {
        categories.push(("Shell configuration", scanner.measure(config, true)));
    }
    Ok(Report {
        apps,
        app_total,
        categories,
        issue,
        root_bytes: scanner.root_bytes,
    })
}

#[derive(Debug)]
enum Update {
    Disk(Result<Disk, String>),
    Report(Result<Report, String>),
}
#[derive(Debug)]
struct Job {
    updates: Receiver<Update>,
    cancel: Arc<AtomicBool>,
    revision: u64,
    finished: bool,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Default)]
pub struct Storage {
    pub disk: Option<Result<Disk, String>>,
    pub report: Option<Report>,
    pub error: Option<String>,
    pub thresholds: LowSpace,
    pub snapshot: u64,
    job: Option<Job>,
    completed: Option<Instant>,
    revision: u64,
    wanted: bool,
}
impl Storage {
    pub const fn busy(&self) -> bool {
        self.job.is_some()
    }
    pub fn stale(&self) -> bool {
        self.completed
            .is_none_or(|time| time.elapsed() >= CACHE_AGE)
            || self.revision != crate::app_center::storage_revision()
    }
    pub fn enter(&mut self) {
        if self.stale()
            || self
                .job
                .as_ref()
                .is_some_and(|job| job.cancel.load(Ordering::Relaxed))
        {
            self.wanted = true;
        }
    }
    pub const fn refresh(&mut self) {
        if !self.busy() {
            self.wanted = true;
        }
    }
    pub fn close(&mut self) {
        self.wanted = false;
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn poll(&mut self, active: bool) -> bool {
        let mut dirty = false;
        let revision = crate::app_center::storage_revision();
        if active && self.revision != revision {
            self.wanted = true;
        }
        if let Some(job) = &mut self.job {
            if job.revision != revision {
                job.cancel.store(true, Ordering::Relaxed);
            }
            loop {
                match job.updates.try_recv() {
                    Ok(update) => {
                        dirty = true;
                        if job.cancel.load(Ordering::Relaxed) {
                            continue;
                        }
                        match update {
                            Update::Disk(disk) => self.disk = Some(disk),
                            Update::Report(result) => {
                                job.finished = true;
                                match result {
                                    Ok(report) => {
                                        self.report = Some(report);
                                        self.snapshot = self.snapshot.wrapping_add(1);
                                        self.error = None;
                                    }
                                    Err(error) => self.error = Some(error),
                                }
                                self.completed = Some(Instant::now());
                                self.revision = job.revision;
                                self.wanted = false;
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        if !job.cancel.load(Ordering::Relaxed) && !job.finished {
                            self.error.get_or_insert_with(|| {
                                "Storage worker stopped; Refresh to retry".into()
                            });
                            self.wanted = false;
                            self.revision = job.revision;
                            self.completed = Some(Instant::now());
                        }
                        self.job = None;
                        dirty = true;
                        break;
                    }
                }
            }
        }
        if active && self.wanted && self.job.is_none() {
            self.wanted = false;
            self.error = None;
            self.start(revision);
            dirty = true;
        }
        dirty
    }
    fn start(&mut self, revision: u64) {
        let (send, updates) = mpsc::sync_channel(2);
        let cancel = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&cancel);
        let result = std::thread::Builder::new()
            .name("storage-scan".into())
            .spawn(move || {
                let _ = send.send(Update::Disk(disk::query(Path::new("/"))));
                if !stopped.load(Ordering::Relaxed) {
                    let _ = send.send(Update::Report(report(&stopped)));
                }
            });
        match result {
            Ok(_) => {
                self.job = Some(Job {
                    updates,
                    cancel,
                    revision,
                    finished: false,
                });
            }
            Err(error) => {
                self.error = Some(format!("Storage worker unavailable: {error}"));
                self.revision = revision;
            }
        }
    }
}
