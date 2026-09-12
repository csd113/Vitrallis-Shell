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
    if let Some(data) = data {
        return storage::atomic(path, data);
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
    storage::directory(journal)?;
    let mut rows = Vec::new();
    for (i, w) in writes.iter().enumerate() {
        if let Some(old) = &w.before {
            storage::atomic(&journal.join(format!("{i}.before")), old)?;
        }
        if let Some(after) = &w.after {
            storage::atomic(&journal.join(format!("{i}.after")), after)?;
        }
        rows.push(serde_json::json!({"path":w.path,"before":value(w.before.as_ref()),"after":value(w.after.as_ref())}));
    }
    storage::atomic(
        &journal.join("pending.json"),
        &FileData {
            bytes: serde_json::to_vec(&rows).map_err(|e| e.to_string())?,
            mode: 0o600,
        },
    )
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
        if recover(journal, |p| writes.iter().any(|w| w.path == p)).is_ok() {
            match std::fs::remove_file(marker) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(format!("{error}; marker cleanup failed: {e}")),
            }
            storage::sync(marker.parent().ok_or("Missing marker parent")?)?;
        } else {
            storage::atomic(marker, &marker_data)?;
        }
        return Err(error);
    }
    Ok(())
}
#[cfg(test)]
fn apply_with(
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
    if storage::read(&journal.join("pending.json"), metadata::CATALOG_LIMIT)?.is_some() {
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
    let Some(file) = storage::read(&journal.join("pending.json"), metadata::CATALOG_LIMIT)? else {
        return Ok(false);
    };
    let v = metadata::json(&file.bytes)?;
    let rows = v
        .as_array()
        .filter(|a| a.len() <= 2056)
        .ok_or("Invalid recovery journal")?;
    let mut writes = Vec::new();
    let mut seen = BTreeSet::new();
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
fn load_saved(journal: &Path, i: usize, kind: &str, v: &Value) -> Result<FileData, String> {
    metadata::fields(v, "sha256 mode")?;
    let d = storage::read(&journal.join(format!("{i}.{kind}")), metadata::BUNDLE_LIMIT)?
        .ok_or("Missing recovery backup")?;
    if v["sha256"] != storage::sha(&d.bytes) || v["mode"].as_u64() != Some(u64::from(d.mode)) {
        return Err("Corrupt recovery backup".into());
    }
    Ok(d)
}
#[cfg(test)]
mod tests {
    use super::*;
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
        let w = plan(
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
            apply_with(&journal, &[w], |_| {
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
