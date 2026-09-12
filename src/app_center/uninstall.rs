//! Receipt-scoped uninstall; saved data remains and removals use the install journal.
use super::{
    install, metadata, running,
    storage::{self, FileData, Locations},
    transaction::{self, Write},
};
use std::{
    collections::BTreeSet,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn uninstall(loc: &Locations, package: &metadata::Package) -> Result<(), String> {
    use running::Processes;
    metadata::identity(&package.id)?;
    let root = loc.root(package);
    let entry = installed_entry(&root, package)?;
    if !running::Native.list(&entry)?.is_empty() {
        return Err("Close the app before uninstalling; no files were removed".into());
    }
    install::recover(loc, package)?;
    let writes = plan(loc, package)?;
    if !running::Native.list(&entry)?.is_empty() {
        return Err("App started again; uninstall cancelled".into());
    }
    let marker = root.join(".installation-pending");
    storage::read(&marker, 1024)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let journal = install::journal_root(loc, package).join(format!("uninstall-{stamp}"));
    storage::atomic(
        &marker,
        &FileData {
            bytes: b"Uninstall incomplete; check and repair.\n".to_vec(),
            mode: 0o600,
        },
    )?;
    transaction::apply(&journal, &writes)?;
    std::fs::remove_file(marker).map_err(|e| e.to_string())?;
    storage::sync(&root)
}
fn installed_entry(root: &Path, p: &metadata::Package) -> Result<std::path::PathBuf, String> {
    if p.legacy() {
        return Ok(root.join("bitcoin.py"));
    }
    if let Some(file) = storage::read(&root.join("app.toml"), metadata::FILE_LIMIT)? {
        let manifest = metadata::manifest(&file.bytes)?;
        if manifest["id"] != p.id {
            return Err("Installed app ID differs; uninstall refused".into());
        }
        return Ok(root.join(metadata::text(&manifest["entry"], 240)?));
    }
    metadata::path(&p.entry)?;
    Ok(root.join(&p.entry))
}
fn validate_owned_path(name: &str) -> Result<(), String> {
    metadata::path(name)?;
    if name.split('/').any(|c| {
        matches!(
            c.to_ascii_lowercase().as_str(),
            ".vitrallis-receipt.json" | ".installation-pending" | ".venv" | "runtime"
        )
    }) {
        return Err("Inventory collides with installer/runtime state".into());
    }
    Ok(())
}
fn plan(loc: &Locations, p: &metadata::Package) -> Result<Vec<Write>, String> {
    let root = loc.root(p);
    let receipt = install::receipt(&root)?;
    let mut paths = BTreeSet::new();
    if let Some(receipt) = receipt {
        if receipt["origin"] != p.origin.as_str()
            || receipt["repository"] != p.repository.as_str()
            || receipt["id"] != p.id
        {
            return Err("Installed origin differs; uninstall refused".into());
        }
        for name in receipt["files"]
            .as_object()
            .ok_or("Invalid receipt")?
            .keys()
        {
            validate_owned_path(name)?;
            paths.insert(root.join(name));
        }
    } else if p.legacy() && storage::read(&root.join("bitcoin.py"), metadata::FILE_LIMIT)?.is_some()
    {
        // The reviewed adapter has fixed ownership of its legacy entry/launcher/icon.
        // Without a receipt, remove other catalog files only when their hashes match.
        paths.insert(root.join("bitcoin.py"));
        for file in &p.files {
            validate_owned_path(&file.path)?;
            if let Some(local) = storage::read(&root.join(&file.path), metadata::FILE_LIMIT)?
                && storage::sha(&local.bytes) == file.sha256
            {
                paths.insert(root.join(&file.path));
            }
        }
    } else {
        return Err("No installed app receipt; uninstall refused".into());
    }
    let launcher = if p.legacy() {
        root.join("launch")
    } else {
        loc.state.join("launchers").join(&p.id)
    };
    paths.insert(launcher.clone());
    if p.legacy() {
        paths.insert(root.join("bitcoin.png"));
    }
    let desktop = if p.legacy() {
        "pocket-bitcoin.desktop".into()
    } else {
        format!("{}.desktop", p.id)
    };
    let exec = format!("Exec={}", install::desktop_quote(&launcher)?);
    for directory in [loc.data.join("applications"), loc.home.join("Desktop")] {
        let path = directory.join(&desktop);
        if let Some(file) = storage::read(&path, metadata::FILE_LIMIT)?
            && std::str::from_utf8(&file.bytes).is_ok_and(|s| s.lines().any(|line| line == exec))
        {
            paths.insert(path);
        }
    }
    let mut writes = paths
        .into_iter()
        .map(transaction::remove)
        .collect::<Result<Vec<_>, _>>()?;
    if p.legacy() {
        remove_menu(loc, &launcher, &mut writes)?;
    }
    // Keep ownership metadata until the remaining removals have succeeded.
    writes.push(transaction::remove(root.join(".vitrallis-receipt.json"))?);
    Ok(writes)
}
fn remove_menu(loc: &Locations, launcher: &Path, writes: &mut Vec<Write>) -> Result<(), String> {
    let path = loc.home.join(".pocket-home/config.json");
    let Some(before) = storage::read(&path, 1024 * 1024)? else {
        return Ok(());
    };
    let mut value = metadata::json(&before.bytes)?;
    let pages = value["pages"]
        .as_array_mut()
        .ok_or("Invalid PocketHome pages")?;
    let quoted = format!("'{}'", launcher.to_string_lossy().replace('\'', "'\\''"));
    let mut changed = false;
    for page in pages {
        let items = page["items"]
            .as_array_mut()
            .ok_or("Invalid PocketHome items")?;
        let count = items.len();
        items.retain(|item| {
            item["shell"] != quoted && item["shell"] != launcher.to_string_lossy().as_ref()
        });
        changed |= count != items.len();
    }
    if !changed {
        return Ok(());
    }
    let after = FileData {
        bytes: serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
        mode: before.mode,
    };
    writes.push(Write {
        path,
        before: Some(before),
        after: Some(after),
    });
    Ok(())
}
