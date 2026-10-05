//! Explicit filesystem operations. Symlinks are never followed by file mutations.
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

pub const COPY_BUFFER: usize = 64 * 1024;
/// Guard regular-file opens against devices, FIFOs, and final-component symlinks.
/// # Errors
/// Reports open/type errors without blocking on a FIFO.
pub fn open_regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    let _read_options = options.read(true);
    #[cfg(unix)]
    let _guarded_options = options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Expected a regular file"));
    }
    Ok(file)
}
/// # Errors
/// Rejects roots, dot entries, parent traversal, and noncanonical parent aliases.
pub fn checked_path(path: &Path) -> io::Result<PathBuf> {
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || path.as_os_str().as_encoded_bytes().ends_with(b"/.")
    {
        return Err(io::Error::other(
            "Dot and parent entries cannot be modified",
        ));
    }
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("A file name is required"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(parent.canonicalize()?.join(name))
}
/// # Errors
/// Requires one nonempty name without separators, dot entries, or control characters.
pub fn named(parent: &Path, name: &str) -> io::Result<PathBuf> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(io::Error::other(
            "Use one file name without slashes or dot entries",
        ));
    }
    checked_path(&parent.join(name))
}
fn absent(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Destination already exists",
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
/// # Errors
/// Rejects collisions including dangling symlinks.
pub fn create_folder(parent: &Path, name: &str) -> io::Result<()> {
    fs::create_dir(named(parent, name)?)
}
/// # Errors
/// Rejects collisions, self moves, and moving a directory into itself.
pub fn move_entry(source: &Path, destination: &Path) -> io::Result<()> {
    let (source_path, destination_path) = operation_paths(source, destination)?;
    rename_new(&source_path, &destination_path)
}
fn operation_paths(source: &Path, destination: &Path) -> io::Result<(PathBuf, PathBuf)> {
    let source_path = checked_path(source)?;
    let destination_path = checked_path(destination)?;
    let metadata = fs::symlink_metadata(&source_path)?;
    if source_path == destination_path
        || (metadata.is_dir() && destination_path.starts_with(&source_path))
    {
        return Err(io::Error::other("Destination must be outside the source"));
    }
    absent(&destination_path)?;
    Ok((source_path, destination_path))
}
/// Atomic no-replace rename on supported native hosts. Never falls back to overwriting.
/// # Errors
/// Returns filesystem/encoding errors, including cross-filesystem moves.
pub fn rename_new(source: &Path, destination: &Path) -> io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source_name = CString::new(source.as_os_str().as_bytes())?;
    let destination_name = CString::new(destination.as_os_str().as_bytes())?;
    // SAFETY: both pointers are live NUL-terminated path strings. The syscall
    // does not retain them. Exclusive rename is required to close collision races.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source_name.as_ptr(),
            libc::AT_FDCWD,
            destination_name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let result = {
        // SAFETY: both pointers are live NUL-terminated path strings. renamex_np
        // does not retain them; RENAME_EXCL atomically rejects an existing destination.
        unsafe {
            libc::renamex_np(
                source_name.as_ptr(),
                destination_name.as_ptr(),
                libc::RENAME_EXCL,
            )
        }
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Exclusive rename unavailable on this host",
    ));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Create a private staging file adjacent to the destination; cleaned on drop.
