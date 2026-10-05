#![forbid(unsafe_code)]
use std::{os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

// Fixture directories use explicit modes: the discovery/storage checks reject
// group-writable paths, and developer umasks vary (the handheld test image uses
// 002).
fn secure_directory(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[test]
fn sdl_event_loop_launches_reaps_and_renders_recovery() -> Result<(), Box<dyn std::error::Error>> {
    for mode in ["auto", "software"] {
        let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
            .env("SDL_VIDEODRIVER", "dummy")
            .args(["--renderer", mode, "--smoke-test"])
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let log = String::from_utf8_lossy(&output.stderr);
        assert!(log.contains("event=app_started"));
        assert!(log.contains("event=app_exited"));
        assert!(log.contains("event=smoke_passed"));
        assert!(log.contains(&format!("requested={mode} mode=software")));
        assert!(log.contains("accelerated=false software=true"));
        assert!(log.contains("video_driver=\"dummy\""));
        assert!(log.contains(if mode == "auto" {
            "fallback=true hardware_error="
        } else {
            "fallback=false"
        }));
    }
    Ok(())
}

#[test]
fn required_hardware_fails_clearly_on_dummy_video() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
        .env("SDL_VIDEODRIVER", "dummy")
        .args(["--renderer", "hardware", "--smoke-test"])
        .output()?;
    assert!(!output.status.success());
    let log = String::from_utf8_lossy(&output.stderr);
    assert!(log.contains("hardware renderer unavailable"));
    assert!(log.contains("--renderer software"));
    assert!(!log.contains("event=renderer_initialized"));
    assert!(!log.contains("event=smoke_passed"));
    Ok(())
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        let path =
            std::env::temp_dir().join(format!("vitrallis render test {}", std::process::id()));
        secure_directory(&path, 0o700)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("scratch cleanup failed: {error}");
        }
    }
}

#[test]
fn renderer_outputs_native_size_bmp_and_refuses_overwrite() -> Result<(), Box<dyn std::error::Error>>
{
    let scratch = Scratch::new()?;
    let file = scratch.0.join("preview.bmp");
    let mut command = Command::new(env!("CARGO_BIN_EXE_vitrallis"));
    let _base_command = command
        .current_dir(&scratch.0)
        .env("SDL_VIDEODRIVER", "dummy")
        .args(["--demo", "--size", "480x272", "--screenshot"])
        .arg(&file);
    let first = command.output()?;
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let log = String::from_utf8_lossy(&first.stderr);
    assert!(log.contains("requested=auto mode=software"));
    assert!(log.contains("fallback=true hardware_error="));
    let bytes = std::fs::read(&file)?;
    assert_eq!(bytes.get(..2).ok_or("Missing fixture element")?, b"BM");
    assert_eq!(
        bytes.get(18..22).ok_or("Missing fixture element")?,
        &480_u32.to_le_bytes()
    );
    assert_eq!(
        bytes.get(22..26).ok_or("Missing fixture element")?,
        &272_u32.to_le_bytes()
    );
    // A frame must contain more than a cleared background.
    assert!(
        bytes
            .get(54..)
            .ok_or("Missing BMP pixel data")?
            .windows(3)
            .any(|pixel| pixel == {
                let accent = vitrallis_native::theme::ACCENT;
                [accent.b, accent.g, accent.r]
            })
    );
    assert!(!command.output()?.status.success());
    assert_eq!(std::fs::read(&file)?, bytes);
    let software_file = scratch.0.join("software.bmp");
    let software = command
        .args(["--renderer", "software", "--screenshot"])
        .arg(&software_file)
        .output()?;
    assert!(
        software.status.success(),
        "{}",
        String::from_utf8_lossy(&software.stderr)
    );
    assert_eq!(std::fs::read(software_file)?, bytes);
    assert!(String::from_utf8_lossy(&software.stderr).contains("requested=software mode=software"));
    Ok(())
}

#[test]
fn imported_catalog_is_read_only_and_missing_icons_render_safely()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("vitrallis-import-test-{}", std::process::id()));
    secure_directory(&root, 0o700)?;
    let scratch = Scratch(root);
    let config = scratch.0.join("config.json");
    let data = br#"{"pages":[{"name":"Apps","items":[{"name":"Real Label","shell":"/bin/sh","icon":"missing.png"},{"name":"Broken","shell":"/missing/vitrallis","icon":""},]}]}"#;
    std::fs::write(&config, data)?;
    let listed = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
        .arg("--app-config")
        .arg(&config)
        .arg("--assets")
        .arg(&scratch.0)
        .arg("--list-apps")
        .output()?;
    assert!(listed.status.success());
    let catalog: serde_json::Value = serde_json::from_slice(&listed.stdout)?;
    assert_eq!(catalog_field(&catalog, "/apps/4/name")?, "Real Label");
    assert_eq!(catalog_field(&catalog, "/apps/5/name")?, "Broken");
    assert!(catalog_field(&catalog, "/apps/5/unavailable")?.is_string());
    let frame = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
        .env("SDL_VIDEODRIVER", "dummy")
        .arg("--app-config")
        .arg(&config)
        .arg("--assets")
        .arg(&scratch.0)
        .arg("--screenshot")
        .arg(scratch.0.join("import.bmp"))
        .output()?;
    assert!(
        frame.status.success(),
        "{}",
        String::from_utf8_lossy(&frame.stderr)
    );
    assert!(String::from_utf8_lossy(&frame.stderr).contains("event=icon_fallback"));
    assert_eq!(std::fs::read(&config)?, data);
    Ok(())
}

