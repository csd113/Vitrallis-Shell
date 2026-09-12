use super::*;
use metadata::{Files, Package};
use std::{collections::BTreeMap, path::Path};
use storage::{FileData, Locations};
fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/app-center")
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
fn generic() -> Result<(Package, Files), String> {
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
        ("tests/test_main.py".into(), b"# fixture\n".to_vec()),
    ]);
    let v = metadata::manifest(&files["app.toml"])?;
    let origin = sources::Repository::parse("example/catalog")?;
    let p = Package {
        origin: origin.clone(),
        repository: origin,
        id: v["id"].as_str().ok_or("id")?.into(),
        name: v["name"].as_str().ok_or("name")?.into(),
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
fn inventory(mut p: Package, files: &Files) -> Package {
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
fn locations() -> Result<(crate::test_support::Scratch, Locations), String> {
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
    assert!(s.catalogs[0].is_default());
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
fn actual_catalog_and_manifest_contracts_reject_malicious_metadata() -> Result<(), String> {
    let bytes = std::fs::read(fixture("catalog.json")).map_err(|e| e.to_string())?;
    let origin = sources::Repository::parse(sources::DEFAULT)?;
    let p = metadata::catalog(&origin, &bytes)?;
    assert!(p[0].legacy());
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
}
impl network::Fetch for FixtureFetch {
    fn fetch(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        let b = self
            .responses
            .get(url)
            .ok_or_else(|| format!("Fixture missing: {url}"))?;
        if b.len() > limit {
            return Err("bounded fixture".into());
        }
        Ok(b.clone())
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
    Ok(FixtureFetch { responses })
}
#[test]
fn branch_resolution_complete_inventory_and_partial_failures() -> Result<(), String> {
    let (mut p, files) = generic()?;
    let mut fetch = transport(&p, &files)?;
    assert_eq!(network::catalog(&fetch, &p.origin)?[0].key(), p.key());
    assert_eq!(network::bundle(&fetch, &p, |_| ())?, files);
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
        assert!(network::bundle(&fetch, &p, |_| ()).is_err());
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
    assert!(rows.iter().all(|r| r.prepared.is_none()));
    Ok(())
}
#[test]
fn native_install_update_repair_origin_and_local_edit_protections() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let (p, mut files) = generic()?;
    // A headless fixture avoids a toolkit prerequisite; real runtime checks still run.
    files.insert("main.py".into(), b"print('hello')\n".to_vec());
    let p = inventory(p, &files);
    let checked = install::prepare(&loc, p.clone(), files.clone())?;
    assert_eq!(checked.installed, "not installed");
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
fn legacy_path_support_customizations_pending_and_stale_check() -> Result<(), String> {
    let (_scratch, loc) = locations()?;
    let bytes = std::fs::read(fixture("catalog.json")).map_err(|e| e.to_string())?;
    let p = metadata::catalog(&sources::Repository::parse(sources::DEFAULT)?, &bytes)?.remove(0);
    let mut files = Files::from([("icon.png".into(), fixture_icon()?)]);
    // Fixed adapter layout and literal version, without a GUI runtime prerequisite.
    files.insert(
        "bitcoin.py".into(),
        format!("VERSION = '{}'\n", p.version).into_bytes(),
    );
    let p = inventory(p, &files);
    let checked = install::prepare(&loc, p.clone(), files.clone())?;
    install::install(&loc, &checked)?;
    let root = loc.home.join(".local/share/pocket-bitcoin");
    assert!(root.join("bitcoin.py").is_file());
    let custom = FileData {
        bytes: b"#!/bin/sh\n# custom\n".to_vec(),
        mode: 0o755,
    };
    storage::atomic(&root.join("launch"), &custom)?;
    std::fs::remove_file(root.join("bitcoin.png")).map_err(|e| e.to_string())?;
    let repair = install::prepare(&loc, p.clone(), files.clone())?;
    install::install(&loc, &repair)?;
    assert_eq!(storage::read(&root.join("launch"), 1024)?, Some(custom));
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
        prepared.checked_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(901))
            .ok_or("time")?;
    }
    assert!(install::install(&loc, &repair).is_err());
    let repair = install::prepare(&loc, p, files)?;
    storage::atomic(
        &root.join("bitcoin.py"),
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
    let runtime = runtime::detect(&loc.home, &loc.root(&p), &files)?;
    runtime::validate(&runtime, &files)?;
    assert!(!marker.exists());
    assert_eq!(
        runtime::legacy_version(&runtime, b"\"\"\"\nVERSION = '1.0.0'\n\"\"\"\n")?,
        None
    );
    assert_eq!(
        runtime::legacy_version(&runtime, b"VERSION: str = '1.10.0'\n")?,
        Some(metadata::version("1.10.0")?)
    );
    assert_eq!(
        runtime::legacy_version(&runtime, b"VERSION = '1.0.0'\nVERSION = '2.0.0'\n")?,
        None
    );
    assert!(metadata::version("999999999999999999999999.0.0")? > metadata::version("1.0.0")?);
    files.insert("main.py".into(), b"def broken(:\n".to_vec());
    assert!(runtime::validate(&runtime, &files).is_err());
    files.insert(
        "requirements.txt".into(),
        b"vitrallis-test-missing-distribution-123456==99.99\n".to_vec(),
    );
    assert!(runtime::detect(&loc.home, &loc.root(&p), &files).is_err());
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

#[cfg(unix)]
#[test]
fn unsafe_pockethome_menu_does_not_block_legacy_installation() -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let (_scratch, loc) = locations()?;
    let menu = loc.home.join(".pocket-home");
    let config = menu.join("config.json");
    storage::directory(&menu)?;
    let original = br#"{"pages":[{"name":"Apps","items":[]}]}"#;
    std::fs::write(&config, original).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&menu, std::fs::Permissions::from_mode(0o775))
        .map_err(|e| e.to_string())?;
    assert!(storage::read(&config, 1024).is_err());
    let bytes = std::fs::read(fixture("catalog.json")).map_err(|e| e.to_string())?;
    let package =
        metadata::catalog(&sources::Repository::parse(sources::DEFAULT)?, &bytes)?.remove(0);
    let files = Files::from([
        ("icon.png".into(), fixture_icon()?),
        (
            "bitcoin.py".into(),
            format!("VERSION = '{}'\n", package.version).into_bytes(),
        ),
    ]);
    let package = inventory(package, &files);
    let checked = install::prepare(&loc, package.clone(), files.clone())?;
    assert!(checked.status.contains("PocketHome menu unchanged"));
    assert!(
        checked
            .prepared
            .as_ref()
            .is_some_and(|p| p.writes.iter().all(|w| w.path != config))
    );
    install::install(&loc, &checked)?;
    assert!(loc.root(&package).join("bitcoin.py").is_file());
    assert_eq!(std::fs::read(&config).map_err(|e| e.to_string())?, original);
    assert_eq!(
        std::fs::metadata(&menu).map_err(|e| e.to_string())?.mode() & 0o777,
        0o775
    );
    // A safe PocketHome menu still receives the normal compatibility entry.
    std::fs::set_permissions(&menu, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    let checked = install::prepare(&loc, package.clone(), files.clone())?;
    assert!(!checked.status.contains("PocketHome menu unchanged"));
    install::install(&loc, &checked)?;
    let updated = metadata::json(&std::fs::read(&config).map_err(|e| e.to_string())?)?;
    assert_eq!(updated["pages"][0]["items"][0]["name"], "Bitcoin CAD");
    // Unsafe application storage must still block preparation and writes.
    std::fs::set_permissions(loc.root(&package), std::fs::Permissions::from_mode(0o775))
        .map_err(|e| e.to_string())?;
    assert!(install::prepare(&loc, package, files).is_err());
    Ok(())
}
