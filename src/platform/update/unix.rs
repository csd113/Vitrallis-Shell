//! Same-filesystem, locked, durable replacement. Only the running shell is targeted.
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Installation {
    target: PathBuf,
    stage: PathBuf,
    original: fs::Metadata,
    // Held through cleanup. Never unlink the lock inode: waiters must share it.
    _lock: File,
}
impl Installation {
    pub fn current() -> Result<Self, String> {
        let path = std::env::current_exe().map_err(|e| format!("Locate shell: {e}"))?;
        #[cfg(target_os = "linux")]
        {
            let running = fs::metadata("/proc/self/exe").map_err(|e| e.to_string())?;
            let installed = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !same_file(&running, &installed) {
                return Err("Shell changed on disk; relaunch before updating".into());
            }
        }
        Self::open(&path).map_err(|e| format!("Cannot stage shell update: {e}"))
    }

    fn open(target: &Path) -> io::Result<Self> {
        let original = fs::symlink_metadata(target)?;
        if !original.is_file()
            || original.nlink() != 1
            || original.mode() & 0o7022 != 0
            || original.mode() & 0o111 == 0
        {
            return Err(io::Error::other(
                "shell must be a regular executable without hard links or unsafe permissions",
            ));
        }
        let parent = target
            .parent()
            .ok_or_else(|| io::Error::other("shell has no parent directory"))?
            .canonicalize()?;
        let parent_meta = fs::metadata(&parent)?;
        if parent_meta.uid() != original.uid() || parent_meta.mode() & 0o022 != 0 {
            return Err(io::Error::other(
                "installation directory ownership/permissions are unsafe",
            ));
        }
        for ancestor in parent.ancestors() {
            let metadata = fs::symlink_metadata(ancestor)?;
            // Sticky shared ancestors (e.g. /tmp) protect private child directories.
            if !metadata.is_dir()
                || (metadata.uid() != 0 && metadata.uid() != original.uid())
                || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
            {
                return Err(io::Error::other(
                    "installation ancestor is writable by other users",
                ));
            }
        }
        let stage = parent.join(".vitrallis-shell-update");
        match fs::DirBuilder::new().mode(0o700).create(&stage) {
            Ok(()) => File::open(&parent)?.sync_all()?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(&stage)?;
        if !metadata.is_dir() || metadata.uid() != original.uid() || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other("unsafe update staging directory"));
        }
        let lock_path = stage.join("lock");
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lock_path)
        {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&lock_path)?;
                if !metadata.is_file()
                    || metadata.nlink() != 1
                    || metadata.uid() != original.uid()
                    || metadata.mode() & 0o077 != 0
                {
                    return Err(io::Error::other("unsafe update lock"));
                }
                OpenOptions::new().read(true).write(true).open(&lock_path)?
            }
            Err(error) => return Err(error),
        };
        lock.try_lock()
            .map_err(|_| io::Error::other("another shell update is in progress"))?;
        // A previous process may have died during download. Its payload is never reused.
        remove_file(&stage.join("download"))?;
        remove_file(&stage.join("previous-next"))?;
        let target = parent.join(
            target
                .file_name()
                .ok_or_else(|| io::Error::other("invalid executable path"))?,
        );
        Ok(Self {
            target,
            stage,
            original,
            _lock: lock,
        })
    }
    pub fn payload(&self) -> Result<File, String> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.stage.join("download"))
            .map_err(|e| format!("Create staging file: {e}"))
    }
    pub fn ready(&self, file: File, version: &semver::Version) -> Result<(), String> {
        file.set_permissions(fs::Permissions::from_mode(self.original.mode() & 0o777))
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("Prepare replacement: {e}"))?;
        drop(file); // Linux refuses exec while a writable descriptor is open.
        let path = self.stage.join("download");
        let output = super::super::command::run(
            path.to_str().ok_or("Installation path is not UTF-8")?,
            &["--version"],
        )
        .map_err(|error| format!("Replacement cannot run: {error}"))?;
        if output.trim() != format!("vitrallis {version}") {
            return Err("Replacement version does not match the release".into());
        }
        Ok(())
    }
    /// Returns false only when replacement succeeded but directory sync failed.
    pub fn commit(&self) -> Result<bool, String> {
        self.replace()
            .map_err(|e| format!("Shell installation failed: {e}"))
    }
    fn replace(&self) -> io::Result<bool> {
        self.unchanged()?;
        self.backup()?;
        let parent = self
            .target
            .parent()
            .ok_or_else(|| io::Error::other("missing installation directory"))?;
        self.unchanged()?;
        // This is the only operation that changes the installed executable.
        fs::rename(self.stage.join("download"), &self.target)?;
        Ok(File::open(parent)
            .and_then(|dir| dir.sync_all())
            .and_then(|()| File::open(&self.stage)?.sync_all())
            .is_ok())
    }
    fn unchanged(&self) -> io::Result<()> {
        let current = fs::symlink_metadata(&self.target)?;
        if !same_file(&self.original, &current)
            || current.len() != self.original.len()
            || current.mtime() != self.original.mtime()
            || current.mtime_nsec() != self.original.mtime_nsec()
        {
            return Err(io::Error::other(
                "shell changed during update; relaunch and retry",
            ));
        }
        Ok(())
    }
    fn backup(&self) -> io::Result<()> {
        // Copy rather than link: a link would change the running inode's link count
        // and make a failed attempt appear unsafe on retry.
        let backup = self.stage.join("previous-next");
        let mut source = File::open(&self.target)?;
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&backup)?;
        io::copy(&mut source, &mut destination)?;
        destination.set_permissions(fs::Permissions::from_mode(self.original.mode() & 0o777))?;
        destination.sync_all()?;
        fs::rename(&backup, self.stage.join("previous"))?;
        File::open(&self.stage)?.sync_all()
    }
}
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_file() && right.is_file() && left.dev() == right.dev() && left.ino() == right.ino()
}
fn remove_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
impl Drop for Installation {
    fn drop(&mut self) {
        for name in ["download", "previous-next"] {
            if let Err(error) = remove_file(&self.stage.join(name)) {
                eprintln!("level=warning event=shell_update_cleanup message={error:?}");
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
