use super::*;
use crate::test_support::Scratch;
use std::io::{Seek, SeekFrom, Write};

fn fixture() -> Result<(Scratch, PathBuf), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let generation = scratch
        .0
        .canonicalize()?
        .join("generations")
        .join("a".repeat(64));
    fs::create_dir_all(&generation)?;
    for name in bundle::BINARIES {
        let path = generation.join(name);
        fs::write(&path, format!("working {name}"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    symlink(
        PathBuf::from("generations").join("a".repeat(64)),
        scratch.0.join("current"),
    )?;
    Ok((scratch, generation.join("vitrallis")))
}
fn prepare(installation: &mut Installation) -> io::Result<()> {
    let path = installation.stage.join("generation");
    fs::create_dir(&path)?;
    for name in bundle::BINARIES {
        let file = path.join(name);
        fs::write(&file, format!("replacement {name}"))?;
        fs::set_permissions(file, fs::Permissions::from_mode(0o755))?;
    }
    installation.prepared = Some(Prepared {
        generation: PathBuf::from("generations").join("b".repeat(64)),
        shell_hash: hash_file(&path.join("vitrallis"))?,
    });
    Ok(())
}
#[test]
fn atomic_generation_switch_retains_all_old_binaries_and_independent_apps()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, target) = fixture()?;
    let apps = scratch.0.join("apps");
    fs::create_dir(&apps)?;
    fs::write(apps.join("sentinel"), "untouched")?;
    let store = crate::shortcuts::Store::at(scratch.0.canonicalize()?.join("shortcuts"));
    let draft = crate::shortcuts::Draft {
        name: "Retained shortcut".into(),
        command: "/bin/echo 'after shell update'".into(),
        cwd: scratch.0.canonicalize()?.to_string_lossy().into_owned(),
        icon: Some(include_bytes!("../../../assets/native/terminal.png").to_vec()),
        ..crate::shortcuts::Draft::default()
    };
    let shortcut = store.save(None, &draft)?;
    let mut installation = Installation::open(&target)?;
    prepare(&mut installation)?;
    for name in bundle::BINARIES {
        assert_eq!(
            fs::read_to_string(scratch.0.join("current").join(name))?,
            format!("working {name}")
        );
    }
    assert!(installation.commit()?);
    for name in bundle::BINARIES {
        assert_eq!(
            fs::read_to_string(scratch.0.join("current").join(name))?,
            format!("replacement {name}")
        );
        assert_eq!(
            fs::read_to_string(scratch.0.join("previous").join(name))?,
            format!("working {name}")
        );
    }
    assert_eq!(fs::read_to_string(apps.join("sentinel"))?, "untouched");
    assert_eq!(store.load(&shortcut)?, draft);
    assert_eq!(fs::read_to_string(&target)?, "working vitrallis");
    let relaunch = installation.relaunch_target()?;
    drop(installation);
    let (_guard, command) = relaunch_command(&relaunch, ["argument with spaces".into()])?;
    assert_eq!(command.get_program(), relaunch.executable);
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["argument with spaces"]
    );
    Ok(())
}
#[test]
fn locks_survive_cloned_descriptors_and_release_explicitly()
-> Result<(), Box<dyn std::error::Error>> {
    let (_scratch, target) = fixture()?;
    let installation = Installation::open(&target)?;
    let inherited = installation.lock.try_clone()?;
    assert!(Installation::open(&target).is_err());
    drop(installation);
    let next = Installation::open(&target)?;
    drop(inherited);
    assert!(Installation::open(&target).is_err());
    drop(next);
    assert!(Installation::open(&target).is_ok());
    Ok(())
}
#[test]
fn failed_unverified_or_interrupted_update_preserves_active_generation()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, target) = fixture()?;
    let installation = Installation::open(&target)?;
    assert!(installation.commit().is_err());
    let mut file = installation.payload()?;
    file.write_all(b"truncated")?;
    drop(file);
    let stage = installation.stage.clone();
    drop(installation);
    assert!(!stage.join("download").exists());
    fs::write(stage.join("download"), "interrupted")?;
    let _retry = Installation::open(&target)?;
    assert!(!stage.join("download").exists());
    assert_eq!(
        fs::read_to_string(scratch.0.join("current/vitrallis"))?,
        "working vitrallis"
    );
    Ok(())
}
#[test]
fn unsafe_or_modified_companion_blocks_update_before_switch()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, target) = fixture()?;
    let notepad = target
        .parent()
        .ok_or("no parent")?
        .join("vitrallis-notepad");
    fs::set_permissions(&notepad, fs::Permissions::from_mode(0o777))?;
    assert!(Installation::open(&target).is_err());
    fs::set_permissions(&notepad, fs::Permissions::from_mode(0o755))?;
    let mut installation = Installation::open(&target)?;
    prepare(&mut installation)?;
    fs::write(&notepad, "modified")?;
    assert!(installation.commit().is_err());
    assert_eq!(
        fs::read_link(scratch.0.join("current"))?,
        PathBuf::from("generations").join("a".repeat(64))
    );
    Ok(())
}
#[test]
fn path_symlinks_incomplete_inventory_and_pointer_escape_are_refused()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, target) = fixture()?;
    let link = scratch.0.join("vitrallis");
    symlink(&target, &link)?;
    assert!(Installation::open(&link).is_err());
    let files = target.parent().ok_or("no parent")?.join("vitrallis-files");
    fs::remove_file(&files)?;
    symlink(&target, &files)?;
    assert!(Installation::open(&target).is_err());
    fs::remove_file(scratch.0.join("current"))?;
    symlink("../../outside", scratch.0.join("current"))?;
    assert!(Installation::open(&target).is_err());
    Ok(())
}
#[test]
fn corrupt_bundle_and_foreign_binary_never_become_ready() -> Result<(), Box<dyn std::error::Error>>
{
    let (_scratch, target) = fixture()?;
    let mut installation = Installation::open(&target)?;
    let mut file = installation.payload()?;
    file.write_all(b"not a bundle")?;
    file.seek(SeekFrom::Start(0))?;
    assert!(
        installation
            .ready(
                file,
                &semver::Version::new(1, 2, 3),
                super::super::Target::for_triple("x86_64-unknown-linux-gnu")?,
                [0; 32]
            )
            .is_err()
    );
    assert!(installation.commit().is_err());
    assert_eq!(fs::read_to_string(target)?, "working vitrallis");
    Ok(())
}
#[test]
fn publication_collision_preserves_active_generation() -> Result<(), Box<dyn std::error::Error>> {
    let (scratch, target) = fixture()?;
    let mut installation = Installation::open(&target)?;
    prepare(&mut installation)?;
    let destination = scratch.0.join("generations").join("b".repeat(64));
    fs::create_dir(&destination)?;
    fs::write(destination.join("vitrallis"), "conflict")?;
    assert!(installation.commit().is_err());
    assert_eq!(
        fs::read_to_string(scratch.0.join("current/vitrallis"))?,
        "working vitrallis"
    );
    Ok(())
}
#[test]
fn relaunch_rejects_changed_binary_and_preserves_pid_in_exec()
-> Result<(), Box<dyn std::error::Error>> {
    let (_scratch, target) = fixture()?;
    let executable = b"#!/bin/sh\nprintf 'relaunch-pid=%s\\n%s\\n' \"$$\" \"$1\"\n";
    fs::write(&target, executable)?;
    let child = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "platform::update::unix::tests::relaunch_exec_helper",
            "--nocapture",
        ])
        .env("VITRALLIS_TEST_RELAUNCH_TARGET", &target)
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let pid = child.id();
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains(&format!("relaunch-pid={pid}\nargument with spaces\n")),
        "{stdout}"
    );
    let relaunch = super::super::Relaunch {
        executable: target.clone(),
        sha256: hash_file(&target)?,
    };
    fs::write(&target, b"modified")?;
    assert!(relaunch_command(&relaunch, []).is_err());
    Ok(())
}
#[test]
fn relaunch_exec_helper() -> Result<(), Box<dyn std::error::Error>> {
    let Some(target) = std::env::var_os("VITRALLIS_TEST_RELAUNCH_TARGET") else {
        return Ok(());
    };
    let target = PathBuf::from(target);
    let hash = hash_file(&target)?;
    relaunch(
        &super::super::Relaunch {
            executable: target,
            sha256: hash,
        },
        ["argument with spaces".into()],
    )?;
    Err("exec returned".into())
}

