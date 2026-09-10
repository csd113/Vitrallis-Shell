use crate::{
    config::Config,
    input::{Action, action},
    launcher::{Launcher, Phase},
    layout::Layout,
    platform::Platform,
    process::{self, NativeProcess, Processes},
    renderer::{Screen, icons, render, screenshot},
};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
    render::Texture,
};
use std::time::{Duration, Instant};

pub fn run(platform: &impl Platform, config: &Config) -> Result<(), String> {
    let (width, height) = config.size.unwrap_or_else(|| platform.resolution());
    let layout = Layout::home(width, height)?;
    let state = Launcher::new(platform.apps()?, layout.columns, layout.tiles.len())?;
    let sdl = sdl2::init().map_err(|e| format!("SDL init: {e}"))?;
    let video = sdl.video().map_err(|e| format!("SDL video: {e}"))?;
    sdl2::hint::set("SDL_TOUCH_MOUSE_EVENTS", "0");
    sdl2::hint::set("SDL_MOUSE_TOUCH_EVENTS", "0");
    let mut builder = video.window("Vitrallis", u32::from(width), u32::from(height));
    builder.position_centered();
    if platform.fullscreen() {
        builder.fullscreen_desktop();
    }
    let window = builder.build().map_err(|e| format!("window: {e}"))?;
    let mut canvas = window
        .into_canvas()
        .software()
        .build()
        .map_err(|e| format!("software renderer: {e}"))?;
    let (actual_w, actual_h) = canvas.window().size();
    let layout = Layout::home(
        u16::try_from(actual_w).map_err(|_| "window too wide")?,
        u16::try_from(actual_h).map_err(|_| "window too tall")?,
    )?;
    let creator = canvas.texture_creator();
    let icons = icons(&creator, &state.apps);
    render(&mut canvas, &layout, &state, &icons)?;
    if let Some(path) = &config.screenshot {
        screenshot(&canvas, path)?;
        return Ok(());
    }
    canvas.present();
    eprintln!(
        "level=info event=ready width={} height={} apps={}",
        layout.width,
        layout.height,
        state.apps.len()
    );
    event_loop(
        &sdl,
        &mut canvas,
        &layout,
        state,
        &icons,
        platform,
        config.smoke,
    )
}

fn event_loop(
    sdl: &sdl2::Sdl,
    canvas: &mut Screen,
    layout: &Layout,
    mut state: Launcher,
    icons: &[Option<Texture<'_>>],
    platform: &impl Platform,
    smoke: bool,
) -> Result<(), String> {
    let mut events = sdl.event_pump()?;
    let mut child = NativeProcess::default();
    let mut dirty = false;
    let mut last_wait_error = None;
    let mut next_poll = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(10);
    if smoke {
        let event = Event::KeyDown {
            timestamp: 0,
            window_id: canvas.window().id(),
            keycode: Some(Keycode::Return),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        };
        sdl.event()?.push_event(event)?;
    }
    loop {
        if dirty {
            render(canvas, layout, &state, icons)?;
            canvas.present();
            dirty = false;
        }
        let event = if state.phase == Phase::Running {
            let wait = next_poll
                .saturating_duration_since(Instant::now())
                .as_millis();
            events.wait_event_timeout(u32::try_from(wait).unwrap_or(250).clamp(1, 250))
        } else if smoke {
            events.wait_event_timeout(250)
        } else {
            Some(events.wait_event())
        };
        if let Some(event) = event {
            if matches!(
                event,
                Event::Quit { .. }
                    | Event::Window {
                        win_event: WindowEvent::Close,
                        ..
                    }
            ) {
                return Ok(());
            }
            if matches!(
                event,
                Event::Window {
                    win_event: WindowEvent::Exposed | WindowEvent::Shown | WindowEvent::Restored,
                    ..
                }
            ) {
                dirty = true;
            }
            dirty |= handle_action(&event, canvas, layout, &mut state, icons, &mut child)?;
        }
        let result = if state.phase == Phase::Running && Instant::now() >= next_poll {
            next_poll = Instant::now() + Duration::from_millis(250);
            child.poll()
        } else {
            Ok(None)
        };
        match result {
            Ok(Some(status)) => {
                eprintln!("level=info event=app_exited status={status:?}");
                state.finished(if status.success() {
                    "APP CLOSED - READY".into()
                } else {
                    format!("APP EXITED: {status}")
                });
                last_wait_error = None;
                if platform.raise_after_exit() {
                    canvas.window_mut().raise();
                }
                dirty = true;
                if smoke {
                    render(canvas, layout, &state, icons)?;
                    canvas.present();
                    if !status.success() {
                        return Err("smoke child failed".into());
                    }
                    eprintln!("level=info event=smoke_passed");
                    return Ok(());
                }
            }
            Ok(None) => {}
            Err(error) => {
                // Keep ownership and block new launches until this child can be reaped.
                if last_wait_error.as_ref() != Some(&error) {
                    eprintln!("level=error event=wait_failed message={error:?}");
                    state.status.clone_from(&error);
                    last_wait_error = Some(error);
                    dirty = true;
                }
            }
        }
        if smoke && Instant::now() >= deadline {
            return Err("smoke test timed out".into());
        }
    }
}

fn handle_action(
    event: &Event,
    canvas: &mut Screen,
    layout: &Layout,
    state: &mut Launcher,
    icons: &[Option<Texture<'_>>],
    child: &mut impl Processes,
) -> Result<bool, String> {
    let Some(action) = action(event, layout, state.apps.len()) else {
        return Ok(false);
    };
    let before = (state.selected, state.phase);
    let was_ready = state.phase == Phase::Ready;
    if let Some(index) = state.input(action) {
        // Present transition feedback before process creation.
        render(canvas, layout, state, icons)?;
        canvas.present();
        process::activate(state, child, index);
    }
    Ok(before != (state.selected, state.phase)
        || was_ready
            && matches!(
                action,
                Action::Back | Action::Activate | Action::SelectAndActivate(_)
            ))
}