fn listed(command: &mut Command) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let output = command.output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn catalog_field<'a>(
    catalog: &'a serde_json::Value,
    path: &str,
) -> Result<&'a serde_json::Value, Box<dyn std::error::Error>> {
    catalog
        .pointer(path)
        .ok_or_else(|| "Missing catalog fixture field".into())
}

#[test]
fn catalog_paths_and_device_session_entries_survive_import_boundaries()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("vitrallis catalog paths {}", std::process::id()));
    secure_directory(&root, 0o700)?;
    let scratch = Scratch(root.canonicalize()?);
    let home = scratch.0.join("user home");
    let assets = scratch.0.join("exported assets");
    secure_directory(&home.join(".pocket-home"), 0o755)?;
    secure_directory(&home, 0o755)?;
    secure_directory(&assets, 0o755)?;
    let user_config = home.join(".pocket-home/config.json");
    let default_config = assets.join("config.json");
    let default =
        br#"{"pages":[{"name":"Apps","items":[{"name":"Default","shell":"/bin/sh","icon":""}]}]}"#;
    std::fs::write(&default_config, default)?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_vitrallis"));
    let _desktop_command = command
        .current_dir(&scratch.0)
        .env("HOME", &home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env_remove("VITRALLIS_SESSION")
        .args(["--assets", "exported assets", "--list-apps"]);
    let desktop = listed(&mut command)?;
    assert_eq!(
        (*desktop.get("apps").ok_or("Missing fixture element")?)
            .as_array()
            .ok_or("missing apps")?
            .len(),
        4
    );
    assert_eq!(
        (*desktop
            .get("diagnostics")
            .ok_or("Missing fixture element")?)
        .as_array()
        .ok_or("diagnostics")?,
        &Vec::<serde_json::Value>::new()
    );
    let _handheld_command = command.arg("--linux-handheld");
    let fallback = listed(&mut command)?;
    assert_eq!(
        catalog_field(&fallback, "/apps/3/id")?,
        "vitrallis-app-center"
    );
    assert_eq!(catalog_field(&fallback, "/apps/4/name")?, "Default");
    assert!(!user_config.exists());

    let user = serde_json::to_vec(&serde_json::json!({"pages": [{"name": "Apps", "items": [
        {"name": "Terminal", "shell": "/usr/bin/lxterminal -e nmtui", "icon": ""},
        {"name": "Vitrallis", "shell": format!("\"{}\"", home.join(".local/share/vitrallis/launch").display()), "icon": ""}
    ]}]}))?;
    std::fs::write(&user_config, b"unrelated malformed user config")?;
    assert_eq!(
        catalog_field(&listed(&mut command)?, "/apps/4/name")?,
        "Default"
    );
    let _explicit_command = command
        .arg("--app-config")
        .arg(scratch.0.join("explicit.json"));
    std::fs::write(scratch.0.join("explicit.json"), &user)?;
    let configured = listed(&mut command)?;
    assert_eq!(
        (*configured.get("apps").ok_or("Missing fixture element")?)
            .as_array()
            .ok_or("missing apps")?
            .len(),
        6
    );
    assert_eq!(catalog_field(&configured, "/apps/4/name")?, "Terminal");
    assert_eq!(
        catalog_field(&configured, "/apps/4/args")?,
        &serde_json::json!(["--no-remote", "-e", "nmtui"])
    );

    let device = listed(&mut command)?;
    assert_eq!(
        (*device.get("apps").ok_or("Missing fixture element")?)
            .as_array()
            .ok_or("missing apps")?
            .len(),
        6
    );
    assert_eq!(
        catalog_field(&device, "/apps/4/id")?,
        catalog_field(&configured, "/apps/4/id")?
    );
    assert_eq!(
        catalog_field(&device, "/apps/4/args")?,
        &serde_json::json!(["--no-remote", "-e", "nmtui"])
    );
    assert_eq!(catalog_field(&device, "/apps/5/name")?, "Vitrallis");
    assert_eq!(
        catalog_field(&device, "/apps/3/id")?,
        "vitrallis-app-center"
    );

    assert_session_catalog(&mut command, &home)?;
    assert_eq!(std::fs::read(&default_config)?, default);
    assert_eq!(
        std::fs::read(&user_config)?,
        b"unrelated malformed user config"
    );
    Ok(())
}

