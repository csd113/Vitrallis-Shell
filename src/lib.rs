//! Portable Vitrallis application launcher. Core tests run without initializing a display.
mod app;
mod app_center;
mod config;
mod discovery;
mod input;
mod launcher;
mod layout;
mod native;
mod navigation;
mod platform;
mod preferences;
mod process;
mod renderer;
mod settings;
mod shortcuts;
mod ui;
mod updater;

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
        return platform::linux_handheld::authenticate_timezone(&args[1]);
    }
    if args == ["--help"] {
        println!(
            "vitrallis [--linux-handheld] [--app-config FILE] [--assets DIR] [--list-apps] [--demo] [--size WIDTHxHEIGHT] [--renderer auto|hardware|software] [--screenshot NEW.bmp] [--smoke-test] [--graphics-info | --graphics-test]\nDefault: installed Vitrallis apps in a desktop window; --linux-handheld also reads device menu metadata. --demo enables fixtures. Arrows select; Enter/tap opens; settings tile/footer opens system controls; Escape/Home goes back. Close window to quit.\n--renderer defaults to auto (accelerated SDL with software fallback); hardware requires acceleration; software disables it.\n--graphics-info reports the active renderer; --graphics-test checks texture/fill/font readback without loading user data.\n--screenshot saves the first frame, then exits; --smoke-test exercises a demo child and exits."
        );
        return Ok(());
    }
    if args == ["--version"] {
        println!("vitrallis {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let config = config::Config::parse(args.into_iter())?;
    if matches!(
        config.mode,
        config::Mode::GraphicsInfo | config::Mode::GraphicsTest
    ) {
        return graphics_command(&config);
    }
    if config.mode == crate::config::Mode::List {
        discovery::print(&discovery::load(&config)?);
        return Ok(());
    }
    if config.linux_handheld {
        ui::run(&platform::linux_handheld::LinuxHandheld, &config)
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

fn graphics_command(config: &config::Config) -> Result<(), String> {
    use vitrallis_native::renderer::{graphics, initialize};
    let sdl = sdl2::init().map_err(|e| format!("SDL init: {e}"))?;
    let video = sdl
        .video()
        .map_err(|e| format!("SDL video: {e}; check display/session access"))?;
    println!("SDL video: {}", video.current_video_driver());
    let result = initialize(&video, config.renderer, || {
        video
            .window("Vitrallis graphics diagnostics", 64, 32)
            .hidden()
            .build()
            .map_err(|e| e.to_string())
    });
    println!("{}", graphics::capabilities());
    let (mut canvas, info) = result?;
    println!("{}", graphics::report(&info));
    if config.mode == config::Mode::GraphicsTest {
        graphics::self_test(&mut canvas)?;
        println!(
            "Graphics self-test: PASS (texture copy, fill, font atlas, pixel readback, present)"
        );
    }
    Ok(())
}
