use crate::{
    config::Config,
    input::{Action, PointerInput},
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
    let catalog = crate::discovery::load(config)?;
    let mut state = Launcher::new(catalog.apps, layout.columns, layout.tiles.len())?;
    if !catalog.diagnostics.is_empty() {
        state.status = format!("{} APP WARNINGS - SEE LOG", catalog.diagnostics.len());
        if state.apps.is_empty() {
            state.status = "NO APPS - CHECK CONFIG / LOG".into();
        }
    }
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
        config.mode == crate::config::Mode::Smoke,
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
    let mut worker = match crate::platform::system::Worker::start(*platform) {
        Ok(worker) => Some(worker),
        Err(error) => {
            state.settings.message = error;
            None
        }
    };
    let mut events = sdl.event_pump()?;
    let mut child = NativeProcess::default();
    let mut pointer = PointerInput::default();
    let mut accept_after = Instant::now();
    let mut dirty = false;
    let mut last_wait_error = None;
    let mut next_poll = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(10);
    if smoke {
        inject_activation(sdl, canvas)?;
    }
    loop {
        dirty |= refresh_system(&mut worker, &mut state.settings);
        if dirty {
            render(canvas, layout, &state, icons)?;
            canvas.present();
            dirty = false;
        }
        let event = wait_event(&mut events, state.phase, next_poll, smoke);
        if let Some(event) = event {
            if closing(&event) {
                return Ok(());
            }
            dirty |= exposed(&event);
            if matches!(
                event,
                Event::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                }
            ) {
                state.settings.cancel();
                pointer.clear();
                dirty = true;
            }
            if Instant::now() >= accept_after {
                let (action, system_changed) =
                    translate_action(&event, layout, &mut state, &mut pointer, &mut worker);
                dirty |= system_changed;
                let activating = state.phase == Phase::Ready
                    && matches!(
                        action,
                        Some(Action::Activate | Action::SelectAndActivate(_))
                    );
                dirty |= handle_action(action, canvas, layout, &mut state, icons, &mut child)?;
                if activating || system_changed {
                    pointer.clear();
                    accept_after = Instant::now() + Duration::from_millis(400);
                }
            } else {
                pointer.clear();
            }
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
                pointer.clear();
                accept_after = Instant::now() + Duration::from_millis(400);
                if platform.raise_after_exit() {
                    canvas.window_mut().raise();
                }
                dirty = true;
                if smoke {
                    return finish_smoke(canvas, layout, &state, icons, status);
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

const fn closing(event: &Event) -> bool {
    matches!(
        event,
        Event::Quit { .. }
            | Event::Window {
                win_event: WindowEvent::Close,
                ..
            }
    )
}
const fn exposed(event: &Event) -> bool {
    matches!(
        event,
        Event::Window {
            win_event: WindowEvent::Exposed | WindowEvent::Shown | WindowEvent::Restored,
            ..
        }
    )
}
fn translate_action(
    event: &Event,
    layout: &Layout,
    state: &mut Launcher,
    pointer: &mut PointerInput,
    worker: &mut Option<crate::platform::system::Worker>,
) -> (Option<Action>, bool) {
    let count = if state.settings.open {
        if state.settings.confirmation.is_some() {
            2
        } else {
            6
        }
    } else {
        state.visible_count()
    };
    let action = pointer.action(event, layout, count);
    if system_action(action, worker, &mut state.settings) {
        (None, true)
    } else {
        (action, false)
    }
}

fn refresh_system(
    worker: &mut Option<crate::platform::system::Worker>,
    settings: &mut crate::settings::Settings,
) -> bool {
    let mut dirty = settings.expire();
    if let Some(worker) = worker {
        if let Some(update) = worker.update() {
            settings.status = update.status;
            if let Some(result) = update.result {
                settings.message =
                    result.map_or_else(|error| error, |()| "CONTROL APPLIED - ESC:BACK".into());
            }
            dirty = true;
        }
        settings.pending = worker.pending;
        if worker.stale() && settings.status != crate::platform::system::Status::default() {
            settings.status = crate::platform::system::Status::default();
            settings.message = "SYSTEM STATUS STALE".into();
            dirty = true;
        }
    }
    dirty
}
fn system_action(
    action: Option<Action>,
    worker: &mut Option<crate::platform::system::Worker>,
    settings: &mut crate::settings::Settings,
) -> bool {
    let Some(action) = action else {
        return false;
    };
    if !settings.open && action != Action::System {
        return false;
    }
    if let Some(command) = settings.input(action) {
        let result = worker
            .as_mut()
            .ok_or_else(|| "system worker unavailable".into())
            .and_then(|worker| worker.submit(command));
        settings.message = result.map_or_else(|error| error, |()| "APPLYING...".into());
        settings.pending = worker.as_ref().is_some_and(|worker| worker.pending);
    }
    true
}

fn handle_action(
    action: Option<Action>,
    canvas: &mut Screen,
    layout: &Layout,
    state: &mut Launcher,
    icons: &[Option<Texture<'_>>],
    child: &mut impl Processes,
) -> Result<bool, String> {
    let Some(mut action) = action else {
        return Ok(false);
    };
    if let Action::SelectAndActivate(index) = action {
        action = Action::SelectAndActivate(state.page_start() + index);
    }
    let before = (state.selected, state.phase, state.error.is_some());
    let was_ready = state.phase == Phase::Ready;
    if let Some(index) = state.input(action) {
        // Present transition feedback before process creation.
        render(canvas, layout, state, icons)?;
        canvas.present();
        process::activate(state, child, index);
    }
    Ok(
        before != (state.selected, state.phase, state.error.is_some())
            || was_ready
                && matches!(
                    action,
                    Action::Back | Action::Activate | Action::SelectAndActivate(_)
                ),
    )
}

fn inject_activation(sdl: &sdl2::Sdl, canvas: &Screen) -> Result<(), String> {
    let event = Event::KeyDown {
        timestamp: 0,
        window_id: canvas.window().id(),
        keycode: Some(Keycode::Return),
        scancode: None,
        keymod: sdl2::keyboard::Mod::NOMOD,
        repeat: false,
    };
    sdl.event()?.push_event(event)?;
    Ok(())
}

fn finish_smoke(
    canvas: &mut Screen,
    layout: &Layout,
    state: &Launcher,
    icons: &[Option<Texture<'_>>],
    status: std::process::ExitStatus,
) -> Result<(), String> {
    render(canvas, layout, state, icons)?;
    canvas.present();
    if !status.success() {
        return Err("smoke child failed".into());
    }
    eprintln!("level=info event=smoke_passed");
    Ok(())
}

fn wait_event(
    events: &mut sdl2::EventPump,
    phase: Phase,
    next_poll: Instant,
    _smoke: bool,
) -> Option<Event> {
    if phase == Phase::Running {
        let wait = next_poll
            .saturating_duration_since(Instant::now())
            .as_millis();
        events.wait_event_timeout(u32::try_from(wait).unwrap_or(250).clamp(1, 250))
    } else {
        events.wait_event_timeout(250)
    }
}
