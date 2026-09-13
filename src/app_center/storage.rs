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
            out.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
            out.push(char::from(b"0123456789abcdef"[usize::from(byte & 15)]));
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
    let mut f = match File::open(path) {
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
    Read::by_ref(&mut f)
        .take(u64::try_from(limit).map_err(|e| e.to_string())? + 1)
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
    safe(path)?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    // Apply to every new ancestor as well, even with a group-writable umask.
    // Existing directories retain their permissions and must pass safe().
    #[cfg(unix)]
    builder.mode(0o755);
    builder.create(path).map_err(|e| e.to_string())?;
    safe(path)
}
pub fn sync(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
pub fn atomic(path: &Path, data: &FileData) -> Result<(), String> {
    safe(path)?;
    let parent = path.parent().ok_or("Missing parent")?;
    directory(parent)?;
    let tmp = parent.join(format!(
        ".app-center-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut f = options.open(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&data.bytes).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        f.set_permissions(fs::Permissions::from_mode(data.mode))
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        safe(path)?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        sync(parent)
    })();
    if tmp.exists() {
        let _ = fs::remove_file(tmp);
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
        opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        opts.mode(0o600);
        let file = opts.open(path).map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|e| format!("Another Vitrallis storage operation is active: {e}"))?;
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
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
    pub fn root(&self, p: &super::metadata::Package) -> PathBuf {
        self.data.join("vitrallis/apps").join(&p.id)
    }
}