#[test]
fn incomplete_inventory_and_unsafe_arti_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let (_scratch, target) = fixture()?;
    let arti = target.with_file_name("arti");
    fs::remove_file(&arti)?;
    assert!(Installation::open(&target).is_err());
    symlink("missing", &arti)?;
    assert!(Installation::open(&target).is_err());
    Ok(())
}

#[test]
fn arti_version_is_independent_and_other_companions_remain_exact() -> Result<(), semver::Error> {
    let version = semver::Version::parse("0.1.0-beta4")?;
    assert!(version_matches(
        "arti",
        &version,
        "Arti 2.6.0\nRuntime: tokio\n"
    ));
    for wrong in [
        "arti 0.1.0-beta4",
        "Arti 2.5.0",
        "Arti 2.6.0-extra",
        "\nArti 2.6.0",
    ] {
        assert!(!version_matches("arti", &version, wrong));
    }
    assert!(version_matches(
        "vitrallis",
        &version,
        "vitrallis 0.1.0-beta4\n"
    ));
    assert!(!version_matches(
        "vitrallis",
        &version,
        "vitrallis 0.1.0-beta3.9"
    ));
    assert!(!version_matches(
        "vitrallis",
        &version,
        "vitrallis 0.1.0-beta4\nextra"
    ));
    Ok(())
}

// Run on Linux against actual release bytes, separately from offline unit tests.
#[test]
fn release_bundle_upgrade_probe() -> Result<(), Box<dyn std::error::Error>> {
    let Some(bundle_path) = std::env::var_os("VITRALLIS_TEST_UPDATE_BUNDLE") else {
        return Ok(());
    };
    let version = semver::Version::parse(&std::env::var("VITRALLIS_TEST_UPDATE_VERSION")?)?;
    let (scratch, target) = fixture()?;
    if std::env::var_os("VITRALLIS_TEST_FOUR_FILE_SOURCE").is_some() {
        fs::remove_file(target.with_file_name("arti"))?;
    }
    let mut installation = Installation::open(&target)?;
    let mut payload = installation.payload()?;
    let mut source = File::open(bundle_path)?;
    io::copy(&mut source, &mut payload)?;
    payload.seek(SeekFrom::Start(0))?;
    installation.ready(payload, &version, super::super::Target::current()?, [9; 32])?;
    assert!(installation.commit()?);
    assert!(scratch.0.join("current/arti").is_file());
    let relaunch = installation.relaunch_target()?;
    drop(installation);
    let (_guard, command) = relaunch_command(&relaunch, [])?;
    assert_eq!(command.get_program(), relaunch.executable);
    Ok(())
}
