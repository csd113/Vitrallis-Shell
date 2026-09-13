//! Discover installed manifest packages and the built-in App Center.
use super::{
    metadata,
    storage::{self, Locations},
};
use crate::{
    app::{AppEntry, AppManifest},
    discovery::Catalog,
};
pub fn integrate(catalog: &mut Catalog) {
    match Locations::current() {
        Ok(loc) => {
            if let Err(error) = installed(catalog, &loc) {
                catalog.diagnostics.push(error);
            }
        }
        Err(error) => catalog.diagnostics.push(error),
    }
    catalog.apps.retain(|a| a.id != super::TILE_ID);
    catalog.apps.insert(
        0,
        AppEntry {
            source: crate::app::AppSource::System,
            id: super::TILE_ID.into(),
            name: "App Center".into(),
            icon: None,
            manifest: AppManifest {
                entry: "/vitrallis/builtin/app-center".into(),
                ..AppManifest::default()
            },
            unavailable: None,
        },
    );
}
pub fn refresh_apps(apps: &[AppEntry]) -> Result<Vec<AppEntry>, String> {
    let loc = Locations::current()?;
    let mut catalog = Catalog {
        apps: apps
            .iter()
            .filter(|a| a.source != crate::app::AppSource::AppCenter)
            .cloned()
            .collect(),
        ..Catalog::default()
    };
    installed(&mut catalog, &loc)?;
    for error in &catalog.diagnostics {
        eprintln!("level=warn event=installed_app_discovery error={error:?}");
    }
    Ok(catalog.apps)
}
pub(super) fn installed(catalog: &mut Catalog, loc: &Locations) -> Result<(), String> {
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
                .push("App discovery limited to 1000 directories".into());
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
                return Err("App directory must match app ID".into());
            }
            let entry = path.join(metadata::text(&v["entry"], 240)?);
            storage::read(&entry, metadata::FILE_LIMIT)?.ok_or("App entry missing")?;
            let launch = loc.state.join("launchers").join(id);
            let available =
                storage::read(&launch, metadata::FILE_LIMIT)?.is_some_and(|d| d.mode & 0o111 != 0);
            let pending = storage::read(&path.join(".installation-pending"), 1024)?.is_some();
            Ok(Some(AppEntry {
                source: crate::app::AppSource::AppCenter,
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
                // An imported alias of the installed launcher must not suppress
                // the canonical managed tile (and bypass uninstall). Explicit
                // custom shortcuts remain independent even for the same target.
                catalog.apps.retain(|old| {
                    old.source == crate::app::AppSource::Custom
                        || old.manifest.entry != app.manifest.entry
                        || old.manifest.runtime.is_some()
                        || !old.manifest.args.is_empty()
                });
                for old in &mut catalog.apps {
                    if old.id == app.id {
                        old.id = format!(
                            "vitrallis-discovered-{}",
                            storage::sha(
                                format!("{:?}:{}:{:?}", old.source, old.id, old.manifest)
                                    .as_bytes()
                            )
                        );
                    }
                }
                // Match full discovery ordering, which appends custom records
                // after installed apps. Retain custom identities across refresh.
                let index = catalog
                    .apps
                    .iter()
                    .position(|old| old.source == crate::app::AppSource::Custom)
                    .unwrap_or(catalog.apps.len());
                catalog.apps.insert(index, app);
            }
            Ok(None) => (),
            Err(e) => catalog.diagnostics.push(e),
        }
    }
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
        installed(&mut catalog, &loc)?;
        assert!(catalog.apps.is_empty());
        let root = loc.data.join("vitrallis/apps/io.vitrallis.hello");
        for (name, bytes) in [
            (
                "app.toml",
                include_bytes!("../../tests/fixtures/app-center/app.toml").as_slice(),
            ),
            ("main.py", b"print('hello')".as_slice()),
        ] {
            storage::atomic(
                &root.join(name),
                &storage::FileData {
                    bytes: bytes.to_vec(),
                    mode: 0o644,
                },
            )?;
        }
        installed(&mut catalog, &loc)?;
        installed(&mut catalog, &loc)?;
        assert_eq!(catalog.apps.len(), 1);
        assert!(catalog.apps[0].unavailable.is_some());
        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use super::super::{install, tests};
    use super::*;

    #[test]
    fn installed_manifest_supplies_identity_icon_and_launch_entry() -> Result<(), String> {
        let (_scratch, loc) = tests::locations()?;
        let (mut package, mut files) = tests::generic()?;
        package.entry = "tools/start.py".into();
        let manifest = String::from_utf8(files["app.toml"].clone()).map_err(|e| e.to_string())?;
        files.insert(
            "app.toml".into(),
            manifest
                .replace("entry = \"main.py\"", "entry = \"tools/start.py\"")
                .into_bytes(),
        );
        files.insert(
            "tools/start.py".into(),
            b"print('manifest entry')\n".to_vec(),
        );
        let package = tests::inventory(package, &files);
        install::install(&loc, &install::prepare(&loc, package.clone(), files)?)?;
        let mut catalog = Catalog::default();
        installed(&mut catalog, &loc)?;
        assert!(catalog.diagnostics.is_empty());
        assert_eq!(catalog.apps.len(), 1);
        let app = &catalog.apps[0];
        assert_eq!(app.id, package.id);
        assert_eq!(app.name, package.name);
        assert_eq!(app.icon, Some(loc.root(&package).join("icon.png")));
        assert!(app.unavailable.is_none());
        let output = std::process::Command::new(&app.manifest.entry)
            .current_dir(&loc.home)
            .output()
            .map_err(|e| e.to_string())?;
        assert!(output.status.success());
        assert_eq!(output.stdout, b"manifest entry\n");
        Ok(())
    }
}