fn assert_session_catalog(
    command: &mut Command,
    home: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let _session_command = command.env("VITRALLIS_SESSION", "1");
    let session = listed(command)?;
    assert_eq!(
        (*session.get("apps").ok_or("Missing fixture element")?)
            .as_array()
            .ok_or("missing apps")?
            .len(),
        6,
        "session catalogue retains the full menu"
    );
    assert_eq!(
        catalog_field(&session, "/apps/5/id")?,
        "vitrallis-exit-session",
        "the managed session gets its exit action"
    );
    assert_eq!(
        catalog_field(&session, "/apps/5/entry")?,
        "/usr/bin/python3",
        "session exit uses the supervised Python helper"
    );
    assert_eq!(
        catalog_field(&session, "/apps/5/args")?,
        &serde_json::json!([
            home.join(".local/share/vitrallis/vitrallis-session.py"),
            "stop"
        ]),
        "session exit calls the current helper with stop"
    );
    assert_eq!(
        catalog_field(&session, "/apps/3/id")?,
        "vitrallis-app-center",
        "App Center stays available in a managed session"
    );
    assert_eq!(
        catalog_field(&session, "/apps/5/name")?,
        "Exit Vitrallis",
        "the session exit label is visible"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn fifo_catalog_is_rejected_without_blocking() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, Instant};
    let root = std::env::temp_dir().join(format!("vitrallis-fifo-test-{}", std::process::id()));
    secure_directory(&root, 0o700)?;
    let scratch = Scratch(root);
    let fifo = scratch.0.join("config.json");
    assert!(Command::new("mkfifo").arg(&fifo).status()?.success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
        .env("HOME", &scratch.0)
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .arg("--app-config")
        .arg(&fifo)
        .arg("--list-apps")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child.kill()?;
            let _reaped_status = child.wait()?;
            return Err("FIFO blocked catalogue discovery".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    let catalog: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let apps = (*catalog.get("apps").ok_or("Missing fixture element")?)
        .as_array()
        .ok_or("missing apps")?;
    assert_eq!(apps.len(), 4);
    assert_eq!(
        (*apps
            .get(3)
            .ok_or("Missing fixture element")?
            .get("id")
            .ok_or("Missing fixture element")?),
        "vitrallis-app-center"
    );
    assert!(
        (*apps
            .get(3)
            .ok_or("Missing fixture element")?
            .get("unavailable")
            .ok_or("Missing fixture element")?)
        .is_null()
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("regular file"));
    Ok(())
}

/// Optional real-backend regression: run explicitly in a graphical session or
/// a physical Mesa GPU session. Ordinary CI needs only the dummy-driver tests above.
#[test]
#[ignore = "requires an SDL accelerated backend and a graphical session"]
fn accelerated_readback_and_presentation() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("vitrallis-accelerated-{}", std::process::id()));
    secure_directory(&root, 0o700)?;
    let scratch = Scratch(root);
    let mut command = Command::new(env!("CARGO_BIN_EXE_vitrallis"));
    let _graphics_command = command
        .current_dir(&scratch.0)
        .env("HOME", &scratch.0)
        .env("XDG_CONFIG_HOME", scratch.0.join("config"))
        .env("XDG_DATA_HOME", scratch.0.join("data"))
        .env_remove("VITRALLIS_SESSION")
        .arg("--demo");
    for (size, width, height) in [("480x272", 480_u32, 272_u32), ("800x480", 800, 480)] {
        let mut baseline = Vec::new();
        for mode in ["software", "hardware", "auto"] {
            let file = scratch.0.join(format!("{size}-{mode}.bmp"));
            let output = command
                .args(["--size", size, "--renderer", mode, "--screenshot"])
                .arg(&file)
                .output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let log = String::from_utf8_lossy(&output.stderr);
            assert!(log.contains(if mode == "software" {
                "mode=software"
            } else {
                "mode=hardware"
            }));
            assert!(log.contains("fallback=false"));
            let bytes = std::fs::read(file)?;
            assert_eq!(bytes.get(..2).ok_or("Missing fixture element")?, b"BM");
            assert_eq!(
                bytes.get(18..22).ok_or("Missing fixture element")?,
                &width.to_le_bytes()
            );
            assert_eq!(
                bytes.get(22..26).ok_or("Missing fixture element")?,
                &height.to_le_bytes()
            );
            assert!(
                bytes
                    .get(54..)
                    .ok_or("Missing BMP pixel data")?
                    .windows(3)
                    .any(|pixel| pixel == {
                        let accent = vitrallis_native::theme::ACCENT;
                        [accent.b, accent.g, accent.r]
                    })
            );
            if baseline.is_empty() {
                baseline = bytes;
            } else {
                assert_eq!(bytes.len(), baseline.len());
                assert_eq!(
                    bytes.get(..54).ok_or("Missing fixture element")?,
                    baseline.get(..54).ok_or("Missing fixture element")?
                );
                // Allow small filtering/rounding differences across backends,
                // while detecting blank, flipped, shifted or color-swapped readback.
                let different = (*bytes.get(54..).ok_or("Missing fixture element")?)
                    .iter()
                    .zip(baseline.get(54..).ok_or("Missing fixture element")?)
                    .filter(|(a, b)| a.abs_diff(**b) > 2)
                    .count();
                assert!(
                    different < (bytes.len() - 54) / 100,
                    "{size} {mode}: {different} differing channels"
                );
            }
        }
    }
    // Use a fresh command: screenshot and smoke are intentionally separate modes.
    let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
        .current_dir(&scratch.0)
        .env("HOME", &scratch.0)
        .env("XDG_CONFIG_HOME", scratch.0.join("config"))
        .env("XDG_DATA_HOME", scratch.0.join("data"))
        .env_remove("VITRALLIS_SESSION")
        .args(["--renderer", "hardware", "--smoke-test"])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = String::from_utf8_lossy(&output.stderr);
    assert!(log.contains("mode=hardware"));
    assert!(log.contains("event=smoke_passed"));
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires an SDL accelerated backend and a graphical session"]
fn accelerated_fullscreen_dimensions_match_readback() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("vitrallis-fullscreen-{}", std::process::id()));
    secure_directory(&root, 0o700)?;
    let scratch = Scratch(root);
    for mode in ["software", "hardware"] {
        let file = scratch.0.join(format!("{mode}.bmp"));
        let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
            .current_dir(&scratch.0)
            .env("HOME", &scratch.0)
            .env("XDG_CONFIG_HOME", scratch.0.join("config"))
            .env("XDG_DATA_HOME", scratch.0.join("data"))
            .env_remove("VITRALLIS_SESSION")
            .args([
                "--demo",
                "--linux-handheld",
                "--size",
                "480x272",
                "--renderer",
                mode,
                "--screenshot",
            ])
            .arg(file.as_os_str())
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = std::fs::read(file)?;
        let width = u32::from_le_bytes(bytes.get(18..22).ok_or("Missing BMP width")?.try_into()?);
        let height = u32::from_le_bytes(bytes.get(22..26).ok_or("Missing BMP height")?.try_into()?);
        let log = String::from_utf8_lossy(&output.stderr);
        assert!(log.contains(&format!("requested={mode} mode={mode}")));
        assert!(log.contains(&format!("window_width={width} window_height={height}")));
        assert!(log.contains(&format!("output_width={width} output_height={height}")));
        assert!(log.contains(&format!("display_width={width} display_height={height}")));
    }
    Ok(())
}

