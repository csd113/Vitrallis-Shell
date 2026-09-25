use super::*;
use crate::test_support::Scratch;
use std::io::{Seek, SeekFrom, Write};

// Creates fixture directories with explicit modes. The installer and updater
// safety checks reject group-writable installation directories, so fixtures
// must not inherit the developer's umask (the test device commonly uses 002).
fn secure_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o755);
    builder.create(path)
}

fn fixture() -> Result<(Scratch, PathBuf), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let generation = scratch
        .0
        .canonicalize()?
        .join("generations")
        .join("a".repeat(64));
    secure_directory(&generation)?;
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
    secure_directory(&path)?;
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

/// Two complete generations named by their own whole-bundle digest, with the
/// active one published through `current` and the retained one through `previous`.
struct RestoreFixture {
    _scratch: Scratch,
    root: PathBuf,
    active: PathBuf,
    previous: PathBuf,
}
impl RestoreFixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let scratch = Scratch::new()?;
        let root = scratch.0.canonicalize()?;
        secure_directory(&root.join("generations"))?;
        let active = build_generation(&root, "active")?;
        let previous = build_generation(&root, "previous")?;
        symlink(&active, root.join("current"))?;
        symlink(&previous, root.join("previous"))?;
        Ok(Self {
            _scratch: scratch,
            root,
            active,
            previous,
        })
    }
    fn target(&self) -> PathBuf {
        self.root.join(&self.active).join("vitrallis")
    }
    fn previous_target(&self) -> PathBuf {
        self.root.join(&self.previous).join("vitrallis")
    }
    fn uid(&self) -> Result<u32, std::io::Error> {
        Ok(fs::symlink_metadata(self.target())?.uid())
    }
    fn current(&self) -> Result<PathBuf, std::io::Error> {
        fs::read_link(self.root.join("current"))
    }
    fn previous_pointer(&self) -> Result<PathBuf, std::io::Error> {
        fs::read_link(self.root.join("previous"))
    }
    fn retains_both(&self) -> bool {
        [&self.active, &self.previous].iter().all(|generation| {
            bundle::BINARIES
                .iter()
                .all(|name| hash_file(&self.root.join(generation).join(name)).is_ok())
        })
    }
    fn previous_file(&self, name: &str) -> PathBuf {
        self.root.join(&self.previous).join(name)
    }
}
/// Writes a complete generation and names its directory by the whole-bundle
/// digest the installer and updater use.
fn build_generation(root: &Path, seed: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let staging = root
        .join("generations")
        .join(format!(".stage-{seed}-{}", std::process::id()));
    secure_directory(&staging)?;
    for name in bundle::BINARIES {
        let path = staging.join(name);
        fs::write(&path, format!("{seed} {name}"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    let (digest, _) = generation_digest(&staging, fs::symlink_metadata(&staging)?.uid())?;
    let relative = PathBuf::from("generations").join(hex(&digest));
    fs::rename(&staging, root.join(&relative))?;
    Ok(relative)
}

#[test]
fn restore_activates_the_validated_previous_generation() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    let installation = Installation::open(&fixture.target())?;
    let restored = installation.rollback()?;
    assert!(restored.durable);
    // The fixture files are not real executables, so the version probe is
    // best-effort and the digest is the integrity guarantee.
    assert_eq!(restored.version, None);
    assert_eq!(fixture.current()?, fixture.previous);
    assert_eq!(fixture.previous_pointer()?, fixture.active);
    assert_eq!(restored.relaunch.executable, fixture.previous_target());
    assert_eq!(
        restored.relaunch.sha256,
        hash_file(&restored.relaunch.executable)?
    );
    assert!(fixture.retains_both());
    drop(installation);
    // The restored generation is now the active, openable installation.
    let reopened = Installation::open(&fixture.previous_target())?;
    assert!(!reopened.needs_completion());
    Ok(())
}

#[test]
fn restore_without_a_previous_generation_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    fs::remove_file(fixture.root.join("previous"))?;
    assert!(!previous_available_at(
        &fixture.root,
        &fixture.active,
        fixture.uid()?
    ));
    let installation = Installation::open(&fixture.target())?;
    assert!(installation.rollback().is_err());
    assert_eq!(fixture.current()?, fixture.active);
    assert!(fixture.retains_both());
    Ok(())
}

#[test]
fn restore_refuses_an_identical_or_escaping_previous_pointer()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    for pointer in [
        fixture.active.clone(),
        PathBuf::from("../../outside"),
        PathBuf::from("/absolute/generation"),
        PathBuf::from("generations").join("A".repeat(64)),
        PathBuf::from("generations").join("a".repeat(63)),
    ] {
        fs::remove_file(fixture.root.join("previous"))?;
        symlink(&pointer, fixture.root.join("previous"))?;
        assert!(!previous_available_at(
            &fixture.root,
            &fixture.active,
            fixture.uid()?
        ));
        let installation = Installation::open(&fixture.target())?;
        assert!(installation.rollback().is_err(), "{}", pointer.display());
        assert_eq!(fixture.current()?, fixture.active);
        assert_eq!(fixture.previous_pointer()?, pointer);
    }
    Ok(())
}

