//! Reviewed generic Python recipe and the legacy Bitcoin adapter.
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
#[derive(Debug, Clone)]
pub struct Checked {
    pub package: Package,
    pub installed: String,
    pub status: String,
    pub prepared: Option<Prepared>,
}
#[derive(Debug, Clone)]
pub struct Prepared {
    pub files: Files,
    pub writes: Vec<Write>,
    pub checked_at: Instant,
    pub entry: PathBuf,
}
pub fn label(loc: &Locations, p: &Package) -> Result<String, String> {
    let root = loc.root(p);
    if let Some(receipt) = receipt(&root)? {
        return Ok(metadata::text(&receipt["version"], 32)?.into());
    }
    let Some(entry) = storage::read(&root.join(&p.entry), metadata::FILE_LIMIT)? else {
        return Ok("not installed".into());
    };
    if let Some(version) = metadata::legacy_version(&entry.bytes) {
        return Ok(version.to_string());
    }
    if p.legacy() {
        return legacy_label(loc, &entry.bytes);
    }
    Ok("local / unknown".into())
}
fn legacy_label(loc: &Locations, bytes: &[u8]) -> Result<String, String> {
    let digest = storage::sha(bytes);
    if digest == "14d91cc19782ced7716132a0563161bdb8cd9b86c3ceffb8053ee9409b158ccd" {
        return Ok("cd1f1d7a".into());
    }
    let file = loc
        .home
        .join(".local/share/pocket-update-apps/receipts")
        .join(format!("{digest}.json"));
    if let Some(saved) = storage::read(&file, 65536)? {
        let v = metadata::json(&saved.bytes)?;
        if v["repo"]
            .as_str()
            .and_then(|s| super::sources::Repository::parse(s).ok())
            .is_some_and(|r| r.is_default())
        {
            let commit = metadata::text(&v["commit"], 40)?;
            metadata::hex(commit, 40)?;
            return Ok(commit[..8].into());
        }
    }
    Ok("local / unknown".into())
}
fn receipt(root: &Path) -> Result<Option<Value>, String> {
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
        for hash in files.values() {
            metadata::hex(metadata::text(hash, 64)?, 64)?;
        }
        Ok(v)
    })
    .transpose()
}
pub fn prepare(loc: &Locations, p: Package, files: Files) -> Result<Checked, String> {
    metadata::validate_bundle(&p, &files)?;
    for name in files.keys() {
        if name.split('/').any(|c| {
            matches!(
                c.to_ascii_lowercase().as_str(),
                ".vitrallis-receipt.json" | ".installation-pending" | ".venv" | "runtime"
            )
        }) {
            return Err("Package path collides with installer/runtime state".into());
        }
    }
    let root = loc.root(&p);
    let runtime = runtime::detect(&loc.home, &root, &files)?;
    runtime::validate(&runtime, &files)?;
    if p.legacy() && runtime::legacy_version(&runtime, &files[&p.entry])? != Some(p.version.clone())
    {
        return Err("Catalog version does not match the legacy Python assignment".into());
    }
    recover(loc, &p)?;
    let installed = label(loc, &p)?;
    let old_receipt = receipt(&root)?;
    let source_files: Files = files
        .iter()
        .filter(|(name, _)| !p.legacy() || !matches!(name.as_str(), "launch" | "bitcoin.png"))
        .map(|(name, bytes)| (name.clone(), bytes.clone()))
        .collect();
    protect(&p, &root, &source_files, old_receipt.as_ref(), &runtime)?;
    let mut writes = Vec::new();
    for (name, bytes) in &source_files {
        writes.push(transaction::plan(
            root.join(name),
            FileData {
                bytes: bytes.clone(),
                mode: 0o644,
            },
        )?);
    }
    support(loc, &p, &runtime, &files, &mut writes)?;
    let changed = writes.iter().any(|w| w.before.as_ref() != Some(&w.after));
    let pending = storage::read(&root.join(".installation-pending"), 1024)?.is_some();
    let entry = root.join(&p.entry);
    let receipt = serde_json::json!({"version":p.version.to_string(),"origin":p.origin.as_str(),"repository":p.repository.as_str(),"commit":p.commit,"id":p.id,"files":source_files.iter().map(|(k,v)|(k.clone(),Value::String(storage::sha(v)))).collect::<serde_json::Map<_,_>>()});
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
    Ok(Checked {
        package: p,
        installed,
        status,
        prepared: (changed || pending).then_some(Prepared {
            files,
            writes,
            checked_at: Instant::now(),
            entry,
        }),
    })
}
fn protect(
    p: &Package,
    root: &Path,
    files: &Files,
    receipt: Option<&Value>,
    runtime: &Runtime,
) -> Result<(), String> {
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
    } else if let Some(old) = storage::read(&root.join(&p.entry), metadata::FILE_LIMIT)? {
        let identical = files.get(&p.entry) == Some(&old.bytes);
        if !p.legacy() {
            return Err("Unknown installed origin/version; keeping existing files".into());
        }
        if !identical {
            let known = storage::sha(&old.bytes)
                == "14d91cc19782ced7716132a0563161bdb8cd9b86c3ceffb8053ee9409b158ccd";
            match runtime::legacy_version(runtime, &old.bytes)? {
                Some(v) if v > p.version => {
                    return Err("Installed version is newer; downgrade blocked".into());
                }
                Some(v) if v == p.version => {
                    return Err("Same version has different files; local copy preserved".into());
                }
                None if !known => {
                    return Err("Local version is unknown; keeping existing files".into());
                }
                _ => (),
            }
        }
    }
    for (name, bytes) in files {
        if let Some(old) = storage::read(&root.join(name), metadata::FILE_LIMIT)? {
            let owned = receipt.is_some_and(|r| r["files"].get(name).is_some())
                || (p.legacy() && name == "bitcoin.py");
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
    files: &Files,
    writes: &mut Vec<Write>,
) -> Result<(), String> {
    let root = loc.root(p);
    if p.legacy() {
        for (name, mode) in [("launch", 0o755), ("bitcoin.png", 0o644)] {
            let before = storage::read(&root.join(name), metadata::FILE_LIMIT)?;
            let after = if let Some(existing) = before
                .as_ref()
                .filter(|d| name != "launch" || d.mode & 0o111 != 0)
            {
                existing.clone()
            } else {
                FileData {
                    bytes: if name == "launch" {
                        runtime::launcher(runtime, &root.join(&p.entry))?
                    } else {
                        published_icon(files)?
                    },
                    mode,
                }
            };
            writes.push(Write {
                path: root.join(name),
                before,
                after,
            });
        }
    }
    let launch_path = if p.legacy() {
        root.join("launch")
    } else {
        loc.state.join("launchers").join(&p.id)
    };
    if !p.legacy() {
        let before = storage::read(&launch_path, metadata::FILE_LIMIT)?;
        let after = before
            .as_ref()
            .filter(|d| d.mode & 0o111 != 0)
            .cloned()
            .unwrap_or(FileData {
                bytes: runtime::launcher(runtime, &root.join(&p.entry))?,
                mode: 0o755,
            });
        writes.push(Write {
            path: launch_path.clone(),
            before,
            after,
        });
    }
    let icon = root.join(if p.legacy() {
        "bitcoin.png"
    } else {
        "icon.png"
    });
    let name = if p.legacy() { "Bitcoin CAD" } else { &p.name };
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec={}\nIcon={}\nTerminal=false\nCategories=Utility;\n",
        name.replace('\\', "\\\\"),
        desktop_quote(&launch_path)?,
        icon.to_str()
            .ok_or("Icon path must be UTF-8")?
            .replace('\\', "\\\\")
    );
    let filename = if p.legacy() {
        "pocket-bitcoin.desktop".into()
    } else {
        format!("{}.desktop", p.id)
    };
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
            after,
        });
    }
    if p.legacy() {
        menu(loc, &root, writes)?;
    }
    Ok(())
}
fn desktop_quote(path: &Path) -> Result<String, String> {
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
fn menu(loc: &Locations, root: &Path, writes: &mut Vec<Write>) -> Result<(), String> {
    let path = loc.home.join(".pocket-home/config.json");
    let Some(before) = storage::read(&path, 1024 * 1024)? else {
        return Ok(());
    };
    let mut v = metadata::json(&before.bytes)?;
    let pages = v["pages"]
        .as_array_mut()
        .ok_or("Invalid PocketHome pages")?;
    if pages.iter().any(|p| !p.is_object()) {
        return Err("Invalid PocketHome page".into());
    }
    let matches: Vec<_> = pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p["name"] == "Apps")
        .map(|(i, _)| i)
        .collect();
    if matches.len() != 1 {
        return Err("Expected exactly one Apps page".into());
    }
    let items = pages[matches[0]]["items"]
        .as_array_mut()
        .ok_or("Invalid Apps items")?;
    if items.iter().any(|i| !i.is_object()) {
        return Err("Invalid Home item".into());
    }
    let shell = format!(
        "'{}'",
        root.join("launch").to_string_lossy().replace('\'', "'\\''")
    );
    if !items.iter().any(|i| {
        i["name"] == "Bitcoin CAD"
            || i["shell"] == shell
            || i["shell"] == root.join("launch").to_string_lossy().as_ref()
    }) {
        items.push(
            serde_json::json!({"name":"Bitcoin CAD","icon":root.join("bitcoin.png"),"shell":shell}),
        );
        writes.push(Write {
            path,
            after: FileData {
                bytes: serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?,
                mode: before.mode,
            },
            before: Some(before),
        });
    }
    Ok(())
}
fn journal_root(loc: &Locations, p: &Package) -> PathBuf {
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
        || path == loc.home.join(".pocket-home/config.json") && p.legacy()
        || [loc.data.join("applications"), loc.home.join("Desktop")]
            .iter()
            .any(|d| {
                path == d.join(if p.legacy() {
                    "pocket-bitcoin.desktop".into()
                } else {
                    format!("{}.desktop", p.id)
                })
            })
}
pub fn install(loc: &Locations, checked: &Checked) -> Result<(), String> {
    let prepared = checked
        .prepared
        .as_ref()
        .ok_or("No verified update available")?;
    if prepared.checked_at.elapsed() > Duration::from_secs(15 * 60) {
        return Err("Check expired; check for updates again".into());
    }
    metadata::validate_bundle(&checked.package, &prepared.files)?;
    for w in &prepared.writes {
        if storage::read(&w.path, metadata::BUNDLE_LIMIT)? != w.before {
            return Err("File changed since check; retry Check for updates".into());
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
    transaction::apply(&journal, &prepared.writes)?;
    if checked.package.legacy()
        && let Some(old) = prepared
            .writes
            .iter()
            .find(|w| w.path == root.join("bitcoin.py"))
            .and_then(|w| w.before.as_ref())
    {
        storage::atomic(&root.join("bitcoin.py.before-update"), old)?;
    }
    std::fs::remove_file(marker).map_err(|e| e.to_string())?;
    storage::sync(&root)
}

fn published_icon(files: &Files) -> Result<Vec<u8>, String> {
    let named = files.get("icon.png").or_else(|| files.get("bitcoin.png"));
    let image = named.or_else(|| {
        files
            .iter()
            .find(|(name, _)| Path::new(name).extension() == Some(std::ffi::OsStr::new("png")))
            .map(|(_, bytes)| bytes)
    });
    image.cloned().ok_or_else(||"Catalog package has no icon image; publisher must include one in its verified inventory".into())
}
