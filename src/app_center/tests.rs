use super::*;
use metadata::{Files, Package};
use std::collections::BTreeMap;
use storage::{FileData, Locations};

#[test]
fn desktop_uninstall_resolves_custom_repository_receipts_without_network() -> Result<(), String> {
    struct Offline(std::cell::Cell<usize>);
    impl network::Fetch for Offline {
        fn fetch(&self, _: &str, _: usize) -> Result<Vec<u8>, String> {
            self.0.set(self.0.get() + 1);
            Err("No network permitted".into())
        }
    }
    let (_scratch, loc) = locations()?;
    let (package, files) = generic()?;
    install::install(&loc, &install::prepare(&loc, package.clone(), files)?)?;
    let local = uninstall::local_package(&loc, &package.id)?;
    assert_eq!(local.origin, package.origin);
    assert_eq!(local.repository, package.repository);
    assert_eq!(local.entry, package.entry);
    let target = loc.root(&package).join(&package.entry);
    let offline = Offline(std::cell::Cell::new(0));
    let (send, commands) = mpsc::channel();
    let (updates, receive) = mpsc::channel();
    send.send(Command::SelectInstalled(package.id.clone()))
        .map_err(|e| e.to_string())?;
    send.send(Command::Uninstall(local.key()))
        .map_err(|e| e.to_string())?;
    drop(send);
    service(&loc, &commands, &updates, &AtomicBool::new(false), &offline);
    let events: Vec<_> = receive.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Update::SelectedInstalled(key) if key == &local.key()))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Update::Done(Err(_), _))),
        "{events:?}"
    );
    assert_eq!(offline.0.get(), 0);
    assert!(!target.exists());
    assert!(uninstall::local_package(&loc, &package.id).is_err());
    Ok(())
}

#[test]
fn managed_discovery_keeps_canonical_authority_and_independent_custom_aliases() -> Result<(), String>
{
    let (_scratch, loc) = locations()?;
    let (package, files) = generic()?;
    install::install(&loc, &install::prepare(&loc, package, files)?)?;
    let mut catalog = crate::discovery::Catalog::default();
    discovery::installed(&mut catalog, &loc)?;
    let managed = catalog.apps[0].clone();
    let mut alias = managed.clone();
    alias.source = crate::app::AppSource::PocketHome;
    alias.id = "imported-alias".into();
    let mut custom = alias.clone();
    custom.source = crate::app::AppSource::Custom;
    custom.id = format!("{}{}", crate::shortcuts::PREFIX, "a".repeat(64));
    let mut collision = alias.clone();
    collision.id = managed.id.clone();
    collision.manifest.entry = "/bin/echo".into();
    catalog.apps = vec![alias, collision, custom.clone()];
    discovery::installed(&mut catalog, &loc)?;
    assert_eq!(catalog.apps.len(), 3);
    assert!(catalog.apps.contains(&custom));
    assert!(catalog.apps.contains(&managed));
    let managed_index = catalog
        .apps
        .iter()
        .position(|app| app.id == managed.id)
        .ok_or("managed tile")?;
    let custom_index = catalog
        .apps
        .iter()
        .position(|app| app.id == custom.id)
        .ok_or("custom tile")?;
    assert!(
        managed_index < custom_index,
        "live refresh must match restart ordering"
    );
    assert!(
        catalog
            .apps
            .iter()
            .any(|app| app.source == crate::app::AppSource::PocketHome
                && app.id.starts_with("vitrallis-discovered-"))
    );
    discovery::installed(&mut catalog, &loc)?;
    assert_eq!(
        catalog
            .apps
            .iter()
            .filter(|app| app.source == crate::app::AppSource::AppCenter)
            .count(),
        1
    );
    Ok(())
}
#[path = "lifecycle_tests.rs"]
mod lifecycle;
fn fixture(name: &str) -> std::path::PathBuf {
    std::env::var_os("VITRALLIS_TEST_FIXTURES")
        .map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/app-center")
            },
            std::path::PathBuf::from,
        )
        .join(name)
}
fn fixture_icon() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&[20, 100, 160])
            .map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}
