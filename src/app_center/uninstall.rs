//! Receipt-scoped uninstall; saved data remains and removals use the install journal.
use super::{
    install, metadata, running, runtime_cleanup,
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
    let root = loc.apps().join(id);
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
    transaction::commit(&journal, &writes, &marker)?;
    transaction::discard_completed_removals(&journal, |path| {
        runtime_cleanup::owned_file(&root, path)
    })
    .map_err(|e| format!("App uninstalled; runtime backup cleanup failed: {e}"))
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
    let receipt_write = transaction::remove(root.join(".vitrallis-receipt.json"))?;
    let mut bytes = receipt_write
        .before
        .as_ref()
        .map_or(0, |data| data.bytes.len());
    let mut writes = Vec::new();
    for path in paths {
        let write = transaction::remove(path)?;
        bytes += write.before.as_ref().map_or(0, |data| data.bytes.len());
        // Bound locally edited payloads before retaining all runtime images.
        if bytes > 32 * 1024 * 1024 {
            return Err("App payload exceeds removal bounds; no files were removed".into());
        }
        writes.push(write);
    }
    runtime_cleanup::plan(&root, &mut writes)?;
    // Keep ownership metadata until the remaining removals have succeeded.
    writes.push(receipt_write);
    Ok(writes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_center::tests::{generic, locations};

    #[test]
    fn generated_dependencies_are_journaled_and_custom_data_survives() -> Result<(), String> {
        let (_scratch, loc) = locations()?;
        let (package, files) = generic()?;
        install::install(
            &loc,
            &install::prepare(&loc, package.clone(), files.clone())?,
        )?;
        let root = loc.root(&package);
        let runtime = root
            .join("runtime")
            .join(storage::sha(&files["requirements.txt"]));
        let executable = runtime.join("bin/python3");
        let library = runtime.join("lib/site-packages/dependency.py");
        // Real interpreter copies exceed the package's 2 MiB per-file limit.
        let interpreter = FileData {
            bytes: vec![42; 3 * 1024 * 1024],
            mode: 0o755,
        };
        storage::atomic(&executable, &interpreter)?;
        storage::atomic(
            &library,
            &FileData {
                bytes: b"generated dependency".to_vec(),
                mode: 0o600,
            },
        )?;
        let saved = FileData {
            bytes: b"user data".to_vec(),
            mode: 0o600,
        };
        let retained = [
            loc.app_data(&package.id)?.join("a note.txt"),
            root.join(".venv/custom.txt"),
            root.join("runtime/custom/user-note.txt"),
            root.join("unmanaged.txt"),
        ];
        for path in &retained {
            storage::atomic(path, &saved)?;
        }
        let writes = plan(&loc, &package)?;
        assert_eq!(
            writes.last().map(|w| &w.path),
            Some(&root.join(".vitrallis-receipt.json"))
        );
        let journal = install::journal_root(&loc, &package).join("injected-failure");
        assert!(
            transaction::apply_with(&journal, &writes, |i| {
                if writes[i].path == executable {
                    Err("interrupted runtime removal".into())
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert_eq!(
            storage::read(&executable, metadata::BUNDLE_LIMIT)?,
            Some(interpreter)
        );
        assert_eq!(install::label(&loc, &package)?, "0.1.0");
        install::recover(&loc, &package)?;
        uninstall(&loc, &package)?;
        assert!(!executable.exists());
        assert!(!library.exists());
        assert!(!root.join(".vitrallis-receipt.json").exists());
        let mut reclaimed = 0;
        for entry in
            std::fs::read_dir(install::journal_root(&loc, &package)).map_err(|e| e.to_string())?
        {
            let journal = entry.map_err(|e| e.to_string())?.path();
            if let Some(completed) =
                storage::read(&journal.join("completed.json"), metadata::CATALOG_LIMIT)?
            {
                let rows = metadata::json(&completed.bytes)?;
                for (i, row) in rows
                    .as_array()
                    .ok_or("missing journal rows")?
                    .iter()
                    .enumerate()
                {
                    let path =
                        std::path::PathBuf::from(row["path"].as_str().ok_or("missing path")?);
                    if runtime_cleanup::owned_file(&root, &path) {
                        assert!(!journal.join(format!("{i}.before")).exists());
                        reclaimed += 1;
                    }
                }
            }
        }
        assert_eq!(reclaimed, 2);
        for path in &retained {
            assert_eq!(storage::read(path, 100)?, Some(saved.clone()));
        }
        install::install(&loc, &install::prepare(&loc, package.clone(), files)?)?;
        assert_eq!(install::label(&loc, &package)?, "0.1.0");
        for path in &retained {
            assert_eq!(storage::read(path, 100)?, Some(saved.clone()));
        }
        Ok(())
    }

    #[test]
    fn oversized_runtime_refuses_uninstall_without_changing_payload() -> Result<(), String> {
        let (_scratch, loc) = locations()?;
        let (package, files) = generic()?;
        install::install(&loc, &install::prepare(&loc, package.clone(), files)?)?;
        let root = loc.root(&package);
        let runtime = root.join("runtime").join("a".repeat(64));
        storage::directory(&runtime)?;
        let large =
            std::fs::File::create(runtime.join("too-large.so")).map_err(|e| e.to_string())?;
        large
            .set_len(u64::try_from(metadata::BUNDLE_LIMIT).map_err(|e| e.to_string())? + 1)
            .map_err(|e| e.to_string())?;
        assert!(uninstall(&loc, &package).is_err());
        assert_eq!(install::label(&loc, &package)?, "0.1.0");
        assert!(root.join("main.py").exists());
        assert!(!root.join(".installation-pending").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_runtime_links_refuse_uninstall_before_payload_mutation() -> Result<(), String> {
        use std::os::unix::fs::symlink;
        for hard_link in [false, true] {
            let (_scratch, loc) = locations()?;
            let (package, files) = generic()?;
            install::install(&loc, &install::prepare(&loc, package.clone(), files)?)?;
            let root = loc.root(&package);
            let runtime = root.join("runtime").join("a".repeat(64));
            storage::directory(&runtime)?;
            let external = loc.home.join("unrelated-note");
            std::fs::write(&external, b"preserve unrelated data").map_err(|e| e.to_string())?;
            let target = runtime.join("dependency");
            if hard_link {
                std::fs::hard_link(&external, &target).map_err(|e| e.to_string())?;
            } else {
                symlink(&external, &target).map_err(|e| e.to_string())?;
            }
            assert!(uninstall(&loc, &package).is_err());
            assert_eq!(install::label(&loc, &package)?, "0.1.0");
            assert!(root.join("main.py").is_file());
            assert!(!root.join(".installation-pending").exists());
            assert_eq!(
                std::fs::read(&external).map_err(|e| e.to_string())?,
                b"preserve unrelated data"
            );
        }
        Ok(())
    }
}
