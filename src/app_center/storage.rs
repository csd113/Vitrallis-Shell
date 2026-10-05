//! Bounded regular-file IO and durable conditional writes. No symlink traversal.
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileData {
    pub bytes: Vec<u8>,
    pub mode: u32,
}
pub fn sha(bytes: &[u8]) -> String {
    {
        let digest = Sha256::digest(bytes);
        let mut out = String::with_capacity(64);
        for byte in digest {
            for nibble in [byte >> 4, byte & 15] {
                out.push(char::from(if nibble < 10 {
                    b'0'.saturating_add(nibble)
                } else {
                    b'a'.saturating_add(nibble.saturating_sub(10))
                }));
            }
        }
        out
    }
}
pub fn safe(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        || path
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .any(|b| *b < 32 || *b == 127)
    {
        return Err("Unsafe absolute storage path".into());
    }
    for parent in path.ancestors() {
        match fs::symlink_metadata(parent) {
            Ok(m) => {
                if m.file_type().is_symlink() || (parent != path && !m.is_dir()) {
                    return Err(format!("Unsafe path: {}", parent.display()));
                }
                #[cfg(unix)]
                if (m.is_file() && m.nlink() != 1)
                    || (m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0)
                {
                    return Err(format!("Unsafe links/permissions: {}", parent.display()));
                }
                if parent == path && !m.is_file() && !m.is_dir() {
                    return Err("Special files are forbidden".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
pub fn read(path: &Path, limit: usize) -> Result<Option<FileData>, String> {
    safe(path)?;
    let mut options = OpenOptions::new();
    let _read_options = options.read(true);
    // Prevent a final-component substitution from opening a symlink or blocking
    // on a FIFO between safe() and the descriptor's metadata check.
    #[cfg(unix)]
    let _guarded_options = options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let mut f = match options.open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let m = f.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > u64::try_from(limit).map_err(|e| e.to_string())? {
        return Err(format!("Not a bounded regular file: {}", path.display()));
    }
    #[cfg(unix)]
    {
        let current = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if m.ino() != current.ino() || m.dev() != current.dev() || m.nlink() != 1 {
            return Err("File changed during read".into());
        }
    }
    let mut bytes = Vec::new();
    let _bytes_read = Read::by_ref(&mut f)
        .take(
            u64::try_from(limit)
                .map_err(|e| e.to_string())?
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("File grew during read".into());
    }
    #[cfg(unix)]
    let mode = m.mode() & 0o777;
    #[cfg(not(unix))]
    let mode = 0o644;
    Ok(Some(FileData { bytes, mode }))
}
pub fn directory(path: &Path) -> Result<(), String> {
    directory_mode(path, 0o755)
}
pub fn private_data(path: &Path, home: &Path) -> Result<(), String> {
    safe(path)?;
    match fs::symlink_metadata(path) {
        Ok(info) => {
            if !info.is_dir() {
                return Err("App data must be a directory".into());
            }
            #[cfg(unix)]
            {
                let owner = fs::symlink_metadata(home).map_err(|e| e.to_string())?;
                if info.uid() != owner.uid() || info.mode() & 0o077 != 0 {
                    return Err(
                        "App data must be private and owned by the home directory user".into(),
                    );
                }
            }
            #[cfg(not(unix))]
            let _ = home;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
pub fn private_directory(path: &Path, home: &Path) -> Result<(), String> {
    private_data(path, home)?;
    directory_mode(path, 0o700)?;
    private_data(path, home)
}
fn directory_mode(path: &Path, mode: u32) -> Result<(), String> {
    safe(path)?;
    let mut builder = fs::DirBuilder::new();
    let _recursive_builder = builder.recursive(true);
    // Apply to every new ancestor as well, even with a group-writable umask.
    // Existing directories retain their permissions and must pass safe().
    #[cfg(unix)]
    let _mode_builder = builder.mode(mode);
    #[cfg(not(unix))]
    let _ = mode;
    builder.create(path).map_err(|e| e.to_string())?;
    safe(path)
}
pub fn sync(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
pub fn atomic(path: &Path, data: &FileData) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing parent")?;
    let tmp = parent.join(format!(
        ".app-center-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    atomic_with_temp(path, data, &tmp)
}
pub(super) fn atomic_with_temp(path: &Path, data: &FileData, tmp: &Path) -> Result<(), String> {
    safe(path)?;
    safe(tmp)?;
    let parent = path.parent().ok_or("Missing parent")?;
    if tmp == path || tmp.parent() != Some(parent) {
        return Err("Atomic temporary must be a distinct sibling".into());
    }
    directory(parent)?;
    let mut created = false;
    let result = (|| {
        let mut options = OpenOptions::new();
        let _exclusive_options = options.write(true).create_new(true);
        #[cfg(unix)]
        let _private_options = options.mode(0o600);
        let mut f = options.open(tmp).map_err(|e| e.to_string())?;
        created = true;
        f.write_all(&data.bytes).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        f.set_permissions(fs::Permissions::from_mode(data.mode))
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        safe(path)?;
        fs::rename(tmp, path).map_err(|e| e.to_string())?;
        sync(parent)
    })();
    if created
        && let Err(cleanup) = fs::remove_file(tmp)
        && cleanup.kind() != std::io::ErrorKind::NotFound
    {
        return Err(match result {
            Ok(()) => {
                format!("Atomic write completed, but temporary cleanup failed: {cleanup}")
            }
            Err(error) => format!("{error}; temporary cleanup failed: {cleanup}"),
        });
    }
    result
}
pub struct Lock(File);
impl Lock {
    pub fn take(root: &Path) -> Result<Self, String> {
        directory(root)?;
        let path = root.join("lock");
        safe(&path)?;
        let mut opts = OpenOptions::new();
        let _lock_options = opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        let _guarded_options = opts
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let file = opts.open(&path).map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Storage lock must be a regular file".into());
        }
        safe(&path)?;
        #[cfg(unix)]
        {
            let opened = file.metadata().map_err(|e| e.to_string())?;
            let current = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if opened.ino() != current.ino() || opened.dev() != current.dev() {
                return Err("Storage lock changed during open".into());
            }
        }
        file.try_lock()
            .map_err(|e| format!("Another Vitrallis storage operation is active: {e}"))?;
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        if let Err(error) = self.0.unlock() {
            eprintln!("level=error event=storage_unlock message={error:?}");
        }
    }
}
#[derive(Debug, Clone)]
pub struct Locations {
    pub home: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
    pub sources: PathBuf,
}
impl Locations {
    pub fn current() -> Result<Self, String> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or("HOME must be absolute")?;
        let data = std::env::var_os("XDG_DATA_HOME")
            .map_or_else(|| home.join(".local/share"), PathBuf::from);
        let config =
            std::env::var_os("XDG_CONFIG_HOME").map_or_else(|| home.join(".config"), PathBuf::from);
        for p in [&home, &data, &config] {
            safe(p)?;
        }
        Ok(Self {
            home,
            state: data.join("vitrallis/app-center"),
            sources: config.join("vitrallis/app-center.json"),
            data,
        })
    }
    pub fn apps(&self) -> PathBuf {
        self.home.join("Documents/Vitrallis/Apps")
    }
    pub fn app_data(&self, id: &str) -> Result<PathBuf, String> {
        vitrallis_native::paths::app_data(&self.home, id).map_err(|e| e.to_string())
    }
    pub fn root(&self, p: &super::metadata::Package) -> PathBuf {
        self.apps().join(&p.id)
    }
}