pub(super) fn generic() -> Result<(Package, Files), String> {
    let files = Files::from([
        (
            "app.toml".into(),
            std::fs::read(fixture("app.toml")).map_err(|e| e.to_string())?,
        ),
        ("main.py".into(), b"print('fixture')\n".to_vec()),
        ("icon.png".into(), fixture_icon()?),
        ("README.md".into(), b"Fixture package".to_vec()),
        ("requirements.txt".into(), b"# no dependencies\n".to_vec()),
        ("assets/greeting.txt".into(), b"Hello".to_vec()),
    ]);
    let v = metadata::manifest(&files["app.toml"])?;
    let origin = sources::Repository::parse("example/catalog")?;
    let p = Package {
        runtime: metadata::RuntimeKind::Python,
        origin: origin.clone(),
        repository: origin,
        id: v["id"].as_str().ok_or("id")?.into(),
        name: v["name"].as_str().ok_or("name")?.into(),
        description: "Fixture application".into(),
        changelog: None,
        icon: None,
        version: metadata::version("0.1.0")?,
        entry: "main.py".into(),
        permissions: v["permissions"].clone(),
        installable: true,
        notes: "Fixture".into(),
        commit: "a".repeat(40),
        directory: "apps/hello".into(),
        files: vec![],
    };
    Ok((inventory(p, &files), files))
}
pub(super) fn inventory(mut p: Package, files: &Files) -> Package {
    p.files = files
        .iter()
        .map(|(path, b)| metadata::FileRow {
            path: path.clone(),
            size: b.len(),
            sha256: storage::sha(b),
        })
        .collect();
    p
}
pub(super) fn locations() -> Result<(crate::test_support::Scratch, Locations), String> {
    let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
    let home = scratch.0.canonicalize().map_err(|e| e.to_string())?;
    let data = home.join(".local/share");
    let loc = Locations {
        state: data.join("vitrallis/app-center"),
        sources: home.join(".config/vitrallis/app-center.json"),
        home,
        data,
    };
    Ok((scratch, loc))
}
#[test]
fn sources_normalize_persist_batch_and_keep_default() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let mut s = Sources::load(&loc.sources)?;
    assert_eq!(s.catalogs.len(), 1);
    s.edit(
        None,
        "https://github.com/CSD113/Vitrallis-Apps.git/; Example/One example/two,EXAMPLE/ONE",
    )?;
    assert_eq!(s.catalogs.len(), 3);
    s.remove(0);
    assert_eq!(s.catalogs[0].as_str(), sources::DEFAULT);
    s.save(&loc.sources)?;
    let mut s = Sources::load(&loc.sources)?;
    s.edit(Some(1), "new/repo")?;
    s.remove(1);
    s.save(&loc.sources)?;
    assert_eq!(Sources::load(&loc.sources)?.catalogs, s.catalogs);
    for input in [
        "http://github.com/a/b",
        "https://evil.test/a/b",
        "a/b/tree/main",
        "a/../b",
        "a/b?x",
        "a/b#x",
        "a/b.git@evil",
        "-a/b",
        "a--b/c",
    ] {
        assert!(sources::Repository::parse(input).is_err(), "{input}");
    }
    let bytes = std::fs::read(&loc.sources).map_err(|e| e.to_string())?;
    assert!(s.edit(None, "bad").is_err());
    assert_eq!(
        std::fs::read(&loc.sources).map_err(|e| e.to_string())?,
        bytes
    );
    Ok(())
}
#[test]
fn default_catalog_lists_both_apps_and_respects_installation_flags() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let origin = sources::Repository::parse(sources::DEFAULT)?;
    let mut packages = metadata::catalog(
        &origin,
        include_bytes!("../../tests/fixtures/app-center/catalog.json"),
    )?;
    let mut fetch = transport(&packages[0], &Files::new())?;
    replace_catalog(&mut fetch, &packages[0], &packages)?;

    let rows = check_all(&loc, &Sources::default(), &fetch, |_| ());
    assert_eq!(
        rows.iter()
            .map(|row| row.package.name.as_str())
            .collect::<Vec<_>>(),
        ["Bitcoin Dashboard", "Vitrallis Debug"]
    );
    assert!(rows.iter().all(|row| row.ready && row.package.installable));
    assert_eq!(fetch.requests.borrow().len(), 3);
    assert!(!loc.state.exists());

    // Exercise the same worker messages consumed by the screen after Check.
    let (send, commands) = mpsc::channel();
    let (updates, receive) = mpsc::channel();
    send.send(Command::Check).map_err(|e| e.to_string())?;
    drop(send);
    service(&loc, &commands, &updates, &AtomicBool::new(false), &fetch);
    drop(updates);
    let mut displayed_names = Vec::new();
    let mut messages = Vec::new();
    for update in receive {
        match update {
            Update::Rows(rows) => {
                displayed_names = rows.into_iter().map(|row| row.package.name).collect();
            }
            Update::Done(result, _) => messages.push(result?),
            _ => (),
        }
    }
    assert_eq!(displayed_names, ["Bitcoin Dashboard", "Vitrallis Debug"]);
    assert_eq!(
        messages,
        [
            "Refresh to load available apps",
            "Refresh complete: 2 entries. Select an app.",
        ]
    );
    for package in &mut packages {
        package.installable = false;
    }
    replace_catalog(&mut fetch, &packages[0], &packages)?;
    let disabled = check_all(&loc, &Sources::default(), &fetch, |_| ());
    assert_eq!(disabled.len(), 2);
    assert!(
        disabled
            .iter()
            .all(|row| !row.ready && row.status.starts_with("Disabled:"))
    );
    Ok(())
}

