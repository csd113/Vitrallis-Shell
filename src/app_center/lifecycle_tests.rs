//! Exercise production worker, filesystem, discovery and actual Python launch.
use super::*;

fn upgraded(
    mut package: Package,
    mut files: Files,
    entry: &str,
) -> Result<(Package, Files), String> {
    package.version = metadata::version("0.2.0")?;
    package.commit = "e".repeat(40);
    package.entry = entry.into();
    let manifest =
        String::from_utf8((*files.get("app.toml").ok_or("Missing fixture element")?).clone())
            .map_err(|e| e.to_string())?;
    drop(
        files.insert(
            "app.toml".into(),
            manifest
                .replace("0.1.0", "0.2.0")
                .replace("entry = \"main.py\"", &format!("entry = \"{entry}\""))
                .into_bytes(),
        ),
    );
    drop(files.insert(entry.into(), b"print('new release')\n".to_vec()));
    Ok((inventory(package, &files), files))
}

fn launch(loc: &Locations, p: &Package) -> Result<String, String> {
    let mut menu = crate::discovery::Catalog::default();
    discovery::installed(&mut menu, loc)?;
    let app = menu
        .apps
        .iter()
        .find(|app| app.id == p.id)
        .ok_or("Installed app absent from menu")?;
    assert!(app.unavailable.is_none(), "{:?}", app.unavailable);
    let result = std::process::Command::new(&app.manifest.entry)
        .output()
        .map_err(|e| e.to_string())?;
    assert!(result.status.success(), "{result:?}");
    String::from_utf8(result.stdout).map_err(|e| e.to_string())
}

#[test]
fn persistent_data_survives_update_uninstall_and_reinstall() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (package, mut files) = generic()?;
    drop(files.insert(
        "main.py".into(),
        b"import os,pathlib\np=pathlib.Path(os.environ['VITRALLIS_APP_DATA_DIR'])\nassert pathlib.Path.cwd()==p\nassert pathlib.Path(os.environ['VITRALLIS_APP_DIR']).is_dir()\n(p/'a saved note ! \\u00e9.txt').write_text('keep me',encoding='utf-8')\nprint(os.environ['VITRALLIS_APP_ID'])\n".to_vec(),
    ));
    let checked_package = inventory(package, &files);
    install::install(
        &loc,
        &install::prepare(&loc, checked_package.clone(), files.clone())?,
    )?;
    assert_eq!(
        launch(&loc, &checked_package)?,
        format!("{}\n", checked_package.id)
    );
    let saved = loc
        .app_data(&checked_package.id)?
        .join("a saved note ! é.txt");
    let (next, next_files) = upgraded(checked_package, files, "start.py")?;
    install::install(
        &loc,
        &install::prepare(&loc, next.clone(), next_files.clone())?,
    )?;
    assert_eq!(launch(&loc, &next)?, "new release\n");
    uninstall::uninstall(&loc, &next)?;
    assert!(!loc.root(&next).join("app.toml").exists());
    assert_eq!(
        std::fs::read_to_string(&saved).map_err(|e| e.to_string())?,
        "keep me"
    );
    install::install(&loc, &install::prepare(&loc, next.clone(), next_files)?)?;
    assert_eq!(launch(&loc, &next)?, "new release\n");
    assert_eq!(
        std::fs::read_to_string(&saved).map_err(|e| e.to_string())?,
        "keep me"
    );
    storage::atomic(
        &loc.root(&next).join(".installation-pending"),
        &storage::FileData {
            bytes: b"interrupted".to_vec(),
            mode: 0o600,
        },
    )?;
    let output = std::process::Command::new(loc.state.join("launchers").join(&next.id))
        .output()
        .map_err(|e| e.to_string())?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Installation incomplete"));
    Ok(())
}

#[test]
fn update_replaces_launcher_entry_removes_obsolete_files_and_launches_new_code()
-> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (package, mut files) = generic()?;
    drop(files.insert("obsolete.py".into(), b"print('old code')\n".to_vec()));
    let checked_package = inventory(package, &files);
    install::install(
        &loc,
        &install::prepare(&loc, checked_package.clone(), files.clone())?,
    )?;
    assert_eq!(launch(&loc, &checked_package)?, "fixture\n");
    assert!(files.remove("obsolete.py").is_some());
    let (upgraded_package, upgraded_files) = upgraded(checked_package, files, "start.py")?;
    install::install(
        &loc,
        &install::prepare(&loc, upgraded_package.clone(), upgraded_files)?,
    )?;
    assert!(!loc.root(&upgraded_package).join("obsolete.py").exists());
    assert_eq!(install::label(&loc, &upgraded_package)?, "0.2.0");
    assert_eq!(launch(&loc, &upgraded_package)?, "new release\n");
    uninstall::uninstall(&loc, &upgraded_package)?;
    let mut menu = crate::discovery::Catalog::default();
    discovery::installed(&mut menu, &loc)?;
    assert_eq!(menu.apps.len(), 0);
    Ok(())
}

