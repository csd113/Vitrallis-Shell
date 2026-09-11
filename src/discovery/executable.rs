//! Resolve executable files in working-directory and PATH order.
use std::path::{Path, PathBuf};

pub(super) fn resolve(program: &str, cwd: &Path, search: &[PathBuf]) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(cwd.join(program)).filter(|p| executable(p));
    }
    search
        .iter()
        .map(|p| p.join(program))
        .find(|p| executable(p))
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}
