//! Portable Step 1 launcher. Core tests run without initializing a display.
mod app;
mod config;
mod input;
mod launcher;
mod layout;
mod navigation;
mod platform;
mod process;
mod renderer;
mod ui;

/// Run the launcher or its bounded demo child.
///
/// # Errors
/// Returns a contextual diagnostic for invalid arguments or startup/render failures.
pub fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "arguments must be valid UTF-8".to_owned())
        })
        .collect::<Result<_, _>>()?;
    if args.first().is_some_and(|s| s == "--demo-child") {
        return demo_child(&args);
    }
    if args == ["--help"] {
        println!(
            "vitrallis [--pocketchip] [--size WIDTHxHEIGHT] [--screenshot NEW.bmp] [--smoke-test]\nDefault: desktop demo. Arrows select; Enter/tap opens; Escape/Home clears status. Close window to quit.\n--screenshot saves the first frame, then exits; --smoke-test exercises a demo child and exits."
        );
        return Ok(());
    }
    let config = config::Config::parse(args.into_iter())?;
    if config.pocketchip {
        ui::run(&platform::pocketchip::PocketChip, &config)
    } else {
        ui::run(&platform::generic::Generic, &config)
    }
}
fn demo_child(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err("demo child requires exactly one mode".into());
    }
    match args.get(1).map(String::as_str) {
        Some("ok") => std::thread::sleep(std::time::Duration::from_secs(2)),
        Some("quick") => {}
        Some("fail") => return Err("intentional demo child failure".into()),
        _ => return Err("invalid demo child mode".into()),
    }
    Ok(())
}
