//! Opt-in, test-only workload measurements. No production counters or timers.
use super::*;
use std::{cell::Cell, time::Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub frames: u64,
    pub last_frame: Option<Instant>,
    pub glyphs: u64,
    pub text_operations: u64,
    pub decodes: u64,
    pub uploads: u64,
}
pub fn reset() {
    COUNTS.with(|cell| cell.set(Counts::default()));
}
pub fn snapshot() -> Counts {
    COUNTS.with(Cell::get)
}
thread_local! { static COUNTS: Cell<Counts> = Cell::default(); }
pub fn count(update: impl FnOnce(&mut Counts)) {
    COUNTS.with(|cell| {
        let mut counts = cell.get();
        update(&mut counts);
        cell.set(counts);
    });
}

#[test]
#[ignore = "opt-in rendering benchmark; no timing threshold"]
fn rendering_workloads() -> Result<(), String> {
    sdl2::hint::set("SDL_VIDEODRIVER", "dummy");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let window = video
        .window("render benchmark", 480, 272)
        .hidden()
        .build()
        .map_err(|e| e.to_string())?;
    let canvas = window
        .into_canvas()
        .software()
        .build()
        .map_err(|e| e.to_string())?;
    let creator = canvas.texture_creator();
    let mut canvas = Screen::new(canvas, &creator)?;
    let layout = Layout::home(480, 272)?;
    for (name, mut sample) in samples(&layout)? {
        let textures = artwork(&creator, &sample);
        render(&mut canvas, &layout, &sample, &textures)?;
        if let Some(directory) = std::env::var_os("VITRALLIS_PERF_QA_DIR") {
            screenshot(
                &canvas,
                &std::path::PathBuf::from(directory).join(format!("{name}.bmp")),
            )?;
        }
        COUNTS.with(|cell| cell.set(Counts::default()));
        let start = Instant::now();
        for frame in 0..200 {
            if name == "rapid-keyboard" {
                sample.input(crate::input::Action::Move(
                    crate::navigation::Direction::Right,
                ));
            }
            if name == "rapid-pointer" {
                sample.phase = crate::launcher::Phase::Ready;
                sample.opening = None;
                sample.input(crate::input::Action::SelectAndActivate(
                    frame % sample.apps.len(),
                ));
            }

            render(&mut canvas, &layout, &sample, &textures)?;
            canvas.present();
        }
        let elapsed = start.elapsed();
        eprintln!(
            "workload={name} frames=200 elapsed_us={} counts={:?}",
            elapsed.as_micros(),
            COUNTS.with(Cell::get)
        );
    }
    let icon = [255; 32 * 32 * 4];
    let bounds = Rect {
        x: 0,
        y: 0,
        w: 32,
        h: 32,
    };
    app_center::draw_icon(&mut canvas, &icon, bounds)?;
    reset();
    let start = Instant::now();
    for _ in 0..200 {
        app_center::draw_icon(&mut canvas, &icon, bounds)?;
        canvas.present();
    }
    eprintln!(
        "workload=cached-app-center-icon frames=200 elapsed_us={} counts={:?}",
        start.elapsed().as_micros(),
        snapshot()
    );
    assert_eq!(snapshot().uploads, 0);
    Ok(())
}

fn samples(layout: &Layout) -> Result<Vec<(String, Launcher)>, String> {
    let apps = crate::platform::generic::demo_apps(std::path::Path::new("/fixture/vitrallis"));
    let make_state = || Launcher::new(apps.clone(), layout.columns, layout.tiles.len());
    let mut state = make_state()?;
    let mut samples = vec![("home".to_owned(), make_state()?)];
    state.settings.show();
    samples.push(("settings".into(), state));
    let mut state = make_state()?;
    state.opening = Some("Example application".into());
    samples.push(("modal".into(), state));
    for (name, center) in crate::app_center::Center::qa_samples()? {
        let mut sample = make_state()?;
        sample.app_center = center;
        samples.push((format!("app-center-{name}"), sample));
    }
    for (name, desktop) in crate::shortcuts::screen::Desktop::qa_samples(layout) {
        let mut sample = make_state()?;
        sample.desktop = desktop;
        samples.push((format!("desktop-{name}"), sample));
    }
    let many_apps = (0..1000)
        .map(|index| {
            let mut app = apps[index % apps.len()].clone();
            app.id = format!("fixture-{index}");
            app.icon = Some(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/system/apps.png"),
            );
            app
        })
        .collect();
    samples.push((
        "many-icons".into(),
        Launcher::new(many_apps, layout.columns, layout.tiles.len())?,
    ));
    let mut wallpaper = make_state()?;
    wallpaper.preferences.wallpaper =
        Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/system/apps.png"));
    samples.push(("wallpaper".into(), wallpaper));
    samples.push(("rapid-keyboard".into(), make_state()?));
    samples.push(("rapid-pointer".into(), make_state()?));
    Ok(samples)
}

