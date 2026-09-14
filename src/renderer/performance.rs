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