#[derive(Debug)]
pub struct Temporary {
    pub path: PathBuf,
}
impl Temporary {
    /// # Errors
    /// Reports exhausted names or unwritable destination directories.
    pub fn file(parent: &Path) -> io::Result<(Self, File)> {
        for _ in 0_i32..32_i32 {
            let path = temporary_name(parent);
            let mut options = OpenOptions::new();
            let _exclusive_options = options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            let _private_options = options.mode(0o600);
            match options.open(&path) {
                Ok(file) => return Ok((Self { path }, file)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other("Cannot allocate staging file"))
    }
}
fn temporary_name(parent: &Path) -> PathBuf {
    // Counter only disambiguates names; create_new / mkdir is the security boundary.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    parent.join(format!(
        ".vitrallis-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}
impl Drop for Temporary {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_file(&self.path)
            && e.kind() != io::ErrorKind::NotFound
        {
            eprintln!("Cannot remove temporary file {}: {e}", self.path.display());
        }
    }
}

/// Copy a regular file by streaming into an adjacent temporary, then publish exclusively.
/// Directory copy is explicit, bounded in depth/count, and rolls its private staging tree back.
/// # Errors
/// Rejects symlinks, special files, collisions, changing input, excessive recursion and I/O errors.
pub fn copy_entry(
    source: &Path,
    destination: &Path,
    progress: &mut impl FnMut(u64),
) -> io::Result<()> {
    let (source_path, destination_path) = operation_paths(source, destination)?;
    let parent = destination_path
        .parent()
        .ok_or_else(|| io::Error::other("No destination directory"))?;
    let metadata = fs::symlink_metadata(&source_path)?;
    if metadata.is_file() {
        return copy_file(&source_path, &destination_path, progress);
    }
    if !metadata.is_dir() {
        return Err(io::Error::other(
            "Copy supports regular files and directories, not symlinks or devices",
        ));
    }
    let stage = temporary_name(parent);
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let _private_builder = builder.mode(0o700);
    builder.create(&stage)?;
    let result = (|| {
        let mut count = 0;
        let mut bytes = 0_u64;
        copy_tree(&source_path, &stage, 0, &mut count, &mut |n| {
            // Display progress caps at u64::MAX if the tree's aggregate size
            // exceeds a byte counter; each individual copy is checked below.
            bytes = bytes.saturating_add(n);
            progress(bytes);
        })?;
        rename_new(&stage, &destination_path)
    })();
    if let Err(error) = result {
        return match remove_staged_tree(&stage) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::new(
                error.kind(),
                format!("{error}; staging cleanup failed: {cleanup}"),
            )),
        };
    }
    Ok(())
}
fn copy_tree(
    source: &Path,
    destination: &Path,
    depth: usize,
    count: &mut usize,
    progress: &mut impl FnMut(u64),
) -> io::Result<()> {
    if depth >= 64 {
        return Err(io::Error::other("Copy exceeds 64 directory levels"));
    }
    let before = fs::symlink_metadata(source)?;
    if !before.is_dir() {
        return Err(io::Error::other("Source directory changed during copy"));
    }
    for pending_entry in fs::read_dir(source)? {
        *count = count
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Copy entry count overflow"))?;
        if *count > 100_000 {
            return Err(io::Error::other("Copy exceeds 100,000 entries"));
        }
        let entry = pending_entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            fs::create_dir(&target)?;
            copy_tree(
                &entry.path(),
                &target,
                depth.saturating_add(1),
                count,
                progress,
            )?;
        } else if kind.is_file() {
            let mut previous = 0;
            copy_file(&entry.path(), &target, &mut |n| {
                progress(n.saturating_sub(previous));
                previous = n;
            })?;
        } else {
            return Err(io::Error::other(
                "Symlink or special file encountered; copy cancelled",
            ));
        }
    }
    if !same_snapshot(&before, &fs::symlink_metadata(source)?) {
        return Err(io::Error::other("Source directory changed during copy"));
    }
    #[cfg(unix)]
    fs::set_permissions(
        destination,
        fs::Permissions::from_mode(before.mode() & 0o777),
    )?;
    Ok(())
}
// Only the freshly created, bounded private staging tree reaches this function.
// Restore owner access before rollback, including copied read-only directories.
fn remove_staged_tree(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    for pending_entry in fs::read_dir(path)? {
        let entry = pending_entry?;
        if entry.file_type()?.is_dir() {
            remove_staged_tree(&entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    fs::remove_dir(path)
}
fn copy_file(source: &Path, destination: &Path, progress: &mut impl FnMut(u64)) -> io::Result<()> {
    let mut input = open_regular(source)?;
    let before = input.metadata()?;
    let (temporary, mut output) = Temporary::file(
        destination
            .parent()
            .ok_or_else(|| io::Error::other("Missing parent"))?,
    )?;
    let mut buffer = vec![0; COPY_BUFFER];
    let mut copied = 0_u64;
    let buffer_length = u64::try_from(buffer.len()).map_err(io::Error::other)?;
    loop {
        // One extra byte detects growth without following an endlessly appended log.
        let remaining = before.len().saturating_add(1).saturating_sub(copied);
        let limit = usize::try_from(remaining.min(buffer_length)).map_err(io::Error::other)?;
        let chunk = buffer
            .get_mut(..limit)
            .ok_or_else(|| io::Error::other("Invalid copy buffer length"))?;
        let count = match input.read(chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        output.write_all(
            buffer
                .get(..count)
                .ok_or_else(|| io::Error::other("Invalid copy read count"))?,
        )?;
        copied = copied
            .checked_add(u64::try_from(count).map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("Copy length overflow"))?;
        if copied > before.len() {
            return Err(io::Error::other("Source grew during copy"));
        }
        progress(copied);
    }
    if !same_snapshot(&before, &input.metadata()?) || copied != before.len() {
        return Err(io::Error::other("Source changed during copy"));
    }
    #[cfg(unix)]
    output.set_permissions(fs::Permissions::from_mode(before.mode() & 0o777))?;
    output.sync_all()?;
    drop(output);
    rename_new(&temporary.path, destination)
}
/// Delete is nonrecursive: nonempty folders must be emptied explicitly.
/// The boolean must come from a Cancel-default confirmation for this exact path.
/// # Errors
/// Rejects unsafe paths, missing files, unconfirmed requests, and nonempty directories.
pub fn delete(path: &Path, confirmed: bool) -> io::Result<()> {
    if !confirmed {
        return Err(io::Error::other("Deletion was not confirmed"));
    }
    let checked = checked_path(path)?;
    let metadata = fs::symlink_metadata(&checked)?;
    if metadata.is_dir() {
        fs::remove_dir(checked)
    } else {
        fs::remove_file(checked)
    }
}
/// Compare identity and modification metadata for safe save/copy conflict detection.
#[must_use]
pub fn same_snapshot(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.size() == b.size()
            && a.mtime() == b.mtime()
            && a.mtime_nsec() == b.mtime_nsec()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        a.len() == b.len() && a.modified().ok() == b.modified().ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handler {
    Text,
    Properties,
}
/// Small bounded content dispatch. Executable files are never launched by selection.
/// # Errors
/// Reports unreadable files; special files dispatch to properties without opening.
pub fn handler(path: &Path) -> io::Result<Handler> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Ok(Handler::Properties);
    }
    let mut file = open_regular(path)?;
    let mut bytes = [0; 8192];
    let count = file.read(&mut bytes)?;
    let sample = bytes
        .get(..count)
        .ok_or_else(|| io::Error::other("Invalid content sample length"))?;
    if sample.contains(&0) || sample.iter().any(|b| *b < 9 || (13 < *b && *b < 32)) {
        return Ok(Handler::Properties);
    }
    // A UTF-8 code point may cross the sample boundary; allow only that incomplete suffix.
    let utf8 = match std::str::from_utf8(sample) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none() && count == 8192,
    };
    Ok(if utf8 {
        Handler::Text
    } else {
        Handler::Properties
    })
}
/// # Errors
/// Reports unreadable metadata.
pub fn properties(path: &Path) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    let kind = if metadata.is_dir() {
        "Directory"
    } else if metadata.is_symlink() {
        "Symbolic link"
    } else if metadata.is_file() {
        "Regular file"
    } else {
        "Special file"
    };
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_or_else(
            |error| format!("before 1970 ({} seconds)", error.duration().as_secs()),
            |d| format!("{} seconds since 1970 UTC", d.as_secs()),
        );
    let mut value = format!(
        "{}\nType: {kind}\nSize: {} bytes\nModified: {modified}",
        path.display(),
        metadata.len()
    );
    #[cfg(unix)]
    {
        use std::fmt::Write as _;
        write!(value, "\nPermissions: {:03o}", metadata.mode() & 0o777)
            .map_err(io::Error::other)?;
    }
    Ok(value)
}

/// OS no-follow flag for safe `OpenOptions` call sites outside the FFI boundary.
pub const NOFOLLOW: i32 = libc::O_NOFOLLOW;