/// Physical GPU readback coverage for production scenes, including wallpaper
/// (which currently has no user-facing configuration control).
#[test]
#[ignore = "requires a real accelerated backend; run on each physical target separately"]
fn hardware_scenes_match_software() -> Result<(), String> {
    let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
    let icon = scratch.0.join("artwork.png");
    std::fs::write(&icon, include_bytes!("../../assets/system/apps.png"))
        .map_err(|e| e.to_string())?;
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let layout = Layout::home(480, 272)?;
    let mut references = Vec::new();
    let mut mismatches = Vec::new();
    for mode in [
        backend::RendererMode::Software,
        backend::RendererMode::Hardware,
    ] {
        let (canvas, info) = backend::initialize(&video, mode, || {
            video
                .window("Vitrallis scene validation", 480, 272)
                .hidden()
                .build()
                .map_err(|e| e.to_string())
        })?;
        eprintln!("{info}");
        let creator = canvas.texture_creator();
        let mut canvas = Screen::new(canvas, &creator)?;
        for (index, (name, mut state)) in samples(&layout)?.into_iter().enumerate() {
            // Fixtures use compile-host paths; materialize embedded bytes on the
            // actual test host so missing files cannot silently skip artwork.
            for app in &mut state.apps {
                if app.icon.is_some() {
                    app.icon = Some(icon.clone());
                }
            }
            if state.preferences.wallpaper.is_some() {
                state.preferences.wallpaper = Some(icon.clone());
            }
            let textures = artwork(&creator, &state);
            if state.preferences.wallpaper.is_some() {
                assert!(
                    textures.get(state.apps.len()).is_some_and(Option::is_some),
                    "wallpaper must actually upload"
                );
            }
            if name == "many-icons" {
                assert!(
                    textures
                        .iter()
                        .take(layout.tiles.len())
                        .all(Option::is_some),
                    "visible icons must actually upload"
                );
            }
            render(&mut canvas, &layout, &state, &textures)?;
            let pixels = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
            if mode == backend::RendererMode::Software {
                references.push(pixels);
            } else {
                let reference = references.get(index).ok_or("missing software reference")?;
                assert_eq!(pixels.len(), reference.len());
                let different = pixels
                    .iter()
                    .zip(reference)
                    .filter(|(a, b)| a.abs_diff(**b) > 2)
                    .count();
                // SDL software and Lima linear filtering differ by up to 6/255
                // on this densely scaled icon fixture (measured physical output).
                // Bound every channel and the total error; do not permit shifts,
                // missing icons or arbitrary percentages of corrupted pixels.
                let valid = if name == "many-icons" {
                    let errors = pixels.iter().zip(reference).map(|(a, b)| a.abs_diff(*b));
                    errors.clone().all(|error| error <= 6)
                        && errors.map(usize::from).sum::<usize>() <= pixels.len() / 5
                } else {
                    different < pixels.len() / 100
                };
                if !valid {
                    mismatches.push(format!("{name}: {different} differing channels"));
                }
                eprintln!(
                    "scene={name} different_channels={different} total_channels={}",
                    pixels.len()
                );
            }
            if let Some(directory) = std::env::var_os("VITRALLIS_PERF_QA_DIR") {
                screenshot(
                    &canvas,
                    &std::path::PathBuf::from(directory)
                        .join(format!("{}-{name}.bmp", mode.as_str())),
                )?;
            }
            canvas.present();
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("; "));
    Ok(())
}
