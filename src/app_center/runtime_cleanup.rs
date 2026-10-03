//! Bounded removal of Shell-owned dependency generations; custom environments remain.
use super::{
    metadata, storage,
    transaction::{self, Write},
};
use std::{fs, path::Path};

const BYTE_LIMIT: usize = 64 * 1024 * 1024;
const ENTRY_LIMIT: usize = 4096;

fn generation_name(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

pub(super) fn owned_file(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root.join("runtime")) else {
        return false;
    };
    let mut components = relative.components();
    components
        .next()
        .is_some_and(|c| generation_name(c.as_os_str()))
        && components.next().is_some()
        && relative
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

pub(super) fn plan(root: &Path, writes: &mut Vec<Write>) -> Result<(), String> {
    let base = root.join("runtime");
    storage::safe(&base)?;
    let entries = match fs::read_dir(&base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = 0;
    let mut count = 0;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        count += 1;
        if count > ENTRY_LIMIT {
            return Err("App runtime exceeds removal bounds; no files were removed".into());
        }
        // Publishers cannot supply runtime/ paths. Only the content-addressed
        // generations created by ensure() belong to Shell; other entries stay.
        let name = entry.file_name();
        if !generation_name(&name) {
            continue;
        }
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            return Err("Unsafe runtime generation; no files were removed".into());
        }
        walk(&entry.path(), &mut count, &mut |path| {
            if writes.len() >= transaction::WRITE_LIMIT - 1 {
                return Err("App runtime has too many files; no files were removed".into());
            }
            let before = storage::read(path, metadata::BUNDLE_LIMIT.min(BYTE_LIMIT - bytes))?
                .ok_or("Runtime changed while preparing removal")?;
            bytes += before.bytes.len();
            writes.push(Write {
                path: path.to_path_buf(),
                before: Some(before),
                after: None,
            });
            Ok(())
        })?;
    }
    Ok(())
}

// File transactions retain empty directories so interrupted removal can restore
// bytes and modes without recreating the runtime's private directory hierarchy.
// Provisioning may discard that empty hierarchy before creating a fresh venv.
pub(super) fn remove_empty(root: &Path) -> Result<bool, String> {
    let mut count = 0;
    let mut nonempty = false;
    let directories = walk(root, &mut count, &mut |_| {
        nonempty = true;
        Ok(())
    })?;
    if nonempty {
        return Ok(false);
    }
    for path in directories.iter().rev() {
        storage::safe(path)?;
        fs::remove_dir(path).map_err(|e| e.to_string())?;
        storage::sync(path.parent().ok_or("Missing runtime parent")?)?;
    }
    Ok(true)
}

fn walk(
    root: &Path,
    count: &mut usize,
    file: &mut impl FnMut(&Path) -> Result<(), String>,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut pending = vec![(root.to_path_buf(), 0)];
    let mut directories = Vec::new();
    while let Some((path, depth)) = pending.pop() {
        *count += 1;
        if *count > ENTRY_LIMIT || depth > 32 {
            return Err("App runtime exceeds removal bounds; no files were removed".into());
        }
        storage::safe(&path)?;
        let info = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if info.is_file() {
            file(&path)?;
        } else if info.is_dir() {
            for entry in fs::read_dir(&path).map_err(|e| e.to_string())? {
                if pending.len() + *count >= ENTRY_LIMIT {
                    return Err("App runtime exceeds removal bounds; no files were removed".into());
                }
                pending.push((entry.map_err(|e| e.to_string())?.path(), depth + 1));
            }
            directories.push(path);
        } else {
            return Err("Unsafe runtime entry; no files were removed".into());
        }
    }
    Ok(directories)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_empty_safe_hierarchies_can_be_reprovisioned() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch
            .0
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("runtime");
        let library = root.join("lib/python/site-packages");
        storage::directory(&library)?;
        let note = library.join("user-note.txt");
        fs::write(&note, "keep this").map_err(|e| e.to_string())?;
        assert!(!remove_empty(&root)?);
        assert_eq!(
            fs::read_to_string(&note).map_err(|e| e.to_string())?,
            "keep this"
        );
        fs::remove_file(&note).map_err(|e| e.to_string())?;
        assert!(remove_empty(&root)?);
        assert!(!root.exists());
        Ok(())
    }
}