#[test]
fn update_invalidates_same_size_timestamp_python_cache() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    drop(files.insert(
        "main.py".into(),
        b"from release import VERSION\nprint(VERSION)\n".to_vec(),
    ));
    drop(files.insert("release.py".into(), b"VERSION = 'old'\n".to_vec()));
    let checked_package = inventory(p, &files);
    install::install(
        &loc,
        &install::prepare(&loc, checked_package.clone(), files.clone())?,
    )?;
    // Deliberately produce unchecked hash bytecode. Python accepts it even after
    // source changes; removing the managed module's cache must prevent reuse.
    let compiled = std::process::Command::new("/usr/bin/python3").args(["-c",
        "import py_compile,sys; py_compile.compile(sys.argv[1], invalidation_mode=py_compile.PycInvalidationMode.UNCHECKED_HASH)"])
        .arg(loc.root(&checked_package).join("release.py")).status().map_err(|e| e.to_string())?;
    assert!(compiled.success());
    assert_eq!(launch(&loc, &checked_package)?, "old\n");
    let (upgraded_package, mut upgraded_files) = upgraded(checked_package, files, "main.py")?;
    drop(upgraded_files.insert(
        "main.py".into(),
        b"from release import VERSION\nprint(VERSION)\n".to_vec(),
    ));
    drop(upgraded_files.insert("release.py".into(), b"VERSION = 'new'\n".to_vec()));
    let checked_upgrade = inventory(upgraded_package, &upgraded_files);
    install::install(
        &loc,
        &install::prepare(&loc, checked_upgrade.clone(), upgraded_files)?,
    )?;
    assert_eq!(launch(&loc, &checked_upgrade)?, "new\n");
    Ok(())
}

#[cfg(unix)]
#[test]
fn update_removes_group_writable_python_bytecode_caches() -> Result<(), String> {
    use std::{fs, os::unix::fs::PermissionsExt};
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    drop(files.insert("release.py".into(), b"VERSION = 'old'\n".to_vec()));
    let checked_package = inventory(p, &files);
    install::install(
        &loc,
        &install::prepare(&loc, checked_package.clone(), files.clone())?,
    )?;
    // The device's shared umask 002 makes Python create group-writable caches;
    // the updater must still remove managed modules' stale bytecode.
    let cache = loc.root(&checked_package).join("__pycache__");
    fs::create_dir(&cache).map_err(|e| e.to_string())?;
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o775)).map_err(|e| e.to_string())?;
    let stale = cache.join("release.cpython-313.pyc");
    fs::write(&stale, b"stale bytecode").map_err(|e| e.to_string())?;
    fs::set_permissions(&stale, fs::Permissions::from_mode(0o664)).map_err(|e| e.to_string())?;
    let (next, upgraded_files) = upgraded(checked_package, files, "start.py")?;
    install::install(&loc, &install::prepare(&loc, next.clone(), upgraded_files)?)?;
    assert!(!stale.exists());
    assert_eq!(launch(&loc, &next)?, "new release\n");
    Ok(())
}

