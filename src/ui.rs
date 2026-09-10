use crate::{
    config::Config,
    input::{Action, PointerInput},
    launcher::{Launcher, Phase},
    layout::Layout,
    platform::Platform,
    process::{self, ProcessSet, Processes},
    renderer::{Screen, artwork, render, screenshot},
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
    state.preferences = catalog.preferences;
    if !catalog.diagnostics.is_empty() {
        state.status = format!("{} APP WARNINGS - SEE LOG", catalog.diagnostics.len());
        if state.apps.is_empty() {
            state.status = "NO APPS - CHECK CONFIG / LOG".into();
        }
    }
    let sdl = sdl2::init().map_err(|e| format!("SDL init: {e}"))?;
    let video = sdl.video().map_err(|e| format!("SDL video: {e}"))?;
    sdl.mouse().show_cursor(state.preferences.show_cursor);
    sdl2::hint::set("SDL_TOUCH_MOUSE_EVENTS", "0");
    sdl2::hint::set("SDL_MOUSE_TOUCH_EVENTS", "0");
    // A touch used to focus the launcher must also deliver its matching press.
    // SDL otherwise consumes the first click after window activation.
    sdl2::hint::set("SDL_MOUSE_FOCUS_CLICKTHROUGH", "1");
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
    if let Some(path) = &config.screenshot {
        let creator = canvas.texture_creator();
        let textures = artwork(&creator, &state);
        render(&mut canvas, &layout, &state, &textures)?;
        screenshot(&canvas, path)?;
        return Ok(());
    }
    eprintln!(
        "level=info event=ready width={} height={} apps={}",
        layout.width,
        layout.height,
        state.apps.len()
    );
    event_loop(&sdl, &mut canvas, &layout, state, platform, config)
}

fn event_loop(
    sdl: &sdl2::Sdl,
    canvas: &mut Screen,
    layout: &Layout,
    mut state: Launcher,
    platform: &impl Platform,
    config: &Config,
) -> Result<(), String> {
    let smoke = config.mode == crate::config::Mode::Smoke;
    let creator = canvas.texture_creator();
    let mut textures = artwork(&creator, &state);
    let mut worker = system_worker(platform, &mut state);
    let mut events = sdl.event_pump()?;
    let mut child = ProcessSet::<crate::process::NativeProcess>::default();
    let mut pointer = PointerInput::default();
    let mut accept_after = Instant::now();
    let mut dirty = true;
    let mut last_wait_error = None;
    let mut next_poll = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(10);
    if smoke {
        inject_activation(sdl, canvas)?;
    }
    loop {
        dirty |= refresh_system(&mut worker, &mut state.settings);
        if dirty {
            state.running = child.running_ids();
            render(canvas, layout, &state, &textures)?;
            canvas.present();
            dirty = false;
        }
        let event = wait_event(&mut events, state.phase, next_poll, smoke);
        if let Some(event) = event {
            if closing(&event) {
                return Ok(());
            }
            dirty |= exposed(&event);
            dirty |= window_focus(&event, &mut state, &mut pointer);
            if Instant::now() >= accept_after {
                let (action, system_changed) =
                    translate_action(&event, layout, &mut state, &mut pointer, &mut worker);
                dirty |= system_changed;
                let activating = state.phase == Phase::Ready
                    && matches!(
                        action,
                        Some(Action::Activate | Action::SelectAndActivate(_))
                    );
                dirty |= handle_action(action, canvas, layout, &mut state, &textures, &mut child)?;
                if activating || system_changed {
                    pointer.clear();
                    accept_after = Instant::now() + Duration::from_millis(400);
                }
            } else {
                pointer.clear();
            }
        }
        let result = if child.has_children() && Instant::now() >= next_poll {
            next_poll = Instant::now() + Duration::from_millis(250);
            child.poll()
        } else {
            Ok(None)
        };
        match result {
            Ok(Some(status)) => {
                eprintln!("level=info event=app_exited status={status:?}");
                let raise = child.exited_active && state.phase == Phase::Running;
                if child.exited_active {
                    state.finished(if status.success() {
                        "APP CLOSED - READY".into()
                    } else {
                        format!("APP EXITED: {status}")
                    });
                }
                if state.phase == Phase::Ready && reload_catalog(config, &mut state) {
                    textures = artwork(&creator, &state);
                    sdl.mouse().show_cursor(state.preferences.show_cursor);
                }
                last_wait_error = None;
                pointer.clear();
                accept_after = Instant::now() + Duration::from_millis(400);
                if raise && platform.raise_after_exit() {
                    canvas.window_mut().raise();
                }
                dirty = true;
                if smoke {
                    return finish_smoke(canvas, layout, &state, &textures, status);
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

fn window_focus(event: &Event, state: &mut Launcher, pointer: &mut PointerInput) -> bool {
    match event {
        Event::Window {
            win_event: WindowEvent::FocusLost,
            ..
        } => {
            state.settings.cancel();
            pointer.clear();
            true
        }
        Event::Window {
            win_event: WindowEvent::FocusGained,
            ..
        } => {
            state.returned_home();
            pointer.clear();
            true
        }
        _ => false,
    }
}

fn system_worker(
    platform: &impl Platform,
    state: &mut Launcher,
) -> Option<crate::platform::system::Worker> {
    match crate::platform::system::Worker::start(*platform) {
        Ok(worker) => Some(worker),
        Err(error) => {
            state.settings.message = error;
            None
        }
    }
}

fn reload_catalog(config: &Config, state: &mut Launcher) -> bool {
    let result = crate::discovery::refresh(config).and_then(|catalog| {
        if catalog.apps.is_empty() && !catalog.diagnostics.is_empty() {
            return Err("catalogue unavailable; keeping previous apps".into());
        }
        let changed = state.reload(catalog.apps)?;
        let appearance_changed = state.preferences != catalog.preferences;
        state.preferences = catalog.preferences;
        Ok(changed || appearance_changed)
    });
    result.unwrap_or_else(|error| {
        eprintln!("level=warn event=reload_failed message={error:?}");
        state.status = error;
        false
    })
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
    if state.phase == Phase::Running
        && (matches!(action, Action::Activate)
            || matches!(action, Action::SelectAndActivate(index) if index == state.selected))
    {
        state.status = child
            .focus()
            .map_or_else(|error| error, |()| "APP RESUMED".into());
        return Ok(true);
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
