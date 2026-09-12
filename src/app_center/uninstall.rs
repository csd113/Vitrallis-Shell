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
        return Ok(root.join(metadata::text(&manifest["entry"], 240)?));
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
