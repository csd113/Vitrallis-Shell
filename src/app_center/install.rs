//! Manifest-based Python package installation and receipt-scoped repair.
use super::{
    metadata::{self, Files, Package},
    runtime::{self, Runtime},
    storage::{self, FileData, Locations},
    transaction::{self, Write},
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
/// Discovery retains only catalog metadata, never acquired files or write plans.
#[derive(Debug, Clone)]
pub struct Checked {
    pub package: Package,
    pub installed: String,
    pub status: String,
    pub ready: bool,
}

pub fn check(loc: &Locations, p: Package) -> Result<Checked, String> {
    validate_paths(&p)?;
    let root = loc.root(&p);
    // Recovery may need payload backups; defer it to the selected installation.
    // An interrupted transaction can leave a receipt/inventory only partly replaced.
    if storage::read(&root.join(".installation-pending"), 1024)?.is_some() {
        return Ok(Checked {
            installed: label(loc, &p).unwrap_or_else(|_| "incomplete".into()),
            package: p,
            status: "incomplete / repair".into(),
            ready: true,
        });
    }
    let installed = label(loc, &p)?;
    let saved = receipt(&root)?;
    let mut ready = true;
    if let Some(r) = &saved {
        if r["origin"] != p.origin.as_str()
            || r["repository"] != p.repository.as_str()
            || r["id"] != p.id
        {
            return Err("Installed origin differs; refusing publisher/source switch".into());
        }
        let local = metadata::version(metadata::text(&r["version"], 32)?)?;
        if local > p.version {
            return Err("Installed version is newer; downgrade blocked".into());
        }
        let hashes = p
            .files
            .iter()
            .map(|f| (f.path.clone(), Value::String(f.sha256.clone())))
            .collect::<serde_json::Map<_, _>>();
        if local == p.version && r["files"] != Value::Object(hashes) {
            return Err("Same version has a different inventory; keeping local files".into());
        }
        ready = local != p.version;
        for (name, hash) in r["files"].as_object().ok_or("Invalid receipt")? {
            match storage::read(&root.join(name), metadata::FILE_LIMIT)? {
                Some(old) if hash != &storage::sha(&old.bytes) => {
                    return Err(format!("Local edit preserved: {name}"));
                }
                None => ready = true,
                Some(_) => (),
            }
        }
        let launcher = loc.state.join("launchers").join(&p.id);
        if storage::read(&launcher, metadata::FILE_LIMIT)?.is_none_or(|f| f.mode & 0o111 == 0) {
            ready = true;
        }
        let desktop = format!("{}.desktop", p.id);
        for directory in [loc.data.join("applications"), loc.home.join("Desktop")] {
            if storage::read(&directory.join(&desktop), metadata::FILE_LIMIT)?.is_none() {
                ready = true;
            }
        }
    } else if installed != "not installed" {
        return Err("Unknown installed origin/version; keeping existing files".into());
    }
    let status = if !ready {
        "up to date"
    } else if installed == "not installed" {
        "ready to install"
    } else {
        "update / repair available"
    }
    .into();
    Ok(Checked {
        package: p,
        installed,
        status,
        ready,
    })
}

fn validate_paths(p: &Package) -> Result<(), String> {
    metadata::check_paths(p.files.iter().map(|f| f.path.as_str()))?;
    for file in &p.files {
        validate_owned_path(&file.path)?;
    }
    Ok(())
}
pub(super) fn validate_owned_path(name: &str) -> Result<(), String> {
    metadata::path(name)?;
    if name.split('/').any(|c| {
        matches!(
            c.to_ascii_lowercase().as_str(),
            ".vitrallis-receipt.json"
                | ".installation-pending"
                | ".venv"
                | "runtime"
                | ".vitrallis-bytecode"
                | "__pycache__"
        )
    }) {
        return Err("Package path collides with installer/runtime state".into());
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Planned {
    pub package: Package,
    pub status: String,
    pub prepared: Option<Prepared>,
}
#[derive(Debug, Clone)]
pub struct Prepared {
    pub files: Files,
    pub writes: Vec<Write>,
    pub created_at: Instant,
}
pub fn label(loc: &Locations, p: &Package) -> Result<String, String> {
    let root = loc.root(p);
    if let Some(receipt) = receipt(&root)? {
        return Ok(metadata::text(&receipt["version"], 32)?.into());
    }
    if storage::read(&root.join(&p.entry), metadata::FILE_LIMIT)?.is_none()
        && storage::read(&root.join("app.toml"), metadata::FILE_LIMIT)?.is_none()
    {
        return Ok("not installed".into());
    }
    Ok("local / unknown".into())
}
pub(super) fn receipt(root: &Path) -> Result<Option<Value>, String> {
    storage::read(
        &root.join(".vitrallis-receipt.json"),
        metadata::CATALOG_LIMIT,
    )?
    .map(|d| {
        let v = metadata::json(&d.bytes)?;
        metadata::fields(&v, "version origin repository commit id files")?;
        metadata::version(metadata::text(&v["version"], 32)?)?;
        super::sources::Repository::parse(metadata::text(&v["origin"], 160)?)?;
        super::sources::Repository::parse(metadata::text(&v["repository"], 160)?)?;
        metadata::identity(metadata::text(&v["id"], 128)?)?;
        metadata::hex(metadata::text(&v["commit"], 40)?, 40)?;
        let files = v["files"]
            .as_object()
            .filter(|m| m.len() <= 256)
            .ok_or("Invalid receipt inventory")?;
        metadata::check_paths(files.keys().map(String::as_str))?;
        for name in files.keys() {
            validate_owned_path(name)?;
        }
        for hash in files.values() {
            metadata::hex(metadata::text(hash, 64)?, 64)?;
        }
        Ok(v)
    })
    .transpose()
}
#[cfg(test)]
pub fn prepare(loc: &Locations, p: Package, files: Files) -> Result<Planned, String> {
    prepare_with_modes(loc, p, files, &std::collections::BTreeMap::new())
}
pub fn prepare_with_modes(
    loc: &Locations,
    p: Package,
    files: Files,
    modes: &std::collections::BTreeMap<String, u32>,
) -> Result<Planned, String> {
    metadata::validate_bundle(&p, &files)?;
    validate_paths(&p)?;
    let root = loc.root(&p);
    recover(loc, &p)?;
    let installed = label(loc, &p)?;
    let old_receipt = receipt(&root)?;
    protect(&p, &root, &files, old_receipt.as_ref())?;
    let runtime = runtime::ensure(&root, &files)?;
    runtime::validate(&runtime, &files)?;
    let mut writes = Vec::new();
    for (name, bytes) in &files {
        writes.push(transaction::plan(
            root.join(name),
            FileData {
                bytes: bytes.clone(),
                mode: modes.get(name).copied().unwrap_or(0o644),
            },
        )?);
    }
    obsolete(&root, &files, old_receipt.as_ref(), &mut writes)?;
    support(loc, &p, &runtime, &mut writes)?;
    let changed = writes.iter().any(|w| w.before != w.after);
    let pending = storage::read(&root.join(".installation-pending"), 1024)?.is_some();
    let receipt = serde_json::json!({"version":p.version.to_string(),"origin":p.origin.as_str(),"repository":p.repository.as_str(),"commit":p.commit,"id":p.id,"files":files.iter().map(|(k,v)|(k.clone(),Value::String(storage::sha(v)))).collect::<serde_json::Map<_,_>>()});
    writes.push(transaction::plan(
        root.join(".vitrallis-receipt.json"),
        FileData {
            bytes: serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
            mode: 0o600,
        },
    )?);
    let status = if pending {
        "incomplete / repair"
    } else if changed && installed == "not installed" {
        "ready to install"
    } else if changed {
        "update / repair available"
    } else {
        "up to date"
    }
    .to_owned();
    Ok(Planned {
        package: p,
        status,
        prepared: (changed || pending).then_some(Prepared {
            files,
            writes,
            created_at: Instant::now(),
        }),
    })
}
fn obsolete(
    root: &Path,
    files: &Files,
    receipt: Option<&Value>,
    writes: &mut Vec<Write>,
) -> Result<(), String> {
    let mut modules = std::collections::BTreeSet::new();
    let old = receipt.and_then(|r| r["files"].as_object());
    for name in files.keys().chain(old.into_iter().flat_map(|r| r.keys())) {
        if old.is_some_and(|r| r.contains_key(name)) && !files.contains_key(name) {
            writes.push(transaction::remove(root.join(name))?);
        }
        let path = Path::new(name);
        if path.extension().is_some_and(|ext| ext == "py") {
            let parent = root.join(path.parent().ok_or("Missing module parent")?);
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid module name")?;
            modules.insert((parent, stem.to_owned()));
        }
    }
    // Python timestamp bytecode may remain valid across same-size, same-second
    // replacements. Remove only caches belonging to managed source modules.
    for (parent, stem) in modules {
        let cache = parent.join("__pycache__");
        storage::safe(&cache)?;
        let entries = match std::fs::read_dir(&cache) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        let prefix = format!("{stem}.");
        for (index, entry) in entries.enumerate() {
            if index >= 1024 {
                return Err("Python cache directory exceeds bounds".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            if name.to_str().is_some_and(|n| {
                n.starts_with(&prefix) && Path::new(n).extension().is_some_and(|ext| ext == "pyc")
            }) {
                writes.push(transaction::remove(entry.path())?);
            }
        }
    }
    if writes.len() > 2048 {
        return Err("Installation transaction exceeds bounds".into());
    }
    Ok(())
}
fn protect(p: &Package, root: &Path, files: &Files, receipt: Option<&Value>) -> Result<(), String> {
    if let Some(r) = receipt {
        if r["origin"] != p.origin.as_str()
            || r["repository"] != p.repository.as_str()
            || r["id"] != p.id
        {
            return Err("Installed origin differs; refusing publisher/source switch".into());
        }
        let local = metadata::version(metadata::text(&r["version"], 32)?)?;
        if local > p.version {
            return Err("Installed version is newer; downgrade blocked".into());
        }
        if local == p.version {
            let hashes = files
                .iter()
                .map(|(name, bytes)| (name.clone(), Value::String(storage::sha(bytes))))
                .collect::<serde_json::Map<_, _>>();
            if r["files"] != Value::Object(hashes) {
                return Err("Same version has a different inventory; keeping local files".into());
            }
        }
        for (name, hash) in r["files"].as_object().ok_or("Invalid receipt")? {
            if let Some(old) = storage::read(&root.join(name), metadata::FILE_LIMIT)? {
                if hash != &storage::sha(&old.bytes) {
                    return Err(format!("Local edit preserved: {name}"));
                }
                if local == p.version && files.get(name).is_some_and(|b| b != &old.bytes) {
                    return Err("Same version has different files; local copy preserved".into());
                }
            }
        }
    } else if storage::read(&root.join(&p.entry), metadata::FILE_LIMIT)?.is_some()
        || storage::read(&root.join("app.toml"), metadata::FILE_LIMIT)?.is_some()
    {
        return Err("Unknown installed origin/version; keeping existing files".into());
    }
    for (name, bytes) in files {
        if let Some(old) = storage::read(&root.join(name), metadata::FILE_LIMIT)? {
            let owned = receipt.is_some_and(|r| r["files"].get(name).is_some());
            if !owned && old.bytes != *bytes {
                return Err(format!("Unmanaged custom file preserved: {name}"));
            }
        }
    }
    Ok(())
}
fn support(
    loc: &Locations,
    p: &Package,
    runtime: &Runtime,
    writes: &mut Vec<Write>,
) -> Result<(), String> {
    let root = loc.root(p);
    let launch_path = loc.state.join("launchers").join(&p.id);
    let before = storage::read(&launch_path, metadata::FILE_LIMIT)?;
    let after = FileData {
        bytes: runtime::launcher(runtime, &root.join(&p.entry), &p.commit)?,
        mode: 0o755,
    };
    if let Some(old) = &before {
        let old_entry = super::uninstall::installed_entry(&root, p)?;
        let saved = receipt(&root)?;
        let old_commit = saved
            .as_ref()
            .map_or(Ok(p.commit.as_str()), |r| metadata::text(&r["commit"], 40))?;
        let mut old_files = Files::new();
        if let Some(requirements) = storage::read(&root.join("requirements.txt"), 65536)? {
            old_files.insert("requirements.txt".into(), requirements.bytes);
        }
        let managed_launcher = runtime::candidates(&root, &old_files)
            .into_iter()
            .map(|program| runtime::launcher(&Runtime { program }, &old_entry, old_commit))
            .collect::<Result<Vec<_>, _>>()?
            .contains(&old.bytes);
        if old.bytes != after.bytes && !managed_launcher {
            return Err("App launcher was edited; preserve your changes and restore the managed launcher before updating".into());
        }
    }
    writes.push(Write {
        path: launch_path.clone(),
        before,
        after: Some(after),
    });

    let icon = root.join("icon.png");
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec={}\nIcon={}\nTerminal=false\nCategories=Utility;\n",
        p.name.replace('\\', "\\\\"),
        desktop_quote(&launch_path)?,
        icon.to_str()
            .ok_or("Icon path must be UTF-8")?
            .replace('\\', "\\\\")
    );
    let filename = format!("{}.desktop", p.id);
    for directory in [loc.data.join("applications"), loc.home.join("Desktop")] {
        let path = directory.join(&filename);
        let before = storage::read(&path, metadata::FILE_LIMIT)?;
        // Existing shortcuts are user-owned and may contain custom launch options.
        let after = before.clone().unwrap_or_else(|| FileData {
            bytes: desktop.as_bytes().to_vec(),
            mode: 0o755,
        });
        writes.push(Write {
            path,
            before,
            after: Some(after),
        });
    }
    Ok(())
}
pub(super) fn desktop_quote(path: &Path) -> Result<String, String> {
    let s = path.to_str().ok_or("Desktop path must be UTF-8")?;
    if s.contains('%') {
        return Err("Desktop paths containing % are unsupported".into());
    }
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
    ))
}
pub(super) fn journal_root(loc: &Locations, p: &Package) -> PathBuf {
    loc.state
        .join("transactions")
        .join(storage::sha(p.key().as_bytes()))
}
pub fn recover(loc: &Locations, p: &Package) -> Result<(), String> {
    let base = journal_root(loc, p);
    storage::safe(&base)?;
    let entries = match std::fs::read_dir(&base) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        transaction::recover(&entry.path(), |path| allowed(loc, p, path))?;
    }
    Ok(())
}
fn allowed(loc: &Locations, p: &Package, path: &Path) -> bool {
    path.starts_with(loc.root(p))
        || path == loc.state.join("launchers").join(&p.id)
        || [loc.data.join("applications"), loc.home.join("Desktop")]
            .iter()
            .any(|d| path == d.join(format!("{}.desktop", p.id)))
}
pub fn install(loc: &Locations, checked: &Planned) -> Result<(), String> {
    let prepared = checked
        .prepared
        .as_ref()
        .ok_or("No verified update available")?;
    if prepared.created_at.elapsed() > Duration::from_secs(15 * 60) {
        return Err("Installation plan expired; retry installation".into());
    }
    metadata::validate_bundle(&checked.package, &prepared.files)?;
    for w in &prepared.writes {
        if storage::read(&w.path, metadata::BUNDLE_LIMIT)? != w.before {
            return Err("File changed since preparation; retry installation".into());
        }
    }
    let root = loc.root(&checked.package);
    let marker = root.join(".installation-pending");
    storage::read(&marker, 1024)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let journal = journal_root(loc, &checked.package).join(stamp.to_string());
    storage::atomic(
        &marker,
        &FileData {
            bytes: b"Installation incomplete; check and repair.\n".to_vec(),
            mode: 0o600,
        },
    )?;
    transaction::commit(&journal, &prepared.writes, &marker)
}
