use super::*;
use std::{ffi::OsString, os::unix::fs::PermissionsExt};

#[test]
fn vanished_executable_cwd_and_permissions_return_to_a_dismissible_error()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::{
        input::Action,
        launcher::{Launcher, Phase},
        process::{NativeProcess, activate},
    };
    for fault in ["executable", "cwd", "permission"] {
        let (scratch, store, mut draft) = fixture()?;
        let root = scratch.0.canonicalize()?;
        let script = root.join("script");
        let cwd = root.join("cwd");
        fs::create_dir(&cwd)?;
        fs::write(&script, "#!/bin/sh\nexit 0\n")?;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
        draft.command = command::quote(&script)?;
        draft.cwd = cwd.to_string_lossy().into_owned();
        store.save(None, &draft)?;
        let mut catalog = Catalog::default();
        store.integrate(&mut catalog)?;
        let mut launcher = Launcher::new(catalog.apps, 3, 6)?;
        match fault {
            "executable" => fs::remove_file(&script)?,
            "cwd" => fs::remove_dir(&cwd)?,
            _ => fs::set_permissions(&script, fs::Permissions::from_mode(0o644))?,
        }
        let index = launcher.input(Action::Activate).ok_or("launch requested")?;
        activate(&mut launcher, &mut NativeProcess::default(), index);
        assert_eq!(launcher.phase, Phase::Ready);
        assert!(launcher.error.is_some(), "{fault}");
        assert!(launcher.input(Action::Activate).is_none());
        assert!(launcher.error.is_none());
        assert_eq!(launcher.phase, Phase::Ready);
    }
    Ok(())
}

fn fixture() -> Result<(crate::test_support::Scratch, Store, Draft), String> {
    let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
    let home = scratch.0.canonicalize().map_err(|e| e.to_string())?;
    let store = Store {
        root: home.join("shortcuts"),
    };
    let draft = Draft {
        name: "My script".into(),
        command: "/bin/echo 'two words'".into(),
        cwd: home.to_string_lossy().into(),
        ..Draft::default()
    };
    Ok((scratch, store, draft))
}

#[test]
fn crud_restart_and_icon_copy_are_atomic_and_never_delete_target()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, store, mut draft) = fixture()?;
    let target = scratch.0.join("my executable");
    fs::write(&target, "#!/bin/sh\nprintf target\n")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    draft.command = command::quote(&target)?;
    let original = scratch.0.join("original.png");
    fs::write(
        &original,
        include_bytes!("../../assets/native/terminal.png"),
    )?;
    fs::set_permissions(&original, fs::Permissions::from_mode(0o644))?;
    draft.choose_icon(&original)?;
    let id = store.save(None, &draft)?;
    fs::remove_file(original)?;
    let restarted = Store {
        root: store.root.clone(),
    };
    assert_eq!(restarted.load(&id)?, draft);
    draft.name = "Edited".into();
    assert_eq!(store.save(Some(&id), &draft)?, id);
    let saved = fs::read(store.path(&id)?)?;
    draft.command = "missing-executable-vitrallis".into();
    assert!(store.save(Some(&id), &draft).is_err());
    assert_eq!(fs::read(store.path(&id)?)?, saved);
    let mut catalog = Catalog::default();
    restarted.integrate(&mut catalog)?;
    assert_eq!(catalog.apps.len(), 1);
    assert_eq!(catalog.apps[0].name, "Edited");
    assert_eq!(catalog.apps[0].source, AppSource::Custom);
    store.remove(&catalog.apps[0])?;
    assert!(restarted.load(&id).is_err());
    assert_eq!(fs::read_to_string(target)?, "#!/bin/sh\nprintf target\n");
    Ok(())
}

#[test]
fn corrupt_records_and_icons_do_not_block_other_shortcuts() -> Result<(), Box<dyn std::error::Error>>
{
    let (_scratch, store, draft) = fixture()?;
    let good = store.save(None, &draft)?;
    let bad = format!("{PREFIX}{}", "a".repeat(64));
    fs::write(store.path(&bad)?, b"{")?;
    let mut catalog = Catalog::default();
    store.integrate(&mut catalog)?;
    assert_eq!(catalog.apps[0].id, good);
    assert_eq!(catalog.apps.len(), 1);
    assert_eq!(catalog.diagnostics.len(), 1);
    let mut value: Value = serde_json::from_slice(&fs::read(store.path(&good)?)?)?;
    value["icon"] = json!([0, 1, 2]);
    fs::write(store.path(&good)?, serde_json::to_vec(&value)?)?;
    let loaded = store.load(&good)?;
    assert!(crate::renderer::decode_icon(loaded.icon.as_deref().ok_or("icon")?).is_err());
    assert_eq!(loaded.name, draft.name);
    assert!(store.path("../../target").is_err());
    Ok(())
}

#[test]
fn unmanaged_hiding_and_collisions_never_grant_uninstall_or_removal_authority() -> Result<(), String>
{
    let (_scratch, store, draft) = fixture()?;
    let id = store.save(None, &draft)?;
    let mut unmanaged = crate::platform::generic::demo_apps(Path::new("/vitrallis")).remove(0);
    store.hide(&unmanaged)?;
    let mut catalog = Catalog {
        apps: vec![unmanaged.clone()],
        ..Catalog::default()
    };
    store.integrate(&mut catalog)?;
    assert_eq!(catalog.apps.len(), 1);
    assert_eq!(catalog.apps[0].id, id);
    assert!(store.hide(&catalog.apps[0]).is_err());
    unmanaged.source = AppSource::AppCenter;
    unmanaged.id.clone_from(&id);
    assert!(store.remove(&unmanaged).is_err());
    assert!(store.hide(&unmanaged).is_err());
    for protected in [crate::app_center::TILE_ID, "vitrallis-wifi-settings"] {
        unmanaged.source = AppSource::System;
        unmanaged.id = protected.into();
        assert!(store.hide(&unmanaged).is_err());
    }
    assert!(store.load(&id).is_ok());
    Ok(())
}

#[test]
fn path_quoting_cwd_shebang_and_shell_launch_use_structured_commands()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, _store, _) = fixture()?;
    let root = scratch.0.canonicalize()?;
    let target = root.join("a script");
    fs::write(&target, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\"\n")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    let path = std::env::join_paths([&root])?;
    let manifest = command::manifest(
        "'a script' 'two words' '' '$HOME'",
        Mode::Direct,
        &root,
        &path,
    )?;
    assert_eq!(manifest.entry, target);
    let mut app = crate::platform::generic::demo_apps(Path::new("/vitrallis")).remove(0);
    app.manifest = manifest;
    let output = crate::process::command(&app)?
        .stdout(std::process::Stdio::piped())
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        format!("{}\ntwo words\n\n$HOME\n", root.display())
    );
    app.manifest = command::manifest("printf shell | cat > output", Mode::Shell, &root, &path)?;
    assert_eq!(app.manifest.entry, Path::new("/bin/sh"));
    assert_eq!(
        app.manifest.args,
        [OsString::from("-c"), "printf shell | cat > output".into()]
    );
    assert!(!root.join("output").exists());
    assert!(crate::process::command(&app)?.status()?.success());
    assert_eq!(fs::read(root.join("output"))?, b"shell");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644))?;
    assert!(command::manifest("'a script'", Mode::Direct, &root, &path).is_err());
    assert!(command::manifest("echo", Mode::Direct, &root.join("missing"), &path).is_err());
    Ok(())
}
