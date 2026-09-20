//! Bounded streaming traversal of explicitly owned roots, never the whole disk.
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::BTreeSet,
    fs::{self, Metadata},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const MAX_ENTRIES: usize = 200_000;
const MAX_DEPTH: usize = 64;
const MAX_TIME: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Size {
    pub bytes: u64,
    pub incomplete: bool,
    pub issue: Option<String>,
}
impl Size {
    pub fn fail(&mut self, error: impl std::fmt::Display) {
        self.incomplete = true;
        if self.issue.is_none() {
            self.issue = Some(error.to_string());
        }
    }
    pub fn add(&mut self, other: &Self) {
        self.bytes = self.bytes.saturating_add(other.bytes);
        if other.incomplete {
            self.fail(other.issue.as_deref().unwrap_or("Some files unavailable"));
        }
    }
    pub fn label(&self) -> String {
        if self.incomplete && self.bytes == 0 {
            "Unavailable".into()
        } else {
            format!(
                "{}{}",
                if self.incomplete { ">= " } else { "" },
                super::format_bytes(self.bytes)
            )
        }
    }
}

pub struct Scanner<'a> {
    cancel: &'a AtomicBool,
    started: Instant,
    entries: usize,
    root_device: Option<u64>,
    pub root_bytes: u64,
    roots: Vec<PathBuf>,
    // Only multiply-linked files need identities retained; memory is bounded by MAX_ENTRIES.
    links: BTreeSet<(u64, u64)>,
}
impl<'a> Scanner<'a> {
    pub fn new(cancel: &'a AtomicBool) -> Self {
        Self {
            cancel,
            started: Instant::now(),
            entries: 0,
            root_device: fs::metadata("/").ok().map(|m| identity(&m).0),
            root_bytes: 0,
            roots: Vec::new(),
            links: BTreeSet::new(),
        }
    }
    pub fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
            || self.entries >= MAX_ENTRIES
            || self.started.elapsed() >= MAX_TIME
    }
    pub fn measure(&mut self, path: &Path, optional: bool) -> Size {
        let mut size = Size::default();
        self.walk(path, optional, &mut |_, bytes| {
            size.bytes = size.bytes.saturating_add(bytes);
        })
        .into_iter()
        .for_each(|error| size.fail(error));
        size
    }
    pub fn walk(
        &mut self,
        root: &Path,
        optional: bool,
        visit: &mut impl FnMut(&Path, u64),
    ) -> Option<String> {
        if self.stopped() {
            return Some("Scan cancelled or limit reached; Refresh to retry".into());
        }
        if !root.is_absolute()
            || root
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Some("Unsafe storage location".into());
        }
        if self.roots.iter().any(|p| root.starts_with(p)) {
            return None;
        }
        // Reject symlinks in root ancestors as well as entries encountered below.
        for parent in root.ancestors().skip(1) {
            match fs::symlink_metadata(parent) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Some("Symlinked storage location skipped".into());
                }
                Ok(_) => (),
                Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => return None,
                Err(e) => return Some(e.to_string()),
            }
        }
        let metadata = match fs::symlink_metadata(root) {
            Ok(m) => m,
            Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => return Some(e.to_string()),
        };
        if metadata.file_type().is_symlink() {
            return Some("Symlinked storage root skipped".into());
        }
        let device = identity(&metadata).0;
        let mut issue = None;
        self.entry(root, &metadata, device, 0, visit, &mut issue);
        self.roots.push(root.to_owned());
        issue
    }
    fn entry(
        &mut self,
        path: &Path,
        metadata: &Metadata,
        device: u64,
        depth: usize,
        visit: &mut impl FnMut(&Path, u64),
        issue: &mut Option<String>,
    ) {
        if self.stopped() || depth >= MAX_DEPTH {
            issue.get_or_insert_with(|| "Scan cancelled or limit reached; Refresh to retry".into());
            return;
        }
        self.entries += 1;
        if self.roots.iter().any(|root| path.starts_with(root)) {
            return;
        }
        if identity(metadata).0 != device {
            issue.get_or_insert_with(|| "Nested mount skipped".into());
            return;
        }
        if metadata.is_file() && multiple_links(metadata) && !self.links.insert(identity(metadata))
        {
            return;
        }
        // Symlinks contribute only their own allocation, never their target.
        let bytes = allocated(metadata);
        if self.root_device == Some(device) {
            self.root_bytes = self.root_bytes.saturating_add(bytes);
        }
        visit(path, bytes);
        if !metadata.is_dir() {
            return;
        }
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) => {
                issue.get_or_insert_with(|| e.to_string());
                return;
            }
        };
        for item in entries {
            if self.stopped() {
                issue.get_or_insert_with(|| {
                    "Scan cancelled or limit reached; Refresh to retry".into()
                });
                break;
            }
            let result = item.and_then(|item| {
                // Recheck the directory before descending if it changed into a link/mount.
                let current = fs::symlink_metadata(path)?;
                if !current.is_dir() || identity(&current) != identity(metadata) {
                    return Err(std::io::Error::other("Directory changed during scan"));
                }
                let child = item.path();
                let metadata = fs::symlink_metadata(&child)?;
                self.entry(&child, &metadata, device, depth + 1, visit, issue);
                Ok(())
            });
            if let Err(e) = result {
                issue.get_or_insert_with(|| e.to_string());
            }
        }
    }
}
#[cfg(unix)]
fn identity(m: &Metadata) -> (u64, u64) {
    (m.dev(), m.ino())
}
#[cfg(not(unix))]
fn identity(_: &Metadata) -> (u64, u64) {
    (0, 0)
}
#[cfg(unix)]
fn multiple_links(m: &Metadata) -> bool {
    m.nlink() > 1
}
#[cfg(not(unix))]
fn multiple_links(_: &Metadata) -> bool {
    false
}
#[cfg(unix)]
fn allocated(m: &Metadata) -> u64 {
    m.blocks().saturating_mul(512)
}
#[cfg(not(unix))]
fn allocated(m: &Metadata) -> u64 {
    m.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn work_depth_and_time_limits_are_explicit_partial_results() {
        let scratch = crate::test_support::Scratch::new().unwrap();
        let root = scratch.0.canonicalize().unwrap();
        let cancel = AtomicBool::new(false);
        let mut scanner = Scanner::new(&cancel);
        scanner.entries = MAX_ENTRIES;
        assert!(scanner.measure(&root, false).incomplete);
        scanner.entries = 0;
        scanner.started = Instant::now().checked_sub(MAX_TIME).unwrap();
        assert!(scanner.measure(&root, false).incomplete);
        let mut path = root.clone();
        for _ in 0..MAX_DEPTH {
            path.push("nested");
            std::fs::create_dir(&path).unwrap();
        }
        assert!(Scanner::new(&cancel).measure(&root, false).incomplete);
    }
}
