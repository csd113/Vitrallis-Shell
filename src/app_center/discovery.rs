//! Merge native packages while retaining legacy launcher identities.
use super::{
    metadata,
    storage::{self, Locations},
};
use crate::{
    app::{AppEntry, AppManifest},
    discovery::Catalog,
};
pub fn integrate(catalog: &mut Catalog) {
    let loc = Locations::current();
    if let Ok(loc) = &loc {
        let old = loc.home.join(".local/share/pocket-update-apps/launch");
        let script = loc
            .home
            .join(".local/share/pocket-update-apps/update_apps.py");
        catalog.apps.retain(|a| {
            a.id != super::TILE_ID
                && a.id != "vitrallis-pocketchip-store"
                && a.manifest.entry != old
                && a.manifest.entry != script
                && !(a
                    .manifest
                    .entry
                    .to_str()
                    .is_some_and(super::running::python)
                    && a.manifest
                        .args
                        .first()
                        .is_some_and(|arg| arg == script.as_os_str()))
        });
        if let Err(error) = legacy(catalog, loc) {
            catalog.diagnostics.push(error);
        }
        if let Err(error) = native(catalog, loc) {
            catalog.diagnostics.push(error);
        }
    }
    catalog
        .apps
        .retain(|a| a.id != super::TILE_ID && a.id != "vitrallis-pocketchip-store");
    catalog.apps.push(AppEntry {
        id: super::TILE_ID.into(),
        name: "App Center".into(),
        icon: None,
        manifest: AppManifest {
            entry: "/vitrallis/builtin/app-center".into(),
            ..AppManifest::default()
        },
        unavailable: None,
    });
}
fn native(catalog: &mut Catalog, loc: &Locations) -> Result<(), String> {
    let root = loc.data.join("vitrallis/apps");
    storage::safe(&root)?;
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    for (index, item) in entries.enumerate() {
        if index >= 1000 {
            catalog
                .diagnostics
                .push("Native app discovery limited to 1000 directories".into());
            break;
        }
        let item = item.map_err(|e| e.to_string())?;
        let result = (|| {
            let path = item.path();
            storage::safe(&path)?;
            let Some(file) = storage::read(&path.join("app.toml"), metadata::FILE_LIMIT)? else {
                return Ok(None);
            };
            let v = metadata::manifest(&file.bytes)?;
            let id = metadata::text(&v["id"], 128)?;
            if item.file_name() != std::ffi::OsStr::new(id) {
                return Err("Native directory must match app ID".into());
            }
            let entry = path.join(metadata::text(&v["entry"], 240)?);
            storage::read(&entry, metadata::FILE_LIMIT)?.ok_or("Native entry missing")?;
            let launch = loc.state.join("launchers").join(id);
            let available =
                storage::read(&launch, metadata::FILE_LIMIT)?.is_some_and(|d| d.mode & 0o111 != 0);
            let pending = storage::read(&path.join(".installation-pending"), 1024)?.is_some();
            Ok(Some(AppEntry {
                id: id.into(),
                name: metadata::text(&v["name"], 1000)?.into(),
                icon: Some(path.join("icon.png")),
                manifest: AppManifest {
                    entry: launch,
                    ..AppManifest::default()
                },
                unavailable: if pending {
                    Some("Installation incomplete; repair in App Center".into())
                } else {
                    (!available).then(|| "Missing launcher; repair in App Center".into())
                },
            }))
        })();
        match result {
            Ok(Some(app)) => {
                if !catalog
                    .apps
                    .iter()
                    .any(|old| old.manifest.entry == app.manifest.entry)
                {
                    if catalog.apps.iter().any(|old| old.id == app.id) {
                        catalog.diagnostics.push(format!(
                            "Native app ID conflicts with configured launcher: {}",
                            app.id
                        ));
                    } else {
                        catalog.apps.push(app);
                    }
                }
            }
            Ok(None) => (),
            Err(e) => catalog.diagnostics.push(e),
        }
    }
    Ok(())
}

fn legacy(catalog: &mut Catalog, loc: &Locations) -> Result<(), String> {
    let root = loc.home.join(".local/share/pocket-bitcoin");
    let entry = root.join("bitcoin.py");
    if storage::read(&entry, metadata::FILE_LIMIT)?.is_none() {
        return Ok(());
    }
    let launch = root.join("launch");
    if catalog.apps.iter().any(|a| {
        a.manifest.entry == entry || a.manifest.entry == launch || a.id == metadata::BITCOIN
    }) {
        return Ok(());
    }
    let available =
        storage::read(&launch, metadata::FILE_LIMIT)?.is_some_and(|d| d.mode & 0o111 != 0);
    let pending = storage::read(&root.join(".installation-pending"), 1024)?.is_some();
    catalog.apps.push(AppEntry {
        id: metadata::BITCOIN.into(),
        name: "Bitcoin CAD".into(),
        icon: Some(root.join("bitcoin.png")),
        manifest: AppManifest {
            entry: launch,
            ..AppManifest::default()
        },
        unavailable: (!available || pending)
            .then(|| "Incomplete installation; repair in App Center".into()),
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_storage_has_no_baked_apps_and_installed_entries_deduplicate() -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let home = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let data = home.join("data");
        let loc = Locations {
            sources: home.join("sources.json"),
            state: data.join("vitrallis/app-center"),
            home,
            data,
        };
        let mut catalog = Catalog::default();
        native(&mut catalog, &loc)?;
        legacy(&mut catalog, &loc)?;
        assert!(catalog.apps.is_empty());
        let root = loc.home.join(".local/share/pocket-bitcoin");
        storage::atomic(
            &root.join("bitcoin.py"),
            &storage::FileData {
                bytes: b"VERSION = '1.0.0'".to_vec(),
                mode: 0o644,
            },
        )?;
        legacy(&mut catalog, &loc)?;
        legacy(&mut catalog, &loc)?;
        assert_eq!(catalog.apps.len(), 1);
        assert!(catalog.apps[0].unavailable.is_some());
        Ok(())
    }
}