#[test]
fn actual_catalog_and_manifest_contracts_reject_malicious_metadata() -> Result<(), String> {
    let bytes = std::fs::read(fixture("catalog.json")).map_err(|e| e.to_string())?;
    let origin = sources::Repository::parse(sources::DEFAULT)?;
    let p = metadata::catalog(&origin, &bytes)?;
    assert_eq!(p[0].directory, "apps/bitcoin-dashboard");
    assert_eq!(p[0].entry, "main.py");
    assert!(p[0].installable);
    assert!(metadata::json(br#"{"a":1,"a":2}"#).is_err());
    assert!(metadata::json(br#"{"nested":{"a":1,"a":2}}"#).is_err());
    for names in [
        vec!["../bad"],
        vec!["a", "a/b"],
        vec!["assets/a", "Assets/b"],
        vec!["main.py", "MAIN.py"],
        vec!["a//b"],
        vec!["a\\b"],
    ] {
        assert!(metadata::check_paths(names.into_iter()).is_err());
    }
    for value in ["1.01.0", "v1.0.0", "1.0.0-rc.1", "1.0.0+build"] {
        assert!(metadata::version(value).is_err());
    }
    assert!(metadata::version("1.10.0")? > metadata::version("1.9.0")?);
    let v = metadata::json(&bytes)?;
    for (key, bad) in [
        ("installable", serde_json::json!(1)),
        ("version", serde_json::json!("01.0.0")),
        ("id", serde_json::json!("../bad")),
        ("runtime", serde_json::json!("shell")),
    ] {
        let mut next = v.clone();
        next["apps"][0][key] = bad;
        assert!(
            metadata::catalog(
                &origin,
                &serde_json::to_vec(&next).map_err(|e| e.to_string())?
            )
            .is_err()
        );
    }
    let (p, files) = generic()?;
    metadata::validate_bundle(&p, &files)?;
    let mut bad = files.clone();
    bad.get_mut("main.py").ok_or("main")?.push(0);
    assert!(metadata::validate_bundle(&p, &bad).is_err());
    let mut bad = files.clone();
    bad.insert(
        "app.toml".into(),
        b"manifest_version = 1\nmanifest_version = 1".to_vec(),
    );
    assert!(metadata::validate_bundle(&inventory(p.clone(), &bad), &bad).is_err());
    let mut p = p;
    p.name = "disagreement".into();
    assert!(metadata::validate_bundle(&p, &files).is_err());
    Ok(())
}
struct FixtureFetch {
    responses: BTreeMap<String, Vec<u8>>,
    requests: std::cell::RefCell<Vec<String>>,
}
impl network::Fetch for FixtureFetch {
    fn fetch(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        self.requests.borrow_mut().push(url.into());
        let b = self
            .responses
            .get(url)
            .ok_or_else(|| format!("Fixture missing: {url}"))?;
        if b.len() > limit {
            return Err("bounded fixture".into());
        }
        Ok(b.clone())
    }
    fn fetch_progress(
        &self,
        url: &str,
        limit: usize,
        progress: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<Vec<u8>, String> {
        let bytes = network::Fetch::fetch(self, url, limit)?;
        network::read_download(std::io::Cursor::new(bytes), limit, progress)
    }
}
fn catalog_value(p: &Package) -> serde_json::Value {
    serde_json::json!({"schema_version":1,"apps":[{"id":p.id,"name":p.name,"version":p.version.to_string(),"runtime":"python","description":"fixture","entry":p.entry,"permissions":p.permissions,"installable":p.installable,"compatibility_notes":p.notes,"source":{"repository":p.repository.as_str(),"commit":p.commit,"path":p.directory},"files":p.files.iter().map(|r|serde_json::json!({"path":r.path,"size":r.size,"sha256":r.sha256})).collect::<Vec<_>>()}]})
}
fn transport(p: &Package, files: &Files) -> Result<FixtureFetch, String> {
    let repo = p.origin.as_str();
    let source = p.repository.as_str();
    let mut responses = BTreeMap::new();
    let mut add = |url: String, v: serde_json::Value| -> Result<(), String> {
        responses.insert(url, serde_json::to_vec(&v).map_err(|e| e.to_string())?);
        Ok(())
    };
    add(
        format!("https://api.github.com/repos/{repo}"),
        serde_json::json!({"default_branch":"release/catalog"}),
    )?;
    add(
        format!("https://api.github.com/repos/{repo}/commits/release%2Fcatalog"),
        serde_json::json!({"sha":"b".repeat(40)}),
    )?;
    add(
        format!(
            "https://raw.githubusercontent.com/{repo}/{}/apps.json",
            "b".repeat(40)
        ),
        catalog_value(p),
    )?;
    let tree = "c".repeat(40);
    let tree2 = "d".repeat(40);
    let (base, slug) = p.directory.split_once('/').ok_or("directory")?;
    add(
        format!(
            "https://api.github.com/repos/{source}/git/trees/{}",
            p.commit
        ),
        serde_json::json!({"truncated":false,"tree":[{"path":base,"type":"tree","mode":"040000","sha":tree}]}),
    )?;
    add(
        format!("https://api.github.com/repos/{source}/git/trees/{tree}"),
        serde_json::json!({"truncated":false,"tree":[{"path":slug,"type":"tree","mode":"040000","sha":tree2}]}),
    )?;
    add(
        format!("https://api.github.com/repos/{source}/git/trees/{tree2}?recursive=1"),
        serde_json::json!({"truncated":false,"tree":p.files.iter().map(|f|serde_json::json!({"path":f.path,"size":f.size,"type":"blob","mode":"100644"})).collect::<Vec<_>>()}),
    )?;
    for (name, bytes) in files {
        responses.insert(
            format!(
                "https://raw.githubusercontent.com/{source}/{}/{}/{name}",
                p.commit, p.directory
            ),
            bytes.clone(),
        );
    }
    Ok(FixtureFetch {
        responses,
        requests: std::cell::RefCell::default(),
    })
}
#[test]
fn branch_resolution_complete_inventory_and_partial_failures() -> Result<(), String> {
    let (mut p, files) = generic()?;
    let mut fetch = transport(&p, &files)?;
    assert_eq!(network::catalog(&fetch, &p.origin)?[0].key(), p.key());
    assert_eq!(network::bundle(&fetch, &p, |_| Ok(()))?, files);
    let tree = format!(
        "https://api.github.com/repos/{}/git/trees/{}?recursive=1",
        p.repository.as_str(),
        "d".repeat(40)
    );
    let original = fetch.responses[&tree].clone();
    for change in ["symlink", "truncated", "extra"] {
        let mut v = metadata::json(&original)?;
        match change {
            "symlink" => v["tree"][0]["mode"] = "120000".into(),
            "truncated" => v["truncated"] = true.into(),
            _ => {
                v["tree"].as_array_mut().ok_or("tree")?.push(
                    serde_json::json!({"path":"hidden.py","size":0,"type":"blob","mode":"100644"}),
                );
            }
        }
        fetch.responses.insert(
            tree.clone(),
            serde_json::to_vec(&v).map_err(|e| e.to_string())?,
        );
        assert!(network::bundle(&fetch, &p, |_| Ok(())).is_err());
    }
    let (_scratch, loc) = locations()?;
    let mut sources = Sources::default();
    sources.catalogs.push(p.origin.clone());
    // Disabled entries retain their latest version without any file requests.
    p.installable = false;
    let mut fetch = transport(&p, &files)?;
    fetch.responses.retain(|k, _| !k.contains("git/trees"));
    let rows = check_all(&loc, &sources, &fetch, |_| ());
    assert_eq!(rows.len(), 2);
    assert!(rows[0].status.contains("Fixture missing"));
    assert_eq!(rows[1].package.version, p.version);
    assert!(rows[1].status.starts_with("Disabled"));
    assert!(rows.iter().all(|r| !r.ready));
    Ok(())
}
#[test]
fn install_update_repair_origin_and_local_edit_protections() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    // A headless fixture avoids a toolkit prerequisite; real runtime checks still run.
    files.insert("main.py".into(), b"print('hello')\n".to_vec());
    let p = inventory(p, &files);
    let checked = install::prepare(&loc, p.clone(), files.clone())?;
    assert_eq!(install::check(&loc, p.clone())?.installed, "not installed");
    assert!(checked.prepared.is_some());
    install::install(&loc, &checked)?;
    let root = loc.root(&p);
    assert!(root.join(".vitrallis-receipt.json").is_file());
    assert!(
        loc.data
            .join("applications/io.vitrallis.hello.desktop")
            .is_file()
    );
    let current = install::prepare(&loc, p.clone(), files.clone())?;
    assert!(current.prepared.is_none());
    std::fs::remove_file(root.join("icon.png")).map_err(|e| e.to_string())?;
    let repair = install::prepare(&loc, p.clone(), files.clone())?;
    assert!(repair.prepared.is_some());
    install::install(&loc, &repair)?;
    storage::atomic(
        &root.join("save.json"),
        &FileData {
            bytes: b"save".to_vec(),
            mode: 0o600,
        },
    )?;
    let mut next = p;
    next.version = metadata::version("0.2.0")?;
    files.insert(
        "app.toml".into(),
        String::from_utf8(files["app.toml"].clone())
            .map_err(|e| e.to_string())?
            .replace("0.1.0", "0.2.0")
            .into_bytes(),
    );
    files.insert("main.py".into(), b"print('updated')\n".to_vec());
    next = inventory(next, &files);
    let update = install::prepare(&loc, next.clone(), files.clone())?;
    install::install(&loc, &update)?;
    assert_eq!(
        std::fs::read(root.join("save.json")).map_err(|e| e.to_string())?,
        b"save"
    );
    let (old, mut old_files) = generic()?;
    old_files.insert("main.py".into(), b"print('hello')\n".to_vec());
    assert!(install::prepare(&loc, inventory(old, &old_files), old_files).is_err());
    let mut foreign = next.clone();
    foreign.origin = sources::Repository::parse("another/publisher")?;
    assert!(install::prepare(&loc, foreign, files.clone()).is_err());
    storage::atomic(
        &root.join("main.py"),
        &FileData {
            bytes: b"local changes".to_vec(),
            mode: 0o644,
        },
    )?;
    assert!(install::prepare(&loc, next, files).is_err());
    Ok(())
}
#[test]
fn launcher_customizations_pending_and_stale_check() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let checked = install::prepare(&loc, p.clone(), files.clone())?;
    install::install(&loc, &checked)?;
    let root = loc.root(&p);
    let launcher = loc.state.join("launchers").join(&p.id);
    let managed_launcher = storage::read(&launcher, 1024)?.ok_or("launcher")?;
    assert!(root.join("main.py").is_file());
    let custom = FileData {
        bytes: b"#!/bin/sh\n# custom\n".to_vec(),
        mode: 0o755,
    };
    storage::atomic(&launcher, &custom)?;
    std::fs::remove_file(root.join("icon.png")).map_err(|e| e.to_string())?;
    assert!(install::prepare(&loc, p.clone(), files.clone()).is_err());
    assert_eq!(storage::read(&launcher, 1024)?, Some(custom));
    storage::atomic(&launcher, &managed_launcher)?;
    storage::atomic(
        &root.join(".installation-pending"),
        &FileData {
            bytes: b"pending".to_vec(),
            mode: 0o600,
        },
    )?;
    let mut repair = install::prepare(&loc, p.clone(), files.clone())?;
    assert!(repair.status.contains("repair"));
    if let Some(prepared) = &mut repair.prepared {
        prepared.created_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(901))
            .ok_or("time")?;
    }
    assert!(install::install(&loc, &repair).is_err());
    let repair = install::prepare(&loc, p, files)?;
    storage::atomic(
        &root.join("main.py"),
        &FileData {
            bytes: b"later edit".to_vec(),
            mode: 0o644,
        },
    )?;
    assert!(install::install(&loc, &repair).is_err());
    Ok(())
}
#[cfg(unix)]
#[test]
fn directory_creation_keeps_new_ancestors_safe_and_existing_modes() -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let (_scratch, loc) = locations()?;
    // Keep the fixture root safe when running this regression with umask 002.
    std::fs::set_permissions(&loc.home, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let existing = loc.home.join("private");
    std::fs::create_dir(&existing).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let nested = existing.join("new/applications");
    storage::directory(&nested)?;
    for path in [&existing, &existing.join("new"), &nested] {
        let mode = std::fs::metadata(path).map_err(|e| e.to_string())?.mode();
        assert_eq!(mode & 0o022, 0);
    }
    assert_eq!(
        std::fs::metadata(&existing)
            .map_err(|e| e.to_string())?
            .mode()
            & 0o777,
        0o700
    );
    std::fs::set_permissions(&nested, std::fs::Permissions::from_mode(0o775))
        .map_err(|e| e.to_string())?;
    let refused = nested.join("refused");
    assert!(storage::directory(&refused).is_err());
    assert!(!refused.exists());
    assert_eq!(
        std::fs::metadata(&nested)
            .map_err(|e| e.to_string())?
            .mode()
            & 0o777,
        0o775
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_hardlink_and_lock_checks_fail_before_writes() -> Result<(), String> {
    use std::os::unix::fs::symlink;
    let (_scratch, loc) = locations()?;
    let original = loc.home.join("original");
    std::fs::write(&original, b"data").map_err(|e| e.to_string())?;
    let link = loc.home.join("link");
    symlink(&original, &link).map_err(|e| e.to_string())?;
    assert!(storage::read(&link, 100).is_err());
    std::fs::remove_file(&link).map_err(|e| e.to_string())?;
    std::fs::hard_link(&original, &link).map_err(|e| e.to_string())?;
    assert!(storage::read(&link, 100).is_err());
    let lock = storage::Lock::take(&loc.state)?;
    assert!(storage::Lock::take(&loc.state).is_err());
    drop(lock);
    assert!(storage::Lock::take(&loc.state).is_ok());
    Ok(())
}

#[test]
fn syntax_preflight_never_executes_app_code_and_reports_dependencies() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    let marker = loc.home.join("must-not-exist");
    files.insert(
        "main.py".into(),
        format!(
            "open({:?}, 'w').write('unsafe')\n",
            marker.to_str().ok_or("path")?
        )
        .into_bytes(),
    );
    let runtime = runtime::detect(&loc.root(&p), &files)?;
    runtime::validate(&runtime, &files)?;
    assert!(!marker.exists());
    files.insert("main.py".into(), b"def broken(:\n".to_vec());
    assert!(runtime::validate(&runtime, &files).is_err());
    files.insert(
        "requirements.txt".into(),
        b"vitrallis-test-missing-distribution-123456==99.99\n".to_vec(),
    );
    assert!(runtime::detect(&loc.root(&p), &files).is_err());
    Ok(())
}
#[cfg(unix)]
#[test]
fn running_app_identity_is_rechecked_and_only_exact_script_is_closed() -> Result<(), String> {
    use running::Processes;
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let (_scratch, loc) = locations()?;
    let script = loc.home.join("running.py");
    std::fs::write(&script, "import time\ntime.sleep(30)\n").map_err(|e| e.to_string())?;
    let mut child = Child(
        std::process::Command::new("/usr/bin/python3")
            .arg(&script)
            .spawn()
            .map_err(|e| e.to_string())?,
    );
    std::thread::sleep(std::time::Duration::from_millis(100));
    let native = running::Native;
    let identities = native.list(&script)?;
    let identity = identities
        .iter()
        .find(|p| p.pid == child.0.id())
        .ok_or("Test process not identified")?;
    let mut stale = identity.clone();
    stale.start.push_str("changed");
    native.terminate(&script, &stale)?;
    assert!(child.0.try_wait().map_err(|e| e.to_string())?.is_none());
    assert!(native.list(&loc.home.join("other.py"))?.is_empty());
    running::close(
        &native,
        &script,
        &identities,
        std::time::Duration::from_secs(8),
    )?;
    let status = child.0.wait().map_err(|e| e.to_string())?;
    assert!(!status.success());
    Ok(())
}

fn test_sources(p: &Package) -> Sources {
    Sources {
        catalogs: vec![p.origin.clone()],
        ..Sources::default()
    }
}
fn selected_install(
    loc: &Locations,
    sources: &Sources,
    row: &Checked,
    fetch: &impl network::Fetch,
) -> Result<Vec<String>, String> {
    let (_send, commands) = mpsc::channel();
    let (updates, receive) = mpsc::channel();
    install_one(
        loc,
        sources,
        row,
        &commands,
        &updates,
        &AtomicBool::new(false),
        fetch,
    )?;
    Ok(receive
        .try_iter()
        .filter_map(|u| match u {
            Update::Progress(s) => Some(s),
            _ => None,
        })
        .collect())
}
fn payload_url(p: &Package, name: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{name}",
        p.repository.as_str(),
        p.commit,
        p.directory
    )
}
fn replace_catalog(
    fetch: &mut FixtureFetch,
    p: &Package,
    packages: &[Package],
) -> Result<(), String> {
    let apps = packages
        .iter()
        .map(|p| catalog_value(p)["apps"][0].clone())
        .collect::<Vec<_>>();
    fetch.responses.insert(
        format!(
            "https://raw.githubusercontent.com/{}/{}/apps.json",
            p.origin.as_str(),
            "b".repeat(40)
        ),
        serde_json::to_vec(&serde_json::json!({"schema_version":1,"apps":apps}))
            .map_err(|e| e.to_string())?,
    );
    Ok(())
}
#[test]
fn metadata_only_check_and_selected_install_update_use_one_download_per_file() -> Result<(), String>
{
    let (_scratch, loc) = locations()?;
    let (mut p, mut files) = generic()?;
    files.insert("README.md".into(), vec![b'x'; 40_000]);
    p = inventory(p, &files);
    let sources = test_sources(&p);
    let mut fetch = transport(&p, &files)?;
    let mut other = p.clone();
    other.id = "org.example.other".into();
    other.directory = "apps/other".into();
    replace_catalog(&mut fetch, &p, &[p.clone(), other.clone()])?;
    let rows = check_all(&loc, &sources, &fetch, |_| ());
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.ready));
    assert_eq!(fetch.requests.borrow().len(), 3);
    assert!(!loc.root(&p).exists());
    let total = files.values().map(Vec::len).sum::<usize>();
    assert_eq!(Row::from(&rows[0]).download_size, total);
    let progress = selected_install(&loc, &sources, &rows[0], &fetch)?;
    assert!(
        progress
            .iter()
            .any(|s| s.starts_with("Downloading: 16384 /"))
    );
    assert!(
        progress
            .iter()
            .any(|s| s.starts_with(&format!("Downloading: {total} / {total} bytes (100%)")))
    );
    let verifying = progress
        .iter()
        .position(|s| s.starts_with("Verifying"))
        .ok_or("verify stage")?;
    let installing = progress
        .iter()
        .position(|s| s.starts_with("Installing"))
        .ok_or("install stage")?;
    assert!(verifying < installing);
    for name in files.keys() {
        assert_eq!(
            fetch
                .requests
                .borrow()
                .iter()
                .filter(|u| **u == payload_url(&p, name))
                .count(),
            1
        );
    }
    assert!(
        !fetch
            .requests
            .borrow()
            .iter()
            .any(|u| u.contains("/apps/other/"))
    );
    assert!(!loc.root(&other).exists());
    assert!(!install::check(&loc, p.clone())?.ready);

    p.version = metadata::version("0.2.0")?;
    p.commit = "e".repeat(40);
    files.insert(
        "app.toml".into(),
        String::from_utf8(files["app.toml"].clone())
            .map_err(|e| e.to_string())?
            .replace("0.1.0", "0.2.0")
            .into_bytes(),
    );
    files.insert("main.py".into(), b"print('new version')\n".to_vec());
    p = inventory(p, &files);
    let mut fetch = transport(&p, &files)?;
    replace_catalog(&mut fetch, &p, &[p.clone(), other])?;
    let rows = check_all(&loc, &sources, &fetch, |_| ());
    assert_eq!(rows[0].installed, "0.1.0");
    assert!(rows[0].ready);
    assert_eq!(fetch.requests.borrow().len(), 3);
    selected_install(&loc, &sources, &rows[0], &fetch)?;
    for name in files.keys() {
        assert_eq!(
            fetch
                .requests
                .borrow()
                .iter()
                .filter(|u| **u == payload_url(&p, name))
                .count(),
            1
        );
    }
    assert!(
        !fetch
            .requests
            .borrow()
            .iter()
            .any(|u| u.contains("/apps/other/"))
    );
    assert_eq!(install::label(&loc, &p)?, "0.2.0");
    Ok(())
}
#[test]
fn large_catalog_is_available_without_downloading_payloads() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let mut fetch = transport(&p, &files)?;
    let packages = (0..6)
        .map(|i| {
            let mut next = p.clone();
            next.id = format!("org.example.app{i}");
            for file in &mut next.files {
                file.size = metadata::FILE_LIMIT;
            }
            next
        })
        .collect::<Vec<_>>();
    assert!(
        packages
            .iter()
            .flat_map(|p| &p.files)
            .map(|f| f.size)
            .sum::<usize>()
            > 64 * 1024 * 1024
    );
    replace_catalog(&mut fetch, &p, &packages)?;
    fetch
        .responses
        .retain(|u, _| !u.contains("/git/trees/") && !u.contains("/apps/hello/"));
    let rows = check_all(&loc, &test_sources(&p), &fetch, |_| ());
    assert_eq!(rows.len(), 6);
    assert!(
        rows.iter()
            .all(|r| r.ready && r.status == "ready to install")
    );
    assert_eq!(fetch.requests.borrow().len(), 3);
    assert!(!loc.data.exists());
    Ok(())
}
#[test]
fn failed_or_corrupt_downloads_never_install_or_replace_existing_app() -> Result<(), String> {
    for existing in [false, true] {
        let (_scratch, loc) = locations()?;
        let (mut p, mut files) = generic()?;
        let sources = test_sources(&p);
        if existing {
            let row = install::check(&loc, p.clone())?;
            selected_install(&loc, &sources, &row, &transport(&p, &files)?)?;
            p.version = metadata::version("0.2.0")?;
            files.insert(
                "app.toml".into(),
                String::from_utf8(files["app.toml"].clone())
                    .map_err(|e| e.to_string())?
                    .replace("0.1.0", "0.2.0")
                    .into_bytes(),
            );
            p = inventory(p, &files);
        }
        let before = storage::read(
            &loc.root(&p).join(".vitrallis-receipt.json"),
            metadata::CATALOG_LIMIT,
        )?;
        for failure in ["network", "hash", "short", "long"] {
            let mut fetch = transport(&p, &files)?;
            let url = payload_url(&p, "main.py");
            match failure {
                "network" => {
                    fetch.responses.remove(&url);
                }
                "hash" => {
                    fetch.responses.get_mut(&url).ok_or("payload")?[0] ^= 1;
                }
                "short" => {
                    fetch.responses.get_mut(&url).ok_or("payload")?.pop();
                }
                _ => fetch.responses.get_mut(&url).ok_or("payload")?.push(0),
            }
            let rows = check_all(&loc, &sources, &fetch, |_| ());
            assert!(rows[0].ready);
            assert!(selected_install(&loc, &sources, &rows[0], &fetch).is_err());
            assert_eq!(
                storage::read(
                    &loc.root(&p).join(".vitrallis-receipt.json"),
                    metadata::CATALOG_LIMIT
                )?,
                before
            );
            assert!(!loc.root(&p).join(".installation-pending").exists());
            if existing {
                assert_eq!(
                    std::fs::read(loc.root(&p).join("main.py")).map_err(|e| e.to_string())?,
                    files["main.py"]
                );
            } else {
                assert!(!loc.root(&p).exists());
            }
        }
    }
    Ok(())
}
#[test]
fn streamed_byte_progress_and_cancellation_leave_no_installation() -> Result<(), String> {
    let mut seen = Vec::new();
    let bytes = vec![b'x'; 40_000];
    assert_eq!(
        network::read_download(std::io::Cursor::new(&bytes), bytes.len(), &mut |n| {
            seen.push(n);
            Ok(())
        })?,
        bytes
    );
    seen.dedup();
    assert_eq!(seen, [0, 16384, 32768, 40000]);
    let mut cursor = std::io::Cursor::new(&bytes);
    assert!(
        network::read_download(&mut cursor, bytes.len(), &mut |n| {
            if n >= 16384 {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    assert_eq!(cursor.position(), 16384);
    assert!(network::read_download(std::io::Cursor::new(&bytes), 10, &mut |_| Ok(())).is_err());
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let fetch = transport(&p, &files)?;
    let row = install::check(&loc, p.clone())?;
    assert!(
        acquire(&loc, &row, &fetch, |s| {
            if s.starts_with("Downloading") {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    assert!(!loc.root(&p).exists());
    let fetch = transport(&p, &files)?;
    assert!(
        acquire(&loc, &row, &fetch, |s| {
            if s.starts_with("Verifying") {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    assert!(!loc.root(&p).exists());
    Ok(())
}
#[test]
fn deferred_install_rechecks_source_trust_and_manifest_agreement() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (mut p, files) = generic()?;
    let mut sources = test_sources(&p);
    let row = install::check(&loc, p.clone())?;
    let fetch = transport(&p, &files)?;
    sources.catalogs.clear();
    assert!(selected_install(&loc, &sources, &row, &fetch).is_err());
    assert!(fetch.requests.borrow().is_empty());
    p.repository = sources::Repository::parse("unapproved/source")?;
    let fetch = transport(&p, &files)?;
    let sources = test_sources(&p);
    let rows = check_all(&loc, &sources, &fetch, |_| ());
    assert!(!rows[0].ready);
    assert!(rows[0].status.contains("Approval required"));
    assert_eq!(fetch.requests.borrow().len(), 3);
    let formerly_approved = install::check(&loc, p)?;
    fetch.requests.borrow_mut().clear();
    assert!(selected_install(&loc, &sources, &formerly_approved, &fetch).is_err());
    assert!(fetch.requests.borrow().is_empty());
    let (mut p, files) = generic()?;
    p.name = "Catalog disagrees with app.toml".into();
    let fetch = transport(&p, &files)?;
    let row = install::check(&loc, p.clone())?;
    assert!(selected_install(&loc, &test_sources(&p), &row, &fetch).is_err());
    assert!(!loc.root(&p).exists());
    Ok(())
}

#[test]
fn cancellation_token_interrupts_selected_transfer_before_commit() -> Result<(), String> {
    struct CancelFetch<'a> {
        inner: &'a FixtureFetch,
        cancelled: &'a AtomicBool,
    }
    impl network::Fetch for CancelFetch<'_> {
        fn fetch(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
            self.inner.fetch(url, limit)
        }
        fn fetch_progress(
            &self,
            url: &str,
            limit: usize,
            progress: &mut dyn FnMut(usize) -> Result<(), String>,
        ) -> Result<Vec<u8>, String> {
            self.inner.fetch_progress(url, limit, &mut |n| {
                if n > 0 {
                    self.cancelled.store(true, Ordering::Relaxed);
                }
                progress(n)
            })
        }
    }
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let inner = transport(&p, &files)?;
    let cancelled = AtomicBool::new(false);
    let fetch = CancelFetch {
        inner: &inner,
        cancelled: &cancelled,
    };
    let row = install::check(&loc, p.clone())?;
    let (_send, commands) = mpsc::channel();
    let (updates, _receive) = mpsc::channel();
    let error = install_one(
        &loc,
        &test_sources(&p),
        &row,
        &commands,
        &updates,
        &cancelled,
        &fetch,
    )
    .err()
    .ok_or("expected cancellation")?;
    assert!(error.contains("Cancelled"));
    assert!(!loc.root(&p).exists());
    assert_eq!(
        inner
            .requests
            .borrow()
            .iter()
            .filter(|u| u.contains("raw.githubusercontent.com"))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn stream_io_failure_discards_partial_download() {
    struct BrokenStream(bool);
    impl std::io::Read for BrokenStream {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.0 {
                return Err(std::io::Error::other("connection lost"));
            }
            self.0 = true;
            out[..3].copy_from_slice(b"abc");
            Ok(3)
        }
    }
    let mut progress = Vec::new();
    let result = network::read_download(BrokenStream(false), 10, &mut |n| {
        progress.push(n);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(progress.last(), Some(&3));
    assert!(!progress.contains(&10));
}

#[test]
fn device_inventory_excludes_only_app_local_tests() -> Result<(), String> {
    let (p, mut files) = generic()?;
    files.insert(
        "tests/test_layout.py".into(),
        b"# development test\n".to_vec(),
    );
    files.insert(
        "assets/tests/example.txt".into(),
        b"runtime asset\n".to_vec(),
    );
    let full = inventory(p, &files);
    let fetch = transport(&full, &files)?;
    assert!(network::bundle(&fetch, &full, |_| Ok(())).is_err());
    assert!(metadata::validate_bundle(&full, &files).is_err());
    let mut device = full.clone();
    device.files.retain(|f| !f.path.starts_with("tests/"));
    fetch.requests.borrow_mut().clear();
    let payload = network::bundle(&fetch, &device, |_| Ok(()))?;
    assert!(payload.keys().all(|p| !p.starts_with("tests/")));
    assert!(payload.contains_key("assets/tests/example.txt"));
    assert!(
        !fetch
            .requests
            .borrow()
            .iter()
            .any(|u| u.contains("/hello/tests/"))
    );
    metadata::validate_bundle(&device, &payload)?;
    let (_scratch, loc) = locations()?;
    let row = install::check(&loc, device.clone())?;
    selected_install(&loc, &test_sources(&device), &row, &fetch)?;
    assert!(!loc.root(&device).join("tests").exists());
    for omitted in ["main.py", "README.md", "assets/tests/example.txt"] {
        let mut bad = device.clone();
        bad.files.retain(|f| f.path != omitted);
        assert!(network::bundle(&fetch, &bad, |_| Ok(())).is_err());
    }
    let mut partial = full;
    partial
        .files
        .retain(|f| f.path != "assets/tests/example.txt");
    assert!(network::bundle(&fetch, &partial, |_| Ok(())).is_err());
    Ok(())
}

#[test]
fn device_inventory_rejects_unsafe_git_entries_even_in_excluded_tests() -> Result<(), String> {
    let (p, mut files) = generic()?;
    files.insert("tests/test_main.py".into(), b"# test\n".to_vec());
    let full = inventory(p, &files);
    let mut fetch = transport(&full, &files)?;
    let mut device = full;
    device.files.retain(|f| !f.path.starts_with("tests/"));
    let (_scratch, loc) = locations()?;
    let row = install::check(&loc, device.clone())?;
    selected_install(&loc, &test_sources(&device), &row, &fetch)?;
    assert!(loc.root(&device).join("main.py").is_file());
    assert!(!loc.root(&device).join("tests").exists());
    assert!(
        !fetch
            .requests
            .borrow()
            .iter()
            .any(|u| u.contains("/hello/tests/"))
    );
    let tree = format!(
        "https://api.github.com/repos/{}/git/trees/{}?recursive=1",
        device.repository.as_str(),
        "d".repeat(40)
    );
    let original = fetch.responses[&tree].clone();
    for (mode, kind, name) in [
        ("120000", "blob", "tests/link"),
        ("160000", "commit", "tests/submodule"),
        ("100644", "blob", "tests/../outside"),
        ("100644", "blob", "Tests/hidden.py"),
    ] {
        let mut v = metadata::json(&original)?;
        v["tree"]
            .as_array_mut()
            .ok_or("tree")?
            .push(serde_json::json!({
                "path":name,"size":0,"type":kind,"mode":mode
            }));
        fetch.responses.insert(
            tree.clone(),
            serde_json::to_vec(&v).map_err(|e| e.to_string())?,
        );
        assert!(network::bundle(&fetch, &device, |_| Ok(())).is_err());
    }
    Ok(())
}

#[test]
fn uninstall_removes_receipted_app_and_shortcuts_preserving_data_and_other_apps()
-> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    install::install(&loc, &install::prepare(&loc, p.clone(), files.clone())?)?;
    let root = loc.root(&p);
    let save = root.join("save.json");
    storage::atomic(
        &save,
        &FileData {
            bytes: b"user data".to_vec(),
            mode: 0o600,
        },
    )?;
    let other = loc.data.join("vitrallis/apps/org.example.other/main.py");
    storage::atomic(
        &other,
        &FileData {
            bytes: b"other app".to_vec(),
            mode: 0o644,
        },
    )?;
    let custom = loc.home.join("Desktop").join(format!("{}.desktop", p.id));
    storage::atomic(
        &custom,
        &FileData {
            bytes: b"[Desktop Entry]\nExec=/custom/program\n".to_vec(),
            mode: 0o755,
        },
    )?;
    uninstall::uninstall(&loc, &p)?;
    for name in files.keys() {
        assert!(!root.join(name).exists(), "{name}");
    }
    assert!(!root.join(".vitrallis-receipt.json").exists());
    assert!(!root.join(".installation-pending").exists());
    assert!(!loc.state.join("launchers").join(&p.id).exists());
    assert!(
        !loc.data
            .join("applications")
            .join(format!("{}.desktop", p.id))
            .exists()
    );
    assert!(custom.is_file());
    assert_eq!(
        std::fs::read(save).map_err(|e| e.to_string())?,
        b"user data"
    );
    assert_eq!(
        std::fs::read(other).map_err(|e| e.to_string())?,
        b"other app"
    );
    let checked = install::check(&loc, p.clone())?;
    assert_eq!(checked.installed, "not installed");
    assert!(checked.ready);
    assert!(!Row::from(&checked).can_uninstall());
    assert!(!Row::from(&checked).update_available());
    install::install(&loc, &install::prepare(&loc, p, files)?)?;
    assert!(root.join("main.py").is_file());
    Ok(())
}

#[cfg(unix)]
#[test]
fn uninstall_rejects_source_switches_unsafe_files_and_modified_receipts_before_removal()
-> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    install::install(&loc, &install::prepare(&loc, p.clone(), files)?)?;
    let root = loc.root(&p);
    let mut foreign = p.clone();
    foreign.origin = sources::Repository::parse("another/catalog")?;
    assert!(uninstall::uninstall(&loc, &foreign).is_err());
    let receipt = root.join(".vitrallis-receipt.json");
    let before = storage::read(&receipt, metadata::CATALOG_LIMIT)?.ok_or("receipt")?;
    let mut modified = metadata::json(&before.bytes)?;
    modified["files"]["../outside"] = "0".repeat(64).into();
    storage::atomic(
        &receipt,
        &FileData {
            bytes: serde_json::to_vec(&modified).map_err(|e| e.to_string())?,
            mode: before.mode,
        },
    )?;
    assert!(uninstall::uninstall(&loc, &p).is_err());
    storage::atomic(&receipt, &before)?;
    let main = root.join("main.py");
    let saved = storage::read(&main, metadata::FILE_LIMIT)?.ok_or("main")?;
    std::fs::remove_file(&main).map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(root.join("README.md"), &main).map_err(|e| e.to_string())?;
    assert!(uninstall::uninstall(&loc, &p).is_err());
    assert!(root.join("README.md").exists());
    assert!(!root.join(".installation-pending").exists());
    std::fs::remove_file(&main).map_err(|e| e.to_string())?;
    storage::atomic(&main, &saved)?;
    assert!(root.join("app.toml").is_file());
    Ok(())
}

#[cfg(unix)]
#[test]
fn uninstall_refuses_a_running_app_without_removing_files() -> Result<(), String> {
    use running::Processes;
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    files.insert("main.py".into(), b"import time\ntime.sleep(30)\n".to_vec());
    let p = inventory(p, &files);
    install::install(&loc, &install::prepare(&loc, p.clone(), files)?)?;
    let entry = loc.root(&p).join("main.py");
    let mut child = Child(
        std::process::Command::new("/usr/bin/python3")
            .arg(&entry)
            .spawn()
            .map_err(|e| e.to_string())?,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while running::Native.list(&entry)?.is_empty() {
        if std::time::Instant::now() > deadline {
            return Err("running app not detected".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let result = uninstall::uninstall(&loc, &p);
    assert!(result.is_err_and(|e| e.contains("Close the app")));
    assert!(child.0.try_wait().map_err(|e| e.to_string())?.is_none());
    assert!(entry.is_file());
    assert!(loc.root(&p).join(".vitrallis-receipt.json").is_file());
    assert!(!loc.root(&p).join(".installation-pending").exists());
    Ok(())
}

#[test]
fn canonical_catalog_paths_manifests_and_inventory_are_required() -> Result<(), String> {
    let (p, files) = generic()?;
    for path in [
        "Apps/Hello",
        "apps/Hello",
        "apps/hello_world",
        "apps/hello/nested",
        "other/hello",
    ] {
        let mut value = catalog_value(&p);
        value["apps"][0]["source"]["path"] = path.into();
        assert!(
            metadata::catalog(
                &p.origin,
                &serde_json::to_vec(&value).map_err(|e| e.to_string())?
            )
            .is_err()
        );
    }
    for omitted in [
        "app.toml",
        "main.py",
        "icon.png",
        "requirements.txt",
        "README.md",
        "assets/greeting.txt",
    ] {
        let mut incomplete = files.clone();
        incomplete.remove(omitted);
        assert!(
            metadata::validate_bundle(&inventory(p.clone(), &incomplete), &incomplete).is_err(),
            "{omitted}"
        );
    }
    let mut with_tests = files;
    with_tests.insert("tests/test_main.py".into(), b"# test".to_vec());
    let value = catalog_value(&inventory(p.clone(), &with_tests));
    assert!(
        metadata::catalog(
            &p.origin,
            &serde_json::to_vec(&value).map_err(|e| e.to_string())?
        )
        .is_err()
    );
    let mut value = catalog_value(&p);
    value["apps"][0]["files"]
        .as_array_mut()
        .ok_or("files")?
        .reverse();
    assert!(
        metadata::catalog(
            &p.origin,
            &serde_json::to_vec(&value).map_err(|e| e.to_string())?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn unreceipted_source_never_supplies_version_or_ownership() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, files) = generic()?;
    let root = loc.root(&p);
    storage::atomic(
        &root.join("main.py"),
        &FileData {
            bytes: b"VERSION = '0.1.0'\n".to_vec(),
            mode: 0o644,
        },
    )?;
    assert_eq!(install::label(&loc, &p)?, "local / unknown");
    assert!(install::check(&loc, p.clone()).is_err());
    assert!(install::prepare(&loc, p.clone(), files).is_err());
    assert!(uninstall::uninstall(&loc, &p).is_err());
    assert!(root.join("main.py").exists());
    Ok(())
}

#[test]
#[ignore = "explicit online contract check against GitHub"]
fn published_catalog_packages_match_the_current_contract() -> Result<(), String> {
    let origin = sources::Repository::parse(sources::DEFAULT)?;
    let packages = network::catalog(&network::Curl, &origin)?;
    assert!(!packages.is_empty());
    for package in packages {
        let files = network::bundle(&network::Curl, &package, |_| Ok(()))?;
        metadata::validate_bundle(&package, &files)?;
        eprintln!(
            "Verified {} {} at {}/{} ({} files; installable={})",
            package.id,
            package.version,
            package.commit,
            package.directory,
            files.len(),
            package.installable
        );
    }
    Ok(())
}

#[test]
#[ignore = "explicit device QA: downloads an app-local Python dependency into VITRALLIS_QA_HOME"]
fn physical_python_first_installs() -> Result<(), String> {
    let home = std::env::var_os("VITRALLIS_QA_HOME")
        .map(std::path::PathBuf::from)
        .ok_or("Set a new isolated VITRALLIS_QA_HOME")?;
    storage::safe(&home)?;
    let loc = storage::Locations {
        data: home.join(".local/share"),
        state: home.join(".local/share/vitrallis/app-center"),
        sources: home.join(".config/vitrallis/app-center.json"),
        home,
    };
    for suffix in ["one", "two", "three"] {
        let started = std::time::Instant::now();
        let (mut p, mut files) = generic()?;
        p.id = format!("io.vitrallis.qapython{suffix}");
        p.name = format!("Python QA {suffix}");
        let manifest = String::from_utf8(files["app.toml"].clone()).map_err(|e| e.to_string())?;
        let original = metadata::manifest(manifest.as_bytes())?;
        files.insert(
            "app.toml".into(),
            manifest
                .replace(metadata::text(&original["id"], 128)?, &p.id)
                .replace(metadata::text(&original["name"], 1000)?, &p.name)
                .into_bytes(),
        );
        files.insert("requirements.txt".into(), b"pyfiglet==1.0.2\n".to_vec());
        files.insert("main.py".into(), b"import pyfiglet\nimport tkinter as tk\nroot = tk.Tk()\nroot.title('Python QA')\nroot.geometry('480x272')\ntk.Label(root, text=pyfiglet.figlet_format('QA'), font=('monospace', 8)).pack()\nroot.bind('<Escape>', lambda event: root.destroy())\nroot.mainloop()\n".to_vec());
        p = inventory(p, &files);
        if loc.root(&p).exists() {
            return Err("QA app root already exists; use a new isolated home".into());
        }
        eprintln!("QA cold dependency install: {}", p.id);
        install::install(&loc, &install::prepare(&loc, p.clone(), files.clone())?)?;
        assert!(
            loc.root(&p).join("runtime").is_dir(),
            "must exercise actual isolated pip installation"
        );
        assert!(!install::check(&loc, p.clone())?.ready);
        for _ in 0..3 {
            assert!(
                install::prepare(&loc, p.clone(), files.clone())?
                    .prepared
                    .is_none()
            );
        }
        let runtime = super::runtime::detect(&loc.root(&p), &files)?;
        let result = std::process::Command::new(&runtime.program)
            .args([
                "-I",
                "-c",
                "import pyfiglet; assert pyfiglet.figlet_format('QA')",
            ])
            .status()
            .map_err(|e| e.to_string())?;
        assert!(result.success());
        eprintln!(
            "QA passed {}: {:.2}s, cold install + 3 repeat preparations, dependency imported",
            p.id,
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}