#[test]
fn restore_refuses_an_incomplete_unsafe_or_modified_previous()
-> Result<(), Box<dyn std::error::Error>> {
    type Damage = Box<dyn Fn(&RestoreFixture) -> std::io::Result<()>>;
    for (label, structural, damage) in [
        (
            "missing companion",
            true,
            Box::new(|fixture: &RestoreFixture| fs::remove_file(fixture.previous_file("arti")))
                as Damage,
        ),
        (
            "group-writable executable",
            true,
            Box::new(|fixture: &RestoreFixture| {
                fs::set_permissions(
                    fixture.previous_file("vitrallis"),
                    fs::Permissions::from_mode(0o777),
                )
            }),
        ),
        (
            "symlinked executable",
            true,
            Box::new(|fixture: &RestoreFixture| {
                let path = fixture.previous_file("vitrallis-notepad");
                fs::remove_file(&path)?;
                symlink(fixture.root.join(&fixture.active).join("vitrallis"), path)
            }),
        ),
        // Structurally safe, but the content no longer matches the digest that
        // names the generation.
        (
            "modified executable",
            false,
            Box::new(|fixture: &RestoreFixture| {
                fs::write(fixture.previous_file("vitrallis"), b"tampered")
            }),
        ),
    ] {
        let fixture = RestoreFixture::new()?;
        damage(&fixture)?;
        assert_eq!(
            previous_available_at(&fixture.root, &fixture.active, fixture.uid()?),
            !structural,
            "{label}"
        );
        let installation = Installation::open(&fixture.target())?;
        assert!(installation.rollback().is_err(), "{label}");
        assert_eq!(fixture.current()?, fixture.active, "{label}");
        assert_eq!(fixture.previous_pointer()?, fixture.previous, "{label}");
        // The active generation is always untouched by a refused restore.
        assert!(hash_file(fixture.target().as_path()).is_ok(), "{label}");
    }
    Ok(())
}

#[test]
fn restore_refuses_when_the_active_build_changed_under_the_lock()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    let installation = Installation::open(&fixture.target())?;
    fs::write(
        fixture.root.join(&fixture.active).join("vitrallis"),
        b"changed",
    )?;
    let error = installation.rollback().expect_err("changed build");
    assert!(error.contains("Active build changed"), "{error}");
    assert_eq!(fixture.current()?, fixture.active);
    assert_eq!(fixture.previous_pointer()?, fixture.previous);
    Ok(())
}

#[test]
fn restore_ping_pongs_between_the_two_retained_generations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    Installation::open(&fixture.target())?.rollback()?;
    assert_eq!(fixture.current()?, fixture.previous);
    // Relaunching the restored generation keeps the displaced build available.
    Installation::open(&fixture.previous_target())?.rollback()?;
    assert_eq!(fixture.current()?, fixture.active);
    assert_eq!(fixture.previous_pointer()?, fixture.previous);
    assert!(fixture.retains_both());
    Ok(())
}

#[test]
fn restore_keeps_the_active_build_when_the_previous_pointer_cannot_be_written()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    let installation = Installation::open(&fixture.target())?;
    // `Installation::open` cleans stale `.previous-next` staging names; create
    // one afterwards so the second pointer rename cannot complete and the undo
    // path runs.
    let blocker = fixture.root.join(".previous-next");
    fs::create_dir(&blocker)?;
    let error = installation
        .rollback()
        .expect_err("pointer write must fail");
    assert!(error.contains("active build was kept"), "{error}");
    assert_eq!(fixture.current()?, fixture.active);
    assert_eq!(fixture.previous_pointer()?, fixture.previous);
    assert!(fixture.retains_both());
    fs::remove_dir(&blocker)?;
    Ok(())
}

#[test]
fn restore_refuses_a_symlinked_generation_directory() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    let directory = fixture.root.join(&fixture.previous);
    let moved = fixture.root.join("generations").join("moved-previous");
    fs::rename(&directory, &moved)?;
    symlink(&moved, &directory)?;
    assert!(!previous_available_at(
        &fixture.root,
        &fixture.active,
        fixture.uid()?
    ));
    let installation = Installation::open(&fixture.target())?;
    assert!(installation.rollback().is_err());
    assert_eq!(fixture.current()?, fixture.active);
    assert_eq!(fixture.previous_pointer()?, fixture.previous);
    Ok(())
}

#[test]
fn restore_runs_under_the_existing_update_lock() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = RestoreFixture::new()?;
    let installation = Installation::open(&fixture.target())?;
    // Another installation cannot open while the lock is held.
    assert!(Installation::open(&fixture.target()).is_err());
    // The lock owner can still restore, and the lock releases with it.
    installation.rollback()?;
    drop(installation);
    assert!(Installation::open(&fixture.previous_target()).is_ok());
    Ok(())
}
