//! Real native package lifecycle; fixture compilation stays on the build host.
#![cfg(target_os = "linux")]
use super::{
    install,
    metadata::{self, Files},
    running::{self, Processes},
    tests, uninstall,
};
use std::{
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn binary(directory: &Path, version: &str) -> Result<Vec<u8>, String> {
    let output = if let Some(directory) = std::env::var_os("VITRALLIS_QA_NATIVE_FIXTURES") {
        PathBuf::from(directory).join(version)
    } else {
        let source = directory.join(format!("{version}.rs"));
        let source_text = format!(
            "fn main() {{ println!(\"{version}\"); std::thread::sleep(std::time::Duration::from_secs(60)); }}"
        );
        std::fs::write(&source, source_text).map_err(|e| e.to_string())?;
        let output = directory.join(version);
        let result = Command::new("rustc")
            .args(["--crate-name", "fixture", "-C", "strip=symbols", "-o"])
            .arg(&output)
            .arg(&source)
            .output()
            .map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(String::from_utf8_lossy(&result.stderr).into_owned());
        }
        output
    };
    std::fs::read(output).map_err(|e| e.to_string())
}
fn package(version: &str, bytes: Vec<u8>) -> Result<(metadata::Package, Files), String> {
    let (mut p, mut files) = tests::generic()?;
    files.remove("main.py");
    files.remove("requirements.txt");
    let target = metadata::native_target();
    let manifest = format!(
        "manifest_version = 1\nname = {:?}\nid = {:?}\nversion = {:?}\nruntime = \"rust\"\n[binaries]\n{target} = \"bin/fixture\"\n[permissions]\nnetwork = false\naudio = false\nstorage = false\n",
        p.name, p.id, version
    );
    files.insert("app.toml".into(), manifest.into_bytes());
    files.insert("bin/fixture".into(), bytes);
    p.version = metadata::version(version)?;
    p.runtime = metadata::RuntimeKind::Rust([(target.into(), "bin/fixture".into())].into());
    p.entry = "bin/fixture".into();
    p.permissions = serde_json::json!({"network":false,"audio":false,"storage":false});
    Ok((tests::inventory(p, &files), files))
}
#[test]
fn native_install_launch_process_detection_update_and_uninstall() -> Result<(), String> {
    let (scratch, loc) = tests::locations()?;
    let (p, files) = package("0.1.0", binary(&scratch.0, "v1")?)?;
    install::install(&loc, &install::prepare(&loc, p.clone(), files.clone())?)?;
    let root = loc.root(&p);
    assert_eq!(
        std::fs::metadata(root.join(&p.entry))
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert!(!root.join("runtime").exists());
    assert!(install::prepare(&loc, p.clone(), files)?.prepared.is_none());
    std::fs::write(root.join("saved-data"), b"keep").map_err(|e| e.to_string())?;
    run_and_close(&loc, &p, "v1")?;
    let (next, files) = package("0.2.0", binary(&scratch.0, "v2")?)?;
    install::install(&loc, &install::prepare(&loc, next.clone(), files)?)?;
    run_and_close(&loc, &next, "v2")?;
    uninstall::uninstall(&loc, &next)?;
    assert!(!root.join("bin/fixture").exists());
    assert_eq!(
        std::fs::read(root.join("saved-data")).map_err(|e| e.to_string())?,
        b"keep"
    );
    Ok(())
}
fn run_and_close(
    loc: &super::storage::Locations,
    p: &metadata::Package,
    expected: &str,
) -> Result<(), String> {
    let mut child = Command::new(loc.state.join("launchers").join(&p.id))
        .env("HOME", &loc.home)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let entry = loc.root(p).join(&p.entry);
    let result = (|| {
        let deadline = Instant::now() + Duration::from_secs(5);
        let identities = loop {
            let identities = running::Native.list(&entry)?;
            if identities.iter().any(|identity| identity.pid == child.id()) {
                break identities;
            }
            if Instant::now() > deadline {
                return Err("Native app was not detected".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(uninstall::uninstall(loc, p).is_err());
        // Read the version before TERM; this also proves the binary executed.
        let mut line = String::new();
        BufReader::new(child.stdout.take().ok_or("stdout missing")?)
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        assert_eq!(line.trim(), expected);
        running::close(
            &running::Native,
            &entry,
            &identities,
            Duration::from_secs(3),
        )
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[test]
#[ignore = "explicit isolated physical UI fixture; requires VITRALLIS_QA_HOME and prebuilt binaries"]
fn physical_native_fixture() -> Result<(), String> {
    let home = std::env::var_os("VITRALLIS_QA_HOME")
        .map(PathBuf::from)
        .ok_or("Missing isolated QA home")?;
    super::storage::safe(&home)?;
    let loc = super::storage::Locations {
        data: home.join(".local/share"),
        state: home.join(".local/share/vitrallis/app-center"),
        sources: home.join(".config/vitrallis/app-center.json"),
        home,
    };
    let action = std::env::var("VITRALLIS_QA_NATIVE_ACTION").map_err(|e| e.to_string())?;
    let (version, fixture) = match action.as_str() {
        "install" => ("0.1.0", "v1"),
        "update" | "uninstall" => ("0.2.0", "v2"),
        _ => return Err("Unknown QA action".into()),
    };
    let (p, files) = package(version, binary(&loc.home, fixture)?)?;
    if action == "uninstall" {
        uninstall::uninstall(&loc, &p)?;
    } else {
        if !running::Native
            .list(&uninstall::installed_entry(&loc.root(&p), &p)?)?
            .is_empty()
        {
            return Err("Close the QA native app before updating".into());
        }
        install::install(&loc, &install::prepare(&loc, p, files)?)?;
    }
    eprintln!("Physical native fixture {action} passed");
    Ok(())
}
