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
            std::env::temp_dir().join(format!("vitrallis-render-test-{}", std::process::id()));
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
        .env("SDL_VIDEODRIVER", "dummy")
        .args(["--size", "480x272", "--screenshot"])
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
