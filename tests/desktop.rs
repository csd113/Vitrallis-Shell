use std::{path::PathBuf, process::Command};

#[test]
fn sdl_event_loop_launches_reaps_and_renders_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
        .env("SDL_VIDEODRIVER", "dummy")
        .arg("--smoke-test")
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
    Ok(())
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        let path =
            std::env::temp_dir().join(format!("vitrallis render test {}", std::process::id()));
        std::fs::create_dir(&path)?;
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
    command
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
    let bytes = std::fs::read(&file)?;
    assert_eq!(&bytes[..2], b"BM");
    assert_eq!(&bytes[18..22], &480_u32.to_le_bytes());
    assert_eq!(&bytes[22..26], &272_u32.to_le_bytes());
    // A frame must contain more than a cleared background.
    assert!(bytes[54..].windows(3).any(|pixel| pixel == [201, 218, 93]));
    assert!(!command.output()?.status.success());
    assert_eq!(std::fs::read(&file)?, bytes);
    Ok(())
}

#[test]
fn imported_catalog_is_read_only_and_missing_icons_render_safely()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("vitrallis-import-test-{}", std::process::id()));
    std::fs::create_dir(&root)?;
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
    assert_eq!(catalog["apps"][0]["name"], "Real Label");
    assert_eq!(catalog["apps"][1]["name"], "Broken");
    assert!(catalog["apps"][1]["unavailable"].is_string());
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

#[test]
fn existing_background_color_and_wallpaper_render_without_config_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::env::temp_dir().join(format!("vitrallis wallpaper test {}", std::process::id()));
    std::fs::create_dir(&root)?;
    let scratch = Scratch(root);
    let config = scratch.0.join("source config.json");
    let assets = scratch.0.join("exported assets");
    std::fs::create_dir(&assets)?;
    let file = std::fs::File::create(assets.join("wallpaper.png"))?;
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&[10, 20, 30])?;
    for (name, background, expected) in [
        ("color", "FF0080", [128, 0, 255]),
        ("wallpaper", "wallpaper.png", [30, 20, 10]),
    ] {
        let content = serde_json::to_vec(&serde_json::json!({
            "pages": [{"name": "Apps", "items": []}], "background": background,
            "showclock": "no"
        }))?;
        std::fs::write(&config, &content)?;
        let screenshot = scratch.0.join(format!("{name}.bmp"));
        let output = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
            .current_dir(&scratch.0)
            .env("SDL_VIDEODRIVER", "dummy")
            .args([
                "--app-config",
                "source config.json",
                "--assets",
                "exported assets",
            ])
            .args(["--size", "480x272", "--screenshot"])
            .arg(&screenshot)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = std::fs::read(screenshot)?;
        let offset = usize::try_from(u32::from_le_bytes(bytes[10..14].try_into()?))?;
        assert_eq!(&bytes[offset..offset + 3], expected);
        assert_eq!(std::fs::read(&config)?, content);
    }
    Ok(())
}

#[test]
fn catalog_paths_and_device_session_entries_survive_import_boundaries()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("vitrallis catalog paths {}", std::process::id()));
    std::fs::create_dir(&root)?;
    let scratch = Scratch(root);
    let home = scratch.0.join("user home");
    let assets = scratch.0.join("exported assets");
    std::fs::create_dir_all(home.join(".pocket-home"))?;
    std::fs::create_dir(&assets)?;
    let user_config = home.join(".pocket-home/config.json");
    let default_config = assets.join("config.json");
    let default =
        br#"{"pages":[{"name":"Apps","items":[{"name":"Default","shell":"/bin/sh","icon":""}]}]}"#;
    std::fs::write(&default_config, default)?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_vitrallis"));
    command
        .current_dir(&scratch.0)
        .env("HOME", &home)
        .env_remove("VITRALLIS_SESSION")
        .args(["--assets", "exported assets", "--list-apps"]);
    let list = |command: &mut Command| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let output = command.output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(serde_json::from_slice(&output.stdout)?)
    };
    let fallback = list(&mut command)?;
    assert_eq!(fallback["apps"][0]["name"], "Default");
    assert!(!user_config.exists());

    let user = serde_json::to_vec(&serde_json::json!({"pages": [{"name": "Apps", "items": [
        {"name": "Terminal", "shell": "/usr/bin/lxterminal", "icon": ""},
        {"name": "Vitrallis", "shell": format!("\"{}\"", home.join(".local/share/vitrallis/launch").display()), "icon": ""}
    ]}]}))?;
    std::fs::write(&user_config, &user)?;
    let desktop = list(&mut command)?;
    assert_eq!(desktop["apps"].as_array().ok_or("missing apps")?.len(), 3);
    assert_eq!(desktop["apps"][0]["name"], "Terminal");
    assert_eq!(desktop["apps"][0]["args"], serde_json::json!([]));

    command.arg("--pocketchip");
    let device = list(&mut command)?;
    assert_eq!(device["apps"].as_array().ok_or("missing apps")?.len(), 3);
    assert_eq!(device["apps"][0]["id"], desktop["apps"][0]["id"]);
    assert_eq!(
        device["apps"][0]["args"],
        serde_json::json!(["--no-remote"])
    );
    assert_eq!(device["apps"][1]["name"], "Vitrallis");
    assert_eq!(device["apps"][2]["id"], "vitrallis-app-center");

    command.env("VITRALLIS_SESSION", "1");
    let session = list(&mut command)?;
    assert_eq!(session["apps"].as_array().ok_or("missing apps")?.len(), 3);
    assert_eq!(session["apps"][1]["id"], "vitrallis-return-marshmallow");
    assert_eq!(session["apps"][1]["entry"], "/usr/bin/systemctl");
    assert_eq!(
        session["apps"][1]["args"],
        serde_json::json!(["--user", "stop", "vitrallis-session.service"])
    );
    assert_eq!(session["apps"][2]["id"], "vitrallis-app-center");
    assert_eq!(std::fs::read(&default_config)?, default);
    assert_eq!(std::fs::read(&user_config)?, user);
    Ok(())
}

#[cfg(unix)]
#[test]
fn fifo_catalog_is_rejected_without_blocking() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, Instant};
    let root = std::env::temp_dir().join(format!("vitrallis-fifo-test-{}", std::process::id()));
    std::fs::create_dir(&root)?;
    let scratch = Scratch(root);
    let fifo = scratch.0.join("config.json");
    assert!(Command::new("mkfifo").arg(&fifo).status()?.success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_vitrallis"))
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
            child.wait()?;
            return Err("FIFO blocked catalogue discovery".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    let catalog: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let apps = catalog["apps"].as_array().ok_or("missing apps")?;
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["id"], "vitrallis-app-center");
    assert!(apps[0]["unavailable"].is_null());
    assert!(String::from_utf8_lossy(&output.stderr).contains("regular file"));
    Ok(())
}