#[test]
fn graphics_commands_work_without_user_data_and_keep_mode_failures()
-> Result<(), Box<dyn std::error::Error>> {
    for command in ["--graphics-info", "--graphics-test"] {
        for mode in ["auto", "software", "hardware"] {
            let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
                .env("SDL_VIDEODRIVER", "dummy")
                .env("HOME", "/nonexistent-vitrallis-graphics-home")
                .args([command, "--renderer", mode])
                .output()?;
            let report = String::from_utf8_lossy(&output.stdout);
            assert!(report.contains("Available SDL renderers:"));
            assert_eq!(
                output.status.success(),
                mode != "hardware",
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if mode == "hardware" {
                assert!(
                    String::from_utf8_lossy(&output.stderr)
                        .contains("hardware renderer unavailable")
                );
            } else {
                assert!(report.contains("SDL renderer mode: software"));
                assert_eq!(
                    report.contains("Graphics self-test: PASS"),
                    command == "--graphics-test"
                );
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Linux Mesa with a graphical session; API-path test, not physical GPU validation"]
fn mesa_software_is_not_hardware() -> Result<(), Box<dyn std::error::Error>> {
    for mode in ["auto", "hardware", "software"] {
        let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .args(["--renderer", mode, "--graphics-test"])
            .output()?;
        assert_eq!(
            output.status.success(),
            mode != "hardware",
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if mode != "software" {
            let log = String::from_utf8_lossy(&output.stderr);
            assert!(log.contains("software Mesa renderer"), "{log}");
        }
        if mode != "hardware" {
            assert!(String::from_utf8_lossy(&output.stdout).contains("Graphics self-test: PASS"));
        }
    }
    Ok(())
}
