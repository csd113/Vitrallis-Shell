//! Journaled conditional file replacement. Recovery never overwrites later edits.
use super::{
    metadata,
    storage::{self, FileData},
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
pub(super) const WRITE_LIMIT: usize = 2056;
const BYTE_LIMIT: usize = 96 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct Write {
    pub path: PathBuf,
    pub before: Option<FileData>,
    pub after: Option<FileData>,
}
pub fn plan(path: PathBuf, after: FileData) -> Result<Write, String> {
    let before = storage::read(&path, metadata::BUNDLE_LIMIT)?;
    Ok(Write {
        path,
        before,
        after: Some(after),
    })
}
pub fn remove(path: PathBuf) -> Result<Write, String> {
    let before = storage::read(&path, metadata::FILE_LIMIT)?;
    Ok(Write {
        path,
        before,
        after: None,
    })
}
fn replace(path: &Path, data: Option<&FileData>) -> Result<(), String> {
    if let Some(image) = data {
        return storage::atomic(path, image);
    }
    storage::safe(path)?;
    match std::fs::remove_file(path) {
        Ok(()) => storage::sync(path.parent().ok_or("Missing parent")?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
fn value(data: Option<&FileData>) -> Value {
    data.map_or(
        Value::Null,
        |d| serde_json::json!({"sha256":storage::sha(&d.bytes),"mode":d.mode}),
    )
}
fn record(journal: &Path, writes: &[Write]) -> Result<(), String> {
    validate_bounds(writes)?;
    storage::directory(journal)?;
    let mut rows = Vec::with_capacity(writes.len());
    for write in writes {
        let path = write
            .path
            .to_str()
            .ok_or("App storage paths must be UTF-8")?;
        rows.push(serde_json::json!({"path":path,"before":value(write.before.as_ref()),"after":value(write.after.as_ref())}));
    }
    let bytes = serde_json::to_vec(&rows).map_err(|e| e.to_string())?;
    if bytes.len() > metadata::CATALOG_LIMIT {
        return Err("App journal exceeds recovery bounds; no files were changed".into());
    }
    // Publish staging authority before allocating snapshots. No target can be
    // changed until every image is durable and this becomes pending.json.
    storage::atomic_with_temp(
        &journal.join("staging.json"),
        &FileData { bytes, mode: 0o600 },
        &journal.join("staging.json.tmp"),
    )?;
    for (i, w) in writes.iter().enumerate() {
        if let Some(old) = &w.before {
            storage::atomic_with_temp(
                &journal.join(format!("{i}.before")),
                old,
                &journal.join(format!("{i}.before.tmp")),
            )?;
        }
        if let Some(after) = &w.after {
            storage::atomic_with_temp(
                &journal.join(format!("{i}.after")),
                after,
                &journal.join(format!("{i}.after.tmp")),
            )?;
        }
    }
    std::fs::rename(journal.join("staging.json"), journal.join("pending.json"))
        .map_err(|e| e.to_string())?;
    storage::sync(journal)
}
fn validate_bounds(writes: &[Write]) -> Result<(), String> {
    let bytes = writes.iter().try_fold(0_usize, |bytes, write| {
        [write.before.as_ref(), write.after.as_ref()]
            .into_iter()
            .flatten()
            .try_fold(bytes, |total, data| total.checked_add(data.bytes.len()))
    });
    if writes.len() > WRITE_LIMIT || bytes.is_none_or(|total| total > BYTE_LIMIT) {
        return Err("App transaction exceeds recovery bounds; no files were changed".into());
    }
    Ok(())
}
// Finalization is part of the transaction. A rolled-back update must remain
// discoverable; only an unresolved rollback retains the incomplete marker.
pub fn commit(journal: &Path, writes: &[Write], marker: &Path) -> Result<(), String> {
    let marker_data = storage::read(marker, 1024)?.ok_or("Missing transaction marker")?;
    let result = apply_validated(
        journal,
        writes,
        |_| Ok(()),
        || {
            std::fs::remove_file(marker).map_err(|e| e.to_string())?;
            storage::sync(marker.parent().ok_or("Missing marker parent")?)
        },
    );
    if let Err(error) = result {
        match recover(journal, |p| writes.iter().any(|w| w.path == p)) {
            Ok(_) => {
                match std::fs::remove_file(marker) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(format!("{error}; marker cleanup failed: {e}")),
                }
                let parent = marker
                    .parent()
                    .ok_or_else(|| format!("{error}; missing marker parent"))?;
                storage::sync(parent).map_err(|sync_error| {
                    format!("{error}; marker cleanup sync failed: {sync_error}")
                })?;
            }
            Err(recovery_error) => {
                storage::atomic(marker, &marker_data).map_err(|restore_error| format!("{error}; recovery failed: {recovery_error}; marker restore failed: {restore_error}"))?;
                return Err(format!("{error}; recovery failed: {recovery_error}"));
            }
        }
        return Err(error);
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn apply_with(
    journal: &Path,
    writes: &[Write],
    after_write: impl FnMut(usize) -> Result<(), String>,
) -> Result<(), String> {
    apply_validated(journal, writes, after_write, || Ok(()))
}
fn apply_validated(
    journal: &Path,
    writes: &[Write],
    mut after_write: impl FnMut(usize) -> Result<(), String>,
    finalize: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if storage::read(&journal.join("pending.json"), metadata::CATALOG_LIMIT)?.is_some()
        || storage::read(&journal.join("staging.json"), metadata::CATALOG_LIMIT)?.is_some()
    {
        return Err("Pending transaction must be recovered first".into());
    }
    for w in writes {
        if storage::read(&w.path, metadata::BUNDLE_LIMIT)? != w.before {
            return Err("Files changed; check for updates again".into());
        }
    }
    record(journal, writes)?;
    let result: Result<(), String> = (|| {
        for (i, w) in writes.iter().enumerate() {
            if storage::read(&w.path, metadata::BUNDLE_LIMIT)? != w.before {
                return Err("File changed during installation".into());
            }
            replace(&w.path, w.after.as_ref())?;
            after_write(i)?;
        }
        for w in writes {
            if storage::read(&w.path, metadata::BUNDLE_LIMIT)? != w.after {
                return Err("Installed files failed final verification".into());
            }
        }
        finalize()?;
        std::fs::rename(journal.join("pending.json"), journal.join("completed.json"))
            .map_err(|e| e.to_string())?;
        storage::sync(journal)?;
        Ok(())
    })();
    if let Err(error) = result {
        if journal.join("completed.json").exists() {
            std::fs::rename(journal.join("completed.json"), journal.join("pending.json"))
                .map_err(|e| format!("{error}; recovery journal could not be restored: {e}"))?;
        }
        let rollback = rollback(writes);
        return Err(format!(
            "{error}; {}",
            rollback.map_or_else(|e| e, |()| "rolled back; repair on next check".into())
        ));
    }
    Ok(())
}
fn rollback(writes: &[Write]) -> Result<(), String> {
    let mut conflicts = Vec::new();
    for w in writes.iter().rev() {
        let current = storage::read(&w.path, metadata::BUNDLE_LIMIT)?;
        if current == w.before {
            continue;
        }
        if current != w.after {
            conflicts.push(w.path.display().to_string());
            continue;
        }
        if let Some(old) = &w.before {
            storage::atomic(&w.path, old)?;
        } else {
            std::fs::remove_file(&w.path).map_err(|e| e.to_string())?;
            storage::sync(w.path.parent().ok_or("Missing parent")?)?;
        }
    }
    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Later edits preserved; resolve recovery conflicts: {}",
            conflicts.join(", ")
        ))
    }
}
pub fn recover(journal: &Path, allowed: impl Fn(&Path) -> bool) -> Result<bool, String> {
    let staging = storage::read(&journal.join("staging.json"), metadata::CATALOG_LIMIT)?;
    let pending = storage::read(&journal.join("pending.json"), metadata::CATALOG_LIMIT)?;
    let file = match (staging, pending) {
        (Some(_), Some(_)) => return Err("Conflicting transaction states; recovery refused".into()),
        (Some(file), None) => return recover_staging(journal, &file, allowed),
        (None, Some(file)) => file,
        (None, None) => {
            if let Some(path) =
                temporary(journal, "staging.json.tmp", metadata::CATALOG_LIMIT, 0o600)?
            {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
                storage::sync(journal)?;
                return Ok(true);
            }
            return Ok(false);
        }
    };
    let v = metadata::json(&file.bytes)?;
    let rows = v
        .as_array()
        .filter(|a| a.len() <= WRITE_LIMIT)
        .ok_or("Invalid recovery journal")?;
    let mut writes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut byte_count = 0_usize;
    for (i, row) in rows.iter().enumerate() {
        metadata::fields(row, "path before after")?;
        let path = PathBuf::from(metadata::text(&row["path"], 4096)?);
        storage::safe(&path)?;
        if !allowed(&path) || !seen.insert(path.clone()) {
            return Err("Recovery target is outside installation scope".into());
        }
        let before = if row["before"].is_null() {
            None
        } else {
            Some(load_saved(journal, i, "before", &row["before"])?)
        };
        let after = if row["after"].is_null() {
            None
        } else {
            Some(load_saved(journal, i, "after", &row["after"])?)
        };
        for data in [before.as_ref(), after.as_ref()].into_iter().flatten() {
            byte_count = byte_count
                .checked_add(data.bytes.len())
                .ok_or("Recovery snapshot size overflow; no files were changed")?;
            if byte_count > BYTE_LIMIT {
                return Err(
                    "App transaction exceeds recovery bounds; no files were changed".into(),
                );
            }
        }
        writes.push(Write {
            path,
            before,
            after,
        });
    }
    rollback(&writes)?;
    std::fs::rename(journal.join("pending.json"), journal.join("recovered.json"))
        .map_err(|e| e.to_string())?;
    storage::sync(journal)?;
    Ok(true)
}

fn recover_staging(
    journal: &Path,
    file: &FileData,
    allowed: impl Fn(&Path) -> bool,
) -> Result<bool, String> {
    let value = metadata::json(&file.bytes)?;
    let rows = value
        .as_array()
        .filter(|rows| rows.len() <= WRITE_LIMIT)
        .ok_or("Invalid staging journal")?;
    let mut seen = BTreeSet::new();
    let mut snapshots = Vec::new();
    let mut temporaries = Vec::new();
    let mut bytes = 0_usize;
    for (i, row) in rows.iter().enumerate() {
        metadata::fields(row, "path before after")?;
        let target = PathBuf::from(metadata::text(&row["path"], 4096)?);
        storage::safe(&target)?;
        if !allowed(&target) || !seen.insert(target) {
            return Err("Staging target is outside installation scope".into());
        }
        for kind in ["before", "after"] {
            if row[kind].is_null() {
                continue;
            }
            metadata::fields(&row[kind], "sha256 mode")?;
            let mode = metadata::field(metadata::field(row, kind)?, "mode")?
                .as_u64()
                .filter(|mode| *mode <= 0o777)
                .ok_or("Invalid staging image mode")?;
            let name = format!("{i}.{kind}.tmp");
            if temporary(journal, &name, metadata::BUNDLE_LIMIT, mode)?.is_some() {
                temporaries.push((name, metadata::BUNDLE_LIMIT, mode));
            }
            let path = journal.join(format!("{i}.{kind}"));
            match std::fs::symlink_metadata(&path) {
                // A pre-existing directory can prevent atomic publication.
                // It was never a snapshot and is not ours to remove.
                Ok(info) if info.is_dir() => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.to_string()),
                Ok(_) => (),
            }
            let image = load_saved(journal, i, kind, &row[kind])?;
            bytes = bytes
                .checked_add(image.bytes.len())
                .ok_or("Staging snapshot size overflow; no files were changed")?;
            if bytes > BYTE_LIMIT {
                return Err("Staging images exceed recovery bounds".into());
            }
            snapshots.push((path, &row[kind]));
        }
    }
    if temporary(journal, "staging.json.tmp", metadata::CATALOG_LIMIT, 0o600)?.is_some() {
        temporaries.push(("staging.json.tmp".into(), metadata::CATALOG_LIMIT, 0o600));
    }
    for (path, expected) in snapshots {
        if let Some(data) = storage::read(&path, metadata::BUNDLE_LIMIT)? {
            validate_saved(&data, expected)?;
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    for (name, limit, mode) in temporaries {
        if let Some(path) = temporary(journal, &name, limit, mode)? {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    storage::sync(journal)?;
    if storage::read(&journal.join("staging.json"), metadata::CATALOG_LIMIT)?.as_ref() != Some(file)
    {
        return Err("Staging journal changed during cleanup".into());
    }
    std::fs::rename(journal.join("staging.json"), journal.join("aborted.json"))
        .map_err(|e| e.to_string())?;
    storage::sync(journal)?;
    Ok(true)
}

// These exact names are producer-owned scratch space, never published images
// or persistent data. Partial bytes cannot have the completed image's hash.
fn temporary(
    journal: &Path,
    name: &str,
    limit: usize,
    mode: u64,
) -> Result<Option<PathBuf>, String> {
    let path = journal.join(name);
    let Some(data) = storage::read(&path, limit)? else {
        return Ok(None);
    };
    if data.mode != 0o600 && u64::from(data.mode) != mode {
        return Err("Temporary snapshot permissions changed; cleanup refused".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if std::fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .uid()
            != std::fs::symlink_metadata(journal)
                .map_err(|e| e.to_string())?
                .uid()
        {
            return Err("Temporary snapshot owner differs; cleanup refused".into());
        }
    }
    Ok(Some(path))
}

// Derived runtimes are backed up until commit or rollback is durable. They have
// no user data to retain afterward; payload backups keep their existing policy.
// Repeat this after startup so a crash during reclamation cannot leak each venv.
pub(super) fn discard_finished_removals(
    journal: &Path,
    disposable: impl Fn(&Path) -> bool,
) -> Result<(), String> {
    if storage::read(&journal.join("pending.json"), metadata::CATALOG_LIMIT)?.is_some() {
        return Ok(());
    }
    if storage::read(&journal.join("staging.json"), metadata::CATALOG_LIMIT)?.is_some() {
        return Ok(());
    }
    let finished = match storage::read(&journal.join("completed.json"), metadata::CATALOG_LIMIT)? {
        Some(file) => Some(file),
        None => storage::read(&journal.join("recovered.json"), metadata::CATALOG_LIMIT)?,
    };
    let Some(file) = finished else {
        return Ok(());
    };
    let value = metadata::json(&file.bytes)?;
    let rows = value
        .as_array()
        .filter(|rows| rows.len() <= WRITE_LIMIT)
        .ok_or("Invalid finished journal")?;
    let mut paths = Vec::new();
    let mut bytes = 0_usize;
    for (i, row) in rows.iter().enumerate() {
        metadata::fields(row, "path before after")?;
        let target = PathBuf::from(metadata::text(&row["path"], 4096)?);
        if !row["before"].is_null() && row["after"].is_null() && disposable(&target) {
            let backup = journal.join(format!("{i}.before"));
            if let Some(data) = storage::read(&backup, metadata::BUNDLE_LIMIT)? {
                validate_saved(&data, &row["before"])?;
                bytes = bytes
                    .checked_add(data.bytes.len())
                    .ok_or("Removal recovery byte count overflow")?;
                if bytes > BYTE_LIMIT {
                    return Err("Finished runtime backups exceed cleanup bounds".into());
                }
                paths.push((backup, &row["before"]));
            }
        }
    }
    storage::sync(journal)?;
    for (path, expected) in paths {
        if let Some(data) = storage::read(&path, metadata::BUNDLE_LIMIT)? {
            validate_saved(&data, expected)?;
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    storage::sync(journal)
}
fn load_saved(journal: &Path, i: usize, kind: &str, v: &Value) -> Result<FileData, String> {
    let d = storage::read(&journal.join(format!("{i}.{kind}")), metadata::BUNDLE_LIMIT)?
        .ok_or("Missing recovery backup")?;
    validate_saved(&d, v)?;
    Ok(d)
}
fn validate_saved(d: &FileData, v: &Value) -> Result<(), String> {
    metadata::fields(v, "sha256 mode")?;
    if v["sha256"] != storage::sha(&d.bytes) || v["mode"].as_u64() != Some(u64::from(d.mode)) {
        return Err("Corrupt recovery backup".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staging_crash_cleans_declared_scratch_and_preserves_later_edits() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let first = root.join("first");
        let second = root.join("second");
        let original = FileData {
            bytes: b"unchanged installed software".to_vec(),
            mode: 0o600,
        };
        storage::atomic(&first, &original)?;
        storage::atomic(&second, &original)?;
        let writes = [remove(first.clone())?, remove(second.clone())?];
        let journal = root.join("journal");
        storage::directory(&journal.join("1.before"))?;
        assert!(record(&journal, &writes).is_err());
        assert!(journal.join("staging.json").exists());
        let partial = journal.join("1.before.tmp");
        storage::atomic(
            &partial,
            &FileData {
                bytes: (*original.bytes.get(..7).ok_or("Missing fixture element")?).to_vec(),
                mode: 0o600,
            },
        )?;
        let unrelated = journal.join("user-note.txt");
        storage::atomic(&unrelated, &original)?;
        assert!(recover(&journal, |_| false).is_err());
        assert!(partial.exists());
        let edited = FileData {
            bytes: b"later backup edit".to_vec(),
            mode: 0o600,
        };
        storage::atomic(&journal.join("0.before"), &edited)?;
        assert!(recover(&journal, |p| p == first || p == second).is_err());
        assert_eq!(storage::read(&journal.join("0.before"), 100)?, Some(edited));
        assert!(partial.exists());
        storage::atomic(&journal.join("0.before"), &original)?;
        assert!(recover(&journal, |p| p == first || p == second)?);
        assert!(!partial.exists());
        assert!(!journal.join("0.before").exists());
        assert!(journal.join("1.before").is_dir());
        assert!(journal.join("aborted.json").is_file());
        assert_eq!(storage::read(&first, 100)?, Some(original.clone()));
        assert_eq!(storage::read(&second, 100)?, Some(original.clone()));
        assert_eq!(storage::read(&unrelated, 100)?, Some(original));
        assert!(!recover(&journal, |_| true)?);
        let incomplete = root.join("incomplete-metadata");
        storage::atomic(
            &incomplete.join("staging.json.tmp"),
            &FileData {
                bytes: b"{unfinished".to_vec(),
                mode: 0o600,
            },
        )?;
        assert!(recover(&incomplete, |_| false)?);
        assert!(!incomplete.join("staging.json.tmp").exists());
        Ok(())
    }

    #[test]
    fn atomic_temp_collision_preserves_the_preexisting_file() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let destination = root.join("destination");
        let temporary = root.join("temporary");
        let existing = FileData {
            bytes: b"keep the existing file".to_vec(),
            mode: 0o600,
        };
        storage::atomic(&temporary, &existing)?;
        assert!(storage::atomic_with_temp(&destination, &existing, &temporary).is_err());
        assert_eq!(storage::read(&temporary, 100)?, Some(existing));
        assert!(!destination.exists());
        Ok(())
    }
    #[test]
    fn journal_staging_failure_releases_owned_partial_backups() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let first = root.join("first");
        let second = root.join("second");
        let data = FileData {
            bytes: vec![42; 1024 * 1024],
            mode: 0o600,
        };
        storage::atomic(&first, &data)?;
        storage::atomic(&second, &data)?;
        let writes = [remove(first.clone())?, remove(second.clone())?];
        let journal = root.join("journal");
        storage::directory(&journal)?;
        let enospc = std::env::var_os("VITRALLIS_QA_JOURNAL_ENOSPC").is_some();
        if !enospc {
            // Deterministic I/O failure after the first before image is staged.
            storage::directory(&journal.join("1.before"))?;
        }
        let marker = root.join(".installation-pending");
        storage::atomic(
            &marker,
            &FileData {
                bytes: b"pending".to_vec(),
                mode: 0o600,
            },
        )?;
        let error = commit(&journal, &writes, &marker)
            .err()
            .ok_or("journal staging must fail")?;
        if enospc {
            assert!(
                error.to_ascii_lowercase().contains("no space left"),
                "{error}"
            );
        }
        assert_eq!(
            storage::read(&first, metadata::FILE_LIMIT)?,
            Some(data.clone())
        );
        assert_eq!(storage::read(&second, metadata::FILE_LIMIT)?, Some(data));
        assert!(!marker.exists());
        assert!(!journal.join("pending.json").exists());
        assert!(!journal.join("0.before").exists());
        if !enospc {
            assert!(journal.join("1.before").is_dir());
        }
        Ok(())
    }
    #[test]
    fn runtime_reclamation_waits_for_completion_and_preserves_edited_backups() -> Result<(), String>
    {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let runtime = root
            .join("runtime")
            .join("a".repeat(64))
            .join("dependency.py");
        let source = root.join("main.py");
        let original = FileData {
            bytes: b"owned code".to_vec(),
            mode: 0o600,
        };
        storage::atomic(&runtime, &original)?;
        storage::atomic(&source, &original)?;
        let writes = [remove(runtime.clone())?, remove(source.clone())?];
        let journal = root.join("journal");
        record(&journal, &writes)?;
        replace(&runtime, None)?;
        discard_finished_removals(&journal, |path| path == runtime)?;
        assert!(journal.join("0.before").exists());
        assert!(recover(&journal, |path| path == runtime || path == source)?);
        assert_eq!(storage::read(&runtime, 100)?, Some(original));
        discard_finished_removals(&journal, |path| path == runtime)?;
        assert!(!journal.join("0.before").exists());
        assert!(journal.join("1.before").exists());
        apply_with(&journal, &writes, |_| Ok(()))?;
        let edited = FileData {
            bytes: b"a later edit".to_vec(),
            mode: 0o600,
        };
        storage::atomic(&journal.join("0.before"), &edited)?;
        assert!(discard_finished_removals(&journal, |path| path == runtime).is_err());
        assert_eq!(storage::read(&journal.join("0.before"), 100)?, Some(edited));
        storage::atomic(
            &journal.join("0.before"),
            writes[0].before.as_ref().ok_or("missing before image")?,
        )?;
        discard_finished_removals(&journal, |path| path == runtime)?;
        discard_finished_removals(&journal, |path| path == runtime)?;
        assert!(!journal.join("0.before").exists());
        assert!(journal.join("1.before").exists());
        Ok(())
    }
    #[test]
    fn finalization_failure_rolls_back_verified_files_and_commit_releases_marker()
    -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let path = root.join("app");
        let before = FileData {
            bytes: b"old release".to_vec(),
            mode: 0o755,
        };
        storage::atomic(&path, &before)?;
        let writes = [plan(
            path.clone(),
            FileData {
                bytes: b"new release".to_vec(),
                mode: 0o755,
            },
        )?];
        let journal = root.join("failed");
        assert!(
            apply_validated(
                &journal,
                &writes,
                |_| Ok(()),
                || Err("finalization failed".into())
            )
            .is_err()
        );
        assert_eq!(storage::read(&path, 100)?, Some(before));
        assert!(recover(&journal, |p| p == path)?);
        let marker = root.join(".installation-pending");
        storage::atomic(
            &marker,
            &FileData {
                bytes: b"pending".to_vec(),
                mode: 0o600,
            },
        )?;
        commit(&root.join("success"), &writes, &marker)?;
        assert!(!marker.exists());
        assert_eq!(storage::read(&path, 100)?, writes[0].after);
        Ok(())
    }
    #[test]
    fn deletions_roll_back_and_recover_without_overwriting_recreated_files() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let first = root.join("first");
        let second = root.join("second");
        let journal = root.join("journal");
        let old = FileData {
            bytes: b"owned app file".to_vec(),
            mode: 0o600,
        };
        storage::atomic(&first, &old)?;
        storage::atomic(&second, &old)?;
        let writes = vec![remove(first.clone())?, remove(second.clone())?];
        assert!(apply_with(&journal, &writes, |_| Err("disk failure".into())).is_err());
        assert_eq!(storage::read(&first, 100)?, Some(old.clone()));
        assert_eq!(storage::read(&second, 100)?, Some(old.clone()));
        assert!(recover(&journal, |p| p == first || p == second)?);
        let crash = root.join("crash");
        record(&crash, &writes)?;
        replace(&first, None)?;
        assert!(recover(&crash, |p| p == first || p == second)?);
        assert_eq!(storage::read(&first, 100)?, Some(old));
        let later = FileData {
            bytes: b"new user file".to_vec(),
            mode: 0o600,
        };
        assert!(
            apply_with(&journal, &writes, |_| {
                storage::atomic(&first, &later)?;
                Err("interrupted".into())
            })
            .is_err()
        );
        assert_eq!(storage::read(&first, 100)?, Some(later));
        assert!(recover(&journal, |p| p == first || p == second).is_err());
        Ok(())
    }
    #[test]
    fn rollback_preserves_later_edits_and_recovery_is_repeatable() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let path = root.join("app");
        let journal = root.join("journal");
        let old = FileData {
            bytes: b"old".to_vec(),
            mode: 0o600,
        };
        storage::atomic(&path, &old)?;
        let w = plan(
            path.clone(),
            FileData {
                bytes: b"new".to_vec(),
                mode: 0o600,
            },
        )?;
        assert!(apply_with(&journal, &[w], |_| Err("interrupted".into())).is_err());
        assert_eq!(storage::read(&path, 100)?, Some(old));
        assert!(recover(&journal, |p| p == path)?);
        assert!(!recover(&journal, |p| p == path)?);
        let edited_write = plan(
            path.clone(),
            FileData {
                bytes: b"new".to_vec(),
                mode: 0o600,
            },
        )?;
        let later = FileData {
            bytes: b"user edit".to_vec(),
            mode: 0o600,
        };
        assert!(
            apply_with(&journal, &[edited_write], |_| {
                storage::atomic(&path, &later)?;
                Err("interrupted".into())
            })
            .is_err()
        );
        assert_eq!(storage::read(&path, 100)?, Some(later));
        assert!(recover(&journal, |p| p == path).is_err());
        Ok(())
    }
}