#[cfg(unix)]
#[test]
fn update_refuses_unsafe_python_cache_paths() -> Result<(), String> {
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };
    for damage in [
        "symlinked directory",
        "world-writable directory",
        "symlinked entry",
        "hardlinked entry",
    ] {
        let (_scratch, loc) = locations()?;
        let (p, mut files) = generic()?;
        drop(files.insert("release.py".into(), b"VERSION = 'old'\n".to_vec()));
        let checked_package = inventory(p, &files);
        install::install(
            &loc,
            &install::prepare(&loc, checked_package.clone(), files.clone())?,
        )?;
        let root = loc.root(&checked_package);
        let cache = root.join("__pycache__");
        fs::create_dir(&cache).map_err(|e| e.to_string())?;
        fs::set_permissions(&cache, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        match damage {
            "symlinked directory" => {
                fs::remove_dir(&cache).map_err(|e| e.to_string())?;
                symlink(root.join("elsewhere"), &cache).map_err(|e| e.to_string())?;
            }
            "world-writable directory" => {
                fs::set_permissions(&cache, fs::Permissions::from_mode(0o777))
                    .map_err(|e| e.to_string())?;
            }
            "hardlinked entry" => {
                let borrowed = root.join("borrowed.pyc");
                fs::write(&borrowed, b"shared").map_err(|e| e.to_string())?;
                fs::hard_link(&borrowed, cache.join("release.cpython-313.pyc"))
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                symlink(
                    root.join("release.py"),
                    cache.join("release.cpython-313.pyc"),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        let (next, upgraded_files) = upgraded(checked_package, files, "start.py")?;
        let error = install::prepare(&loc, next, upgraded_files)
            .err()
            .ok_or(damage)?;
        assert!(error.contains("Unsafe Python cache"), "{damage}: {error}");
    }
    Ok(())
}

#[test]
fn worker_keeps_catalog_across_sequential_mutations_and_local_scans_without_remote_refresh()
-> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let mut catalog_package = p;
    catalog_package.origin = sources::Repository::parse(sources::DEFAULT)?;
    catalog_package.repository = catalog_package.origin.clone();
    let fetch = transport(&catalog_package, &files)?;
    let (send, commands) = mpsc::channel();
    let (updates, receive) = mpsc::channel();
    for command in [
        Command::Check,
        Command::Install(vec![catalog_package.key()]),
        Command::Scan,
        Command::Uninstall(catalog_package.key()),
        Command::Install(vec![catalog_package.key()]),
    ] {
        send.send(command).map_err(|e| e.to_string())?;
    }
    drop(send);
    service(&loc, &commands, &updates, &AtomicBool::new(false), &fetch);
    drop(updates);
    let mut loaded = false;
    let mut states = Vec::new();
    for update in receive {
        match update {
            Update::Rows(rows) => {
                if loaded {
                    assert!(!rows.is_empty());
                }
                loaded |= !rows.is_empty();
            }
            Update::Row(row) => states.push(row.installed),
            Update::Done(result, _) => {
                let _completed_operation = result?;
            }
            Update::Sources(_)
            | Update::Progress(_)
            | Update::Confirm(_, _)
            | Update::SelectedInstalled(_) => (),
        }
    }
    assert_eq!(states, ["0.1.0", "0.1.0", "not installed", "0.1.0"]);
    assert_eq!(
        fetch
            .requests
            .borrow()
            .iter()
            .filter(|url| url.ends_with("apps.json"))
            .count(),
        1
    );
    assert_eq!(
        cache::load(&loc, &Sources::default())
            .first()
            .ok_or("Missing fixture element")?
            .installed,
        "0.1.0"
    );
    assert_eq!(launch(&loc, &catalog_package)?, "fixture\n");
    Ok(())
}

#[test]
fn failed_repository_refresh_retains_snapshot_and_other_repositories_remain_usable()
-> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let sources = test_sources(&p);
    let mut fetch = transport(&p, &files)?;
    let fetched = cache::fetch_documents(&loc, &sources, &fetch, |_| ());
    let mut rows = cache::store_documents(&loc, &sources, fetched, &mut vec![]);
    assert_eq!(rows.len(), 1);
    fetch.responses.clear();
    let refreshed_documents = cache::fetch_documents(&loc, &sources, &fetch, |_| ());
    let failed = cache::store_documents(&loc, &sources, refreshed_documents, &mut rows);
    assert!(failed.iter().any(|row| row.package.id == p.id && row.ready));
    assert!(
        failed
            .iter()
            .any(|row| row.status.contains("Repository unavailable"))
    );
    assert_eq!(
        cache::load(&loc, &sources)
            .first()
            .ok_or("Missing fixture element")?
            .package
            .id,
        p.id
    );
    Ok(())
}

#[test]
fn malformed_app_is_isolated_and_changelog_text_is_bounded_multiline_data() -> Result<(), String> {
    let (p, _) = generic()?;
    let mut value = catalog_value(&p);
    (*value.get_mut("apps").ok_or("Missing fixture element")?)
        .as_array_mut()
        .ok_or("apps")?
        .push(serde_json::json!({"id":"bad", "name":"Broken"}));
    let rows = metadata::catalog_entries(
        &p.origin,
        &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
    )?;
    assert!((*rows.first().ok_or("Missing fixture element")?).is_ok());
    assert!((*rows.get(1).ok_or("Missing fixture element")?).is_err());
    assert_eq!(
        cache::changelog(b"## 0.2.0\r\n\n- Change one\n- Change two"),
        Some("## 0.2.0\n\n- Change one\n- Change two".into())
    );
    for text in [b"".as_slice(), b"\xff", b"bad\x00text", &vec![b'x'; 65537]] {
        assert!(cache::changelog(text).is_none());
    }
    Ok(())
}

#[test]
fn pinned_git_execute_permissions_reach_installed_helper() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    drop(files.insert("helper.sh".into(), b"#!/bin/sh\nprintf helper\n".to_vec()));
    let checked_package = inventory(p, &files);
    let mut fetch = transport(&checked_package, &files)?;
    let tree = format!(
        "https://api.github.com/repos/{}/git/trees/{}?recursive=1",
        checked_package.repository.as_str(),
        "d".repeat(40)
    );
    let mut entries = metadata::json(
        fetch
            .responses
            .get(&tree)
            .ok_or("Missing fixture element")?,
    )?;
    for row in entries
        .get_mut("tree")
        .ok_or("Missing Git tree fixture")?
        .as_array_mut()
        .ok_or("tree")?
    {
        if row["path"] == "helper.sh" {
            row["mode"] = "100755".into();
        }
    }
    drop(fetch.responses.insert(
        tree,
        serde_json::to_vec(&entries).map_err(|e| e.to_string())?,
    ));
    let bundle = network::download(&fetch, &checked_package, |_| Ok(()))?;
    let plan =
        install::prepare_with_modes(&loc, checked_package.clone(), bundle.files, &bundle.modes)?;
    install::install(&loc, &plan)?;
    let output = std::process::Command::new(loc.root(&checked_package).join("helper.sh"))
        .output()
        .map_err(|e| e.to_string())?;
    assert!(output.status.success());
    assert_eq!(output.stdout, b"helper");
    Ok(())
}

