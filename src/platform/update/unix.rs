//! Locked, immutable build generations; one atomic pointer switches every native binary.
use crate::updater::bundle;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Installation {
    target: PathBuf,
    root: PathBuf,
    stage: PathBuf,
    current: PathBuf,
    originals: Vec<(PathBuf, fs::Metadata)>,
    lock: File,
    prepared: Option<Prepared>,
}
#[derive(Debug)]
struct Prepared {
    generation: PathBuf,
    shell_hash: [u8; 32],
}
impl Installation {
    pub fn current() -> Result<Self, String> {
        let path = std::env::current_exe().map_err(|e| e.to_string())?;
        Self::open(&path).map_err(|e| format!("Cannot stage Vitrallis update: {e}"))
    }
    fn open(target: &Path) -> io::Result<Self> {
        let generation = target
            .parent()
            .ok_or_else(|| io::Error::other("Missing build directory"))?;
        let generations = generation
            .parent()
            .ok_or_else(|| io::Error::other("Missing generations directory"))?;
        let root = generations
            .parent()
            .ok_or_else(|| io::Error::other("Missing installation directory"))?
            .to_path_buf();
        let id = generation
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| io::Error::other("Invalid generation name"))?;
        if target.file_name() != Some(std::ffi::OsStr::new("vitrallis"))
            || generations.file_name() != Some(std::ffi::OsStr::new("generations"))
            || id.len() != 64
            || !id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(io::Error::other(
                "Self-update requires a complete managed Vitrallis bundle installation",
            ));
        }
        let original = fs::symlink_metadata(target)?;
        safe_file(&original, original.uid(), true)?;
        for ancestor in generation.ancestors() {
            safe_directory(ancestor, original.uid(), !ancestor.starts_with(&root))?;
        }
        let current = PathBuf::from("generations").join(id);
        if fs::read_link(root.join("current"))? != current {
            return Err(io::Error::other(
                "Another build is installed; relaunch before updating",
            ));
        }
        match root.join(".installation-pending").symlink_metadata() {
            Ok(_) => {
                return Err(io::Error::other(
                    "Installation needs repair before updating",
                ));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let originals = bundle::BINARIES
            .iter()
            .map(|name| {
                let path = generation.join(name);
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    // beta3.9 and beta4 explicitly bridge the published four-file
                    // format. All incoming updates still require all five files.
                    Err(error)
                        if name == &"arti"
                            && error.kind() == io::ErrorKind::NotFound
                            && matches!(
                                crate::updater::VERSION,
                                "0.1.0-beta3.9" | "0.1.0-beta4"
                            ) =>
                    {
                        return Ok(None);
                    }
                    Err(error) => return Err(error),
                };
                safe_file(&metadata, original.uid(), true)?;
                Ok(Some((path, metadata)))
            })
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        let stage = root.join(".vitrallis-update");
        match fs::DirBuilder::new().mode(0o700).create(&stage) {
            Ok(()) => File::open(&root)?.sync_all()?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        safe_directory(&stage, original.uid(), false)?;
        let lock_path = stage.join("lock");
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc_flags());
        let lock = options.open(lock_path)?;
        safe_file(&lock.metadata()?, original.uid(), false)?;
        lock.try_lock()
            .map_err(|_| io::Error::other("Another installation or update is in progress"))?;
        let installation = Self {
            target: target.to_path_buf(),
            root,
            stage,
            current,
            originals,
            lock,
            prepared: None,
        };
        installation.unchanged()?;
        installation.cleanup()?;
        Ok(installation)
    }
    pub fn payload(&self) -> Result<File, String> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.stage.join("download"))
            .map_err(|e| format!("Create bundle staging file: {e}"))
    }
    pub const fn needs_completion(&self) -> bool {
        self.originals.len() != bundle::BINARIES.len()
    }
    pub fn ready(
        &mut self,
        mut file: File,
        version: &semver::Version,
        target: super::Target,
        digest: [u8; 32],
    ) -> Result<(), String> {
        let path = self.stage.join("generation");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|e| e.to_string())?;
        let shell_hash = bundle::unpack(&mut file, &path, |header| target.verify_header(header))?;
        drop(file);
        for name in bundle::BINARIES {
            let executable = path.join(name);
            let output = super::super::command::run(
                executable
                    .to_str()
                    .ok_or("Installation path is not UTF-8")?,
                &["--version"],
            )?;
            if !version_matches(name, version, &output) {
                return Err(format!("Bundled {name} version does not match the release"));
            }
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        self.prepared = Some(Prepared {
            generation: PathBuf::from("generations").join(hex(&digest)),
            shell_hash,
        });
        Ok(())
    }
    pub fn commit(&self) -> Result<bool, String> {
        self.replace()
            .map_err(|e| format!("Vitrallis installation failed: {e}"))
    }
    fn replace(&self) -> io::Result<bool> {
        let prepared = self
            .prepared
            .as_ref()
            .ok_or_else(|| io::Error::other("Bundle has not passed verification"))?;
        self.unchanged()?;
        let staged = self.stage.join("generation");
        let destination = self.root.join(&prepared.generation);
        if destination.symlink_metadata().is_ok() {
            safe_directory(&destination, self.originals[0].1.uid(), false)?;
            for name in bundle::BINARIES {
                safe_file(
                    &fs::symlink_metadata(destination.join(name))?,
                    self.originals[0].1.uid(),
                    true,
                )?;
                if hash_file(&staged.join(name))? != hash_file(&destination.join(name))? {
                    return Err(io::Error::other(
                        "Existing generation does not match the verified bundle",
                    ));
                }
            }
        } else {
            fs::rename(&staged, &destination)?;
            File::open(self.root.join("generations"))?.sync_all()?;
        }
        replace_pointer(&self.root, "previous", &self.current)?;
        self.unchanged()?;
        // This rename is the only change to the active build. Running processes
        // keep their old executable and resolve companions in that same directory.
        let next = self.root.join(".current-next");
        symlink(&prepared.generation, &next)?;
        fs::rename(&next, self.root.join("current"))?;
        Ok(File::open(&self.root)
            .and_then(|dir| dir.sync_all())
            .is_ok())
    }
    fn unchanged(&self) -> io::Result<()> {
        if fs::read_link(self.root.join("current"))? != self.current {
            return Err(io::Error::other("Active build changed during update"));
        }
        for (path, before) in &self.originals {
            let current = fs::symlink_metadata(path)?;
            if !vitrallis_native::files::same_snapshot(before, &current) {
                return Err(io::Error::other("Installed binaries changed during update"));
            }
        }
        if self.needs_completion() {
            let arti = self.target.with_file_name("arti");
            if !matches!(fs::symlink_metadata(arti), Err(error) if error.kind() == io::ErrorKind::NotFound)
            {
                return Err(io::Error::other(
                    "Installed inventory changed during update",
                ));
            }
        }
        Ok(())
    }
    pub fn relaunch_target(&self) -> Result<super::Relaunch, String> {
        let prepared = self.prepared.as_ref().ok_or("No verified generation")?;
        Ok(super::Relaunch {
            executable: self.root.join(&prepared.generation).join("vitrallis"),
            sha256: prepared.shell_hash,
        })
    }
    fn cleanup(&self) -> io::Result<()> {
        for path in [
            self.stage.join("download"),
            self.root.join(".current-next"),
            self.root.join(".previous-next"),
        ] {
            remove_file(&path)?;
        }
        let generation = self.stage.join("generation");
        match fs::symlink_metadata(&generation) {
            Ok(m) if m.is_dir() => fs::remove_dir_all(generation),
            Ok(_) => Err(io::Error::other("Unsafe generation staging path")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}
fn version_matches(name: &str, version: &semver::Version, output: &str) -> bool {
    if name == "arti" {
        output.lines().next() == Some("Arti 2.6.0")
    } else {
        output.trim() == format!("{name} {version}")
    }
}
// std OpenOptions has no no-follow flag; this existing libc constant is provided
// by the shared native helper so the shell keeps its unsafe-code prohibition.
const fn libc_flags() -> i32 {
    vitrallis_native::files::NOFOLLOW
}
fn safe_file(metadata: &fs::Metadata, uid: u32, executable: bool) -> io::Result<()> {
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.nlink() != 1
        || metadata.mode() & 0o7022 != 0
        || (executable && metadata.mode() & 0o111 == 0)
    {
        return Err(io::Error::other(
            "Unsafe installed file type, ownership, links, or permissions",
        ));
    }
    Ok(())
}
fn safe_directory(path: &Path, uid: u32, shared_ancestor: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || (metadata.uid() != uid && metadata.uid() != 0)
        || (metadata.mode() & 0o022 != 0 && (!shared_ancestor || metadata.mode() & 0o1000 == 0))
    {
        return Err(io::Error::other("Unsafe installation directory"));
    }
    Ok(())
}
fn replace_pointer(root: &Path, name: &str, target: &Path) -> io::Result<()> {
    let next = root.join(format!(".{name}-next"));
    symlink(target, &next)?;
    fs::rename(&next, root.join(name))?;
    File::open(root)?.sync_all()
}
fn remove_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(result, "{byte:02x}");
    }
    result
}
fn hash_file(path: &Path) -> io::Result<[u8; 32]> {
    use std::io::Read;
    let mut file = vitrallis_native::files::open_regular(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > bundle::MAX_BINARY {
        return Err(io::Error::other("Installed executable is too large"));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 16384];
    let mut total = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > metadata.len() {
            return Err(io::Error::other(
                "Installed executable grew during verification",
            ));
        }
        hash.update(&buffer[..count]);
    }
    if !vitrallis_native::files::same_snapshot(&metadata, &file.metadata()?) {
        return Err(io::Error::other(
            "Installed executable changed during verification",
        ));
    }
    Ok(hash.finalize().into())
}
fn relaunch_command(
    target: &super::Relaunch,
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<(Installation, std::process::Command), String> {
    let installation = Installation::open(&target.executable).map_err(|e| e.to_string())?;
    if hash_file(&installation.target).map_err(|e| e.to_string())? != target.sha256 {
        return Err("Installed shell changed since update; relaunch refused".into());
    }
    installation.unchanged().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(&installation.target);
    command.args(args);
    Ok((installation, command))
}
pub(super) fn relaunch(
    target: &super::Relaunch,
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let (_installation, mut command) = relaunch_command(target, args)?;
    Err(format!("Cannot relaunch shell: {}", command.exec()))
}
impl Drop for Installation {
    fn drop(&mut self) {
        if let Err(e) = self.cleanup() {
            eprintln!("Update staging cleanup: {e}");
        }
        if let Err(e) = self.lock.unlock() {
            eprintln!("Update unlock: {e}");
        }
    }
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
