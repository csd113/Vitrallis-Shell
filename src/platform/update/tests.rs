use super::*;
use crate::test_support::Scratch;
use std::io::Write;

fn fixture() -> Result<(Scratch, PathBuf), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let target = scratch.0.join("vitrallis");
    fs::write(&target, b"working shell")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    Ok((scratch, target))
}

#[test]
fn atomic_install_retains_backup_and_never_touches_apps() -> Result<(), Box<dyn std::error::Error>>
{
    let (scratch, target) = fixture()?;
    let apps = scratch.0.join("apps");
    fs::create_dir(&apps)?;
    let app = apps.join("installed-app");
    fs::write(&app, b"app must remain unchanged")?;
    let app_before = fs::metadata(&app)?;
    let installation = Installation::open(&target)?;
    let mut file = installation.payload()?;
    file.write_all(b"replacement shell")?;
    file.set_permissions(fs::Permissions::from_mode(0o755))?;
    file.sync_all()?;
    drop(file);
    assert_eq!(fs::read(&target)?, b"working shell");
    assert!(installation.commit()?);
    assert_eq!(fs::read(&target)?, b"replacement shell");
    assert_eq!(
        fs::read(installation.stage.join("previous"))?,
        b"working shell"
    );
    assert_eq!(fs::read(&app)?, b"app must remain unchanged");
    assert!(same_file(&app_before, &fs::metadata(&app)?));
    assert_eq!(fs::read_dir(&apps)?.count(), 1);
    drop(installation);
    drop(Installation::open(&target)?);
    Ok(())
}

#[test]
fn completed_installation_releases_lock_with_a_shared_descriptor()
-> Result<(), Box<dyn std::error::Error>> {
    let (_scratch, target) = fixture()?;
    let installation = Installation::open(&target)?;
    // A concurrent fork can retain the same open file description until exec.
    let inherited = installation.lock.try_clone()?;
    assert!(Installation::open(&target).is_err());
    drop(installation);
    let retry = Installation::open(&target)?;
    drop(inherited);
    // Closing the old descriptor must not release the new installation's lock.
    assert!(Installation::open(&target).is_err());
    drop(retry);
    drop(Installation::open(&target)?);
    Ok(())
}

#[test]
fn failure_and_interruption_preserve_shell_and_cleanup_payload()
-> Result<(), Box<dyn std::error::Error>> {
    let (_scratch, target) = fixture()?;
    let installation = Installation::open(&target)?;
    assert!(Installation::open(&target).is_err());
    // Model a rename failure after backup by leaving no payload.
    assert!(installation.commit().is_err());
    assert_eq!(fs::read(&target)?, b"working shell");
    let mut file = installation.payload()?;
    file.write_all(b"partial")?;
    drop(file);
    let stage = installation.stage.clone();
    drop(installation);
    assert!(!stage.join("download").exists());
    // Model SIGKILL/power loss residue; the next locked attempt discards it.
    fs::write(stage.join("download"), b"interrupted download")?;
    let retry = Installation::open(&target)?;
    assert!(!stage.join("download").exists());
    assert_eq!(fs::read(&target)?, b"working shell");
    drop(retry);
    Ok(())
}

#[test]
fn unsafe_paths_permissions_and_changed_installations_are_refused()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, target) = fixture()?;
    let link = scratch.0.join("symlink");
    std::os::unix::fs::symlink(&target, &link)?;
    assert!(Installation::open(&link).is_err());
    fs::set_permissions(&target, fs::Permissions::from_mode(0o777))?;
    assert!(Installation::open(&target).is_err());
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    let installation = Installation::open(&target)?;
    fs::write(&target, b"user changed the installed shell")?;
    assert!(installation.commit().is_err());
    assert_eq!(fs::read(&target)?, b"user changed the installed shell");
    Ok(())
}

#[test]
fn failed_executable_probe_cannot_replace_working_shell() -> Result<(), Box<dyn std::error::Error>>
{
    let (_scratch, target) = fixture()?;
    let installation = Installation::open(&target)?;
    let mut file = installation.payload()?;
    file.write_all(b"not an executable")?;
    assert!(
        installation
            .ready(file, &semver::Version::new(1, 0, 0))
            .is_err()
    );
    assert_eq!(fs::read(&target)?, b"working shell");
    Ok(())
}

#[test]
fn unwritable_installation_reports_error_without_touching_shell()
-> Result<(), Box<dyn std::error::Error>> {
    let (scratch, target) = fixture()?;
    // Root bypasses discretionary filesystem permissions; exercise this on
    // ordinary-user hosts and retain the unsafe-permission tests on root CI.
    if fs::metadata(&target)?.uid() != 0 {
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o555))?;
        let result = Installation::open(&target);
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o755))?;
        assert!(result.is_err_and(|error| error.kind() == io::ErrorKind::PermissionDenied));
        assert_eq!(fs::read(&target)?, b"working shell");
    }
    Ok(())
}

#[test]
fn startup_probe_checks_version_before_atomic_replacement() -> Result<(), Box<dyn std::error::Error>>
{
    let (_scratch, target) = fixture()?;
    let installation = Installation::open(&target)?;
    let mut file = installation.payload()?;
    // A local test probe only; production rejects scripts at the ELF gate first.
    file.write_all(b"#!/bin/sh\nprintf 'vitrallis 1.2.3\\n'\n")?;
    installation.ready(file, &semver::Version::new(1, 2, 3))?;
    assert_eq!(fs::read(&target)?, b"working shell");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(installation.stage.join("download"))?;
    assert!(
        installation
            .ready(file, &semver::Version::new(2, 0, 0))
            .is_err()
    );
    assert_eq!(fs::read(&target)?, b"working shell");
    Ok(())
}