#[test]
fn cached_changelog_and_icon_need_no_requests_on_reopen_or_unchanged_refresh() -> Result<(), String>
{
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    drop(files.insert(
        "CHANGELOG.md".into(),
        b"## 0.1.0\n\n- First release.\n".to_vec(),
    ));
    let checked_package = inventory(p, &files);
    let fetch = transport(&checked_package, &files)?;
    let sources = test_sources(&checked_package);
    let fetched = cache::fetch_documents(&loc, &sources, &fetch, |_| ());
    let mut rows = cache::store_documents(&loc, &sources, fetched, &mut vec![]);
    assert!(
        rows.first()
            .ok_or("Missing fixture element")?
            .package
            .changelog
            .as_ref()
            .is_some_and(|s| s.contains("First release"))
    );
    let count = fetch.requests.borrow().len();
    assert!(
        cache::load(&loc, &sources)
            .first()
            .ok_or("Missing fixture element")?
            .package
            .changelog
            .is_some()
    );
    assert_eq!(fetch.requests.borrow().len(), count);
    let refreshed_documents = cache::fetch_documents(&loc, &sources, &fetch, |_| ());
    let current = cache::store_documents(&loc, &sources, refreshed_documents, &mut rows);
    assert_eq!(current.len(), 1);
    assert_eq!(fetch.requests.borrow().len(), count + 3);
    Ok(())
}

#[test]
fn commit_failure_releases_lock_and_retry_repairs() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let prepared = install::prepare(&loc, p.clone(), files.clone())?;
    // A writer creates a target between plan and commit.
    storage::directory(&loc.root(&p))?;
    storage::atomic(
        &loc.root(&p).join("main.py"),
        &storage::FileData {
            bytes: b"foreign\n".to_vec(),
            mode: 0o644,
        },
    )?;
    let result = {
        let _lock = storage::Lock::take(&loc.state)?;
        install::install(&loc, &prepared)
    };
    assert!(
        result
            .as_ref()
            .is_err_and(|error| error.contains("File changed since preparation")),
        "{result:?}"
    );
    assert!(storage::Lock::take(&loc.state).is_ok());
    assert!(!loc.root(&p).join(".installation-pending").exists());
    std::fs::remove_file(loc.root(&p).join("main.py")).map_err(|e| e.to_string())?;
    install::install(&loc, &install::prepare(&loc, p.clone(), files)?)?;
    assert_eq!(launch(&loc, &p)?, "fixture\n");
    Ok(())
}

#[test]
fn presentation_refresh_budget_bounds_staged_bytes() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let checked_package = inventory(p, &files);
    let fetch = transport(&checked_package, &files)?;
    let mut staged = std::collections::BTreeMap::new();
    let mut budget = 0;
    cache::stage(
        &loc,
        &checked_package,
        Some(&fetch),
        &mut staged,
        &mut budget,
    );
    assert!(staged.is_empty());
    assert!(fetch.requests.borrow().is_empty());
    let mut available_budget = cache::PRESENTATION_BUDGET;
    cache::stage(
        &loc,
        &checked_package,
        Some(&fetch),
        &mut staged,
        &mut available_budget,
    );
    assert_eq!(staged.len(), 1);
    assert!(available_budget < cache::PRESENTATION_BUDGET);
    Ok(())
}

#[test]
#[cfg(unix)]
fn unsafe_persistent_data_is_rejected_before_package_mutation() -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let (_scratch, loc) = locations()?;
    let (package, files) = generic()?;
    let data = loc.app_data(&package.id)?;
    storage::directory(&data)?;
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    assert!(install::prepare(&loc, package.clone(), files.clone()).is_err());
    assert!(!loc.root(&package).exists());
    std::fs::remove_dir(&data).map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(&loc.home, &data).map_err(|e| e.to_string())?;
    assert!(install::prepare(&loc, package.clone(), files).is_err());
    assert!(!loc.root(&package).exists());
    Ok(())
}
