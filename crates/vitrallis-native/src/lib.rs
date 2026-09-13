//! Small shared SDL and filesystem components for the bundled applications.
pub mod browser;
pub mod document;
pub mod files;
pub mod ipc;
pub mod keyboard;
pub mod ui;

/// Executables shipped and updated together. Order is the launcher order.
pub const APPLICATIONS: [Application; 3] = [
    Application {
        id: "io.vitrallis.terminal",
        name: "Terminal",
        executable: "vitrallis-terminal",
    },
    Application {
        id: "io.vitrallis.notepad",
        name: "Notepad",
        executable: "vitrallis-notepad",
    },
    Application {
        id: "io.vitrallis.files",
        name: "Files",
        executable: "vitrallis-files",
    },
];

/// Immutable first-party launcher identity; unrelated to App Center manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Application {
    pub id: &'static str,
    pub name: &'static str,
    pub executable: &'static str,
}

/// Resolve a companion in the same build generation as the running executable.
///
/// # Errors
/// Returns an error when the running executable cannot be located.
pub fn companion(name: &str) -> std::io::Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe
        .parent()
        .ok_or_else(|| std::io::Error::other("executable has no directory"))?
        .join(name))
}

/// Choose an existing home directory without assuming a particular device.
#[must_use]
pub fn home() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_dir())
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
}

#[cfg(test)]
mod tests;
