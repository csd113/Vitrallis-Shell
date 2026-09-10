//! Portable PocketHome-compatible launcher. Core tests run without initializing a display.
mod app;
mod config;
mod discovery;
mod input;
mod launcher;
mod layout;
mod navigation;
mod platform;
mod preferences;
mod process;
mod renderer;
mod settings;
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
    if args.first().is_some_and(|s| s == "--set-timezone") {
        if args.len() != 2 {
            return Err("Time zone helper requires one zone".into());
        }
        return platform::pocketchip::authenticate_timezone(&args[1]);
    }
    if args == ["--help"] {
        println!(
            "vitrallis [--pocketchip] [--app-config FILE] [--assets DIR] [--list-apps] [--demo] [--size WIDTHxHEIGHT] [--screenshot NEW.bmp] [--smoke-test]\nDefault: PocketHome metadata in a desktop window. --demo enables fixtures. Arrows select; Enter/tap opens; settings tile/footer opens system controls; Escape/Home goes back. Close window to quit.\n--screenshot saves the first frame, then exits; --smoke-test exercises a demo child and exits."
        );
        return Ok(());
    }
    if args == ["--version"] {
        println!("vitrallis {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let config = config::Config::parse(args.into_iter())?;
    if config.mode == crate::config::Mode::List {
        discovery::print(&discovery::load(&config)?);
        return Ok(());
    }
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

#[cfg(test)]
mod test_support;
