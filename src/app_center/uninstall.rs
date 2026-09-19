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

/// Resolve uninstall authority exclusively from local ownership metadata. This
/// works for removed/custom repositories and never acquires a remote catalog.
pub(super) fn local_package(loc: &Locations, id: &str) -> Result<metadata::Package, String> {
    metadata::identity(id)?;
    let root = loc.data.join("vitrallis/apps").join(id);
    let receipt = install::receipt(&root)?.ok_or("No installed app receipt; uninstall refused")?;
    if receipt["id"] != id {
        return Err("Installed receipt ID differs; uninstall refused".into());
    }
    let file = storage::read(&root.join("app.toml"), metadata::FILE_LIMIT)?
        .ok_or("Installed manifest missing")?;
    let manifest = metadata::manifest(&file.bytes)?;
    if manifest["id"] != id {
        return Err("Installed manifest ID differs; uninstall refused".into());
    }
    Ok(metadata::Package {
        runtime: metadata::RuntimeKind::parse(&manifest)?,
        origin: super::sources::Repository::parse(metadata::text(&receipt["origin"], 160)?)?,
        repository: super::sources::Repository::parse(metadata::text(
            &receipt["repository"],
            160,
        )?)?,
        id: id.into(),
        name: metadata::text(&manifest["name"], 1000)?.into(),
        entry: metadata::manifest_entry(&manifest)?,
        version: metadata::version(metadata::text(&receipt["version"], 32)?)?,
        commit: metadata::text(&receipt["commit"], 40)?.into(),
        description: "Installed application (local receipt)".into(),
        changelog: None,
        icon: None,
        permissions: serde_json::json!({}),
        installable: false,
        notes: "Uninstall removes receipt-owned files and retains application data.".into(),
        directory: String::new(),
        files: Vec::new(),
    })
}

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
    transaction::commit(&journal, &writes, &marker)
}
pub(super) fn installed_entry(
    root: &Path,
    p: &metadata::Package,
) -> Result<std::path::PathBuf, String> {
    if let Some(file) = storage::read(&root.join("app.toml"), metadata::FILE_LIMIT)? {
        let manifest = metadata::manifest(&file.bytes)?;
        if manifest["id"] != p.id {
            return Err("Installed app ID differs; uninstall refused".into());
        }
        return Ok(root.join(metadata::manifest_entry(&manifest)?));
    }
    metadata::path(&p.entry)?;
    Ok(root.join(&p.entry))
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
            install::validate_owned_path(name)?;
            paths.insert(root.join(name));
        }
    } else {
        return Err("No installed app receipt; uninstall refused".into());
    }
    let launcher = loc.state.join("launchers").join(&p.id);
    paths.insert(launcher.clone());
    let desktop = format!("{}.desktop", p.id);
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
    // Keep ownership metadata until the remaining removals have succeeded.
    writes.push(transaction::remove(root.join(".vitrallis-receipt.json"))?);
    Ok(writes)
}
