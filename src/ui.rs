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
    sdl2::hint::set("SDL_VIDEO_ALLOW_SCREENSAVER", "1");
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
    let broker = crate::native::broker(&mut state)?;
    let mut pointer = PointerInput::default();
    let mut accept_after = Instant::now();
    let mut dirty = false;
    let mut next_frame = Instant::now();
    let mut last_wait_error = None;
    let mut next_poll = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(10);
    if smoke {
        inject_activation(sdl, canvas)?;
    }
    present_initial(canvas, layout, &state, &textures)?;
    loop {
        dirty |= refresh_system(&mut worker, &mut state.settings);
        if refresh_app_center(sdl, config, &mut state, &mut dirty)? {
            textures = artwork(&creator, &state);
            dirty = true;
        }
        dirty |= refresh_shell(&mut state, &mut child);
        dirty |= launch_from_center(canvas, layout, &mut state, &textures, &mut child)?;
        dirty |= open_native(&broker, canvas, layout, &mut state, &textures, &mut child)?;
        if dirty && Instant::now() >= next_frame {
            state.running = child.running_ids();
            render(canvas, layout, &state, &textures)?;
            canvas.present();
            dirty = submit_power_after_present(&mut worker, &mut state.settings);
            next_frame = Instant::now() + Duration::from_millis(16);
        }
        let event = wait_event(&mut events, state.phase, next_poll, dirty);
        if let Some(event) = event {
            if closing(&event) && !state.app_center.busy {
                return Ok(());
            }
            dirty |= exposed(&event);
            dirty |= window_focus(&event, &mut state, &mut pointer, &mut accept_after);
            if Instant::now() >= accept_after {
                dirty |= terminate_selected(&event, &mut state, &mut child);
                let (action, system_changed) =
                    translate_action(&event, layout, &mut state, &mut pointer, &mut worker);
                dirty |= system_changed;
                let activating = state.phase == Phase::Ready
                    && matches!(
                        action,
                        Some(Action::Activate | Action::SelectAndActivate(_))
                    );
                dirty |= handle_action(action, canvas, layout, &mut state, &textures, &mut child)?;
                dirty |= open_requested(&mut state, &mut child);
                if activating && state.phase == Phase::Running {
                    pointer.clear();
                    accept_after = Instant::now() + Duration::from_millis(400);
                }
            } else {
                pointer.clear();
            }
        }
        let result = poll_children(&mut child, &mut next_poll);
        match result {
            Ok(Some(status)) => {
                eprintln!("level=info event=app_exited status={status:?}");
                refresh_utility(&mut state, &mut worker, child.exited_active);
                let raise = app_exited(&mut state, status, child.exited_active);
                let catalog_changed = refresh_exit_catalog(sdl, config, &mut state, &mut pointer);
                if catalog_changed {
                    textures = artwork(&creator, &state);
                }
                last_wait_error = None;
                if child.exited_active {
                    accept_after = Instant::now();
                }
                if raise && platform.raise_after_exit() {
                    canvas.window_mut().raise();
                    crate::platform::restore_shell_focus();
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

fn refresh_exit_catalog(
    sdl: &sdl2::Sdl,
    config: &Config,
    state: &mut Launcher,
    pointer: &mut PointerInput,
) -> bool {
    let changed = state.phase == Phase::Ready && reload_catalog(config, state);
    if changed {
        sdl.mouse().show_cursor(state.preferences.show_cursor);
        pointer.clear();
    }
    changed
}

fn open_native(
    broker: &vitrallis_native::ipc::Broker,
    canvas: &mut Screen,
    layout: &Layout,
    state: &mut Launcher,
    icons: &[Option<Texture<'_>>],
    child: &mut ProcessSet,
) -> Result<bool, String> {
    match crate::native::requested(broker, state) {
        Ok(Some(index)) => {
            render(canvas, layout, state, icons)?;
            canvas.present();
            process::activate(state, child, index);
            state.apps[index].manifest.args.clear();
            Ok(true)
        }
        Ok(None) => Ok(false),
        Err(error) => {
            state.failed(error);
            Ok(true)
        }
    }
}
fn poll_children(
    child: &mut ProcessSet,
    next_poll: &mut Instant,
) -> Result<Option<std::process::ExitStatus>, String> {
    if child.has_children() && Instant::now() >= *next_poll {
        *next_poll = Instant::now() + Duration::from_millis(250);
        child.poll()
    } else {
        Ok(None)
    }
}

fn refresh_shell(state: &mut Launcher, child: &mut ProcessSet) -> bool {
    let dirty = refresh_timezone(state, child) | refresh_focus(child, state);
    state.settings.updater.relaunch_if_requested(
        state.app_center.busy || child.has_children() || state.settings.pending,
    ) || dirty
}

fn refresh_timezone(state: &mut Launcher, child: &mut ProcessSet) -> bool {
    if let crate::settings::TimezoneState::Authentication(index) = state.settings.timezone {
        state.settings.timezone = crate::settings::TimezoneState::Idle;
        open_timezone(state, child, index);
        true
    } else {
        false
    }
}

fn launch_from_center(
    canvas: &mut Screen,
    layout: &Layout,
    state: &mut Launcher,
    textures: &[Option<sdl2::render::Texture<'_>>],
    child: &mut ProcessSet,
) -> Result<bool, String> {
    let Some(id) = state.app_center.launch.take() else {
        return Ok(false);
    };
    if let Some(index) = state.apps.iter().position(|app| app.id == id) {
        state.app_center.open = false;
        handle_action(
            Some(Action::SelectAndActivate(index)),
            canvas,
            layout,
            state,
            textures,
            child,
        )
    } else {
        state.app_center.message =
            "App is no longer installed; reopen App Center to check local state".into();
        Ok(true)
    }
}

fn refresh_app_center(
    sdl: &sdl2::Sdl,
    config: &Config,
    state: &mut Launcher,
    dirty: &mut bool,
) -> Result<bool, String> {
    *dirty |= state.app_center.poll();
    let artwork_changed = if state.app_center.refresh && state.phase == Phase::Ready {
        match crate::app_center::refresh_apps(&state.apps).and_then(|mut apps| {
            if config.linux_handheld {
                use crate::platform::Platform;
                for app in &mut apps {
                    crate::platform::linux_handheld::LinuxHandheld.prepare_app(app);
                }
            }
            state.reload(apps)
        }) {
            Ok(_) => {
                state.app_center.refresh = false;
                *dirty = true;
                true
            }
            Err(error) => {
                state.app_center.refresh = false;
                state.app_center.message = format!("Installed apps could not refresh: {error}");
                eprintln!("level=warn event=app_center_menu_refresh error={error:?}");
                false
            }
        }
    } else {
        false
    };
    let input = sdl.video()?.text_input();
    if state.app_center.editing() && !input.is_active() {
        input.start();
    } else if !state.app_center.editing() && input.is_active() {
        input.stop();
    }
    Ok(artwork_changed)
}

fn terminate_selected(event: &Event, state: &mut Launcher, child: &mut ProcessSet) -> bool {
    if state.phase != Phase::Ready
        || state.settings.open
        || state.app_center.open
        || state.error.is_some()
        || !matches!(
            event,
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                repeat: false,
                ..
            }
        )
    {
        return false;
    }
    let Some(app) = state.apps.get(state.selected) else {
        return false;
    };
    if !child.terminate(&app.id) {
        return false;
    }
    state.running = child.running_ids();
    state.finished("APP CLOSED - READY".into());
    true
}

fn refresh_utility(
    state: &mut Launcher,
    worker: &mut Option<crate::platform::system::Worker>,
    active: bool,
) {
    if active && state.settings.network == crate::settings::NetworkState::TimezoneOpen {
        submit_setting(
            Some(crate::settings::Request::Control(
                crate::platform::system::Control::ReadTimezone,
            )),
            worker,
            &mut state.settings,
        );
    }
}

fn app_exited(state: &mut Launcher, status: std::process::ExitStatus, active: bool) -> bool {
    let raise = active && state.phase == Phase::Running;
    if active {
        state.finished(if status.success() {
            "APP CLOSED - READY".into()
        } else {
            format!("APP EXITED: {status}")
        });
    }
    if active && state.settings.network == crate::settings::NetworkState::Open {
        state.settings.network = crate::settings::NetworkState::Idle;
        state.settings.show();
    }
    if active
        && matches!(
            state.settings.network,
            crate::settings::NetworkState::CalibrationOpen
                | crate::settings::NetworkState::TimezoneOpen
        )
    {
        state.settings.network = crate::settings::NetworkState::Idle;
        state.settings.show();
        state.settings.page(crate::settings::Page::Device);
    }
    raise
}

fn present_initial(
    canvas: &mut Screen,
    layout: &Layout,
    state: &Launcher,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    render(canvas, layout, state, textures)?;
    canvas.present();
    canvas.window_mut().raise();
    eprintln!(
        "level=info event=ready width={} height={} apps={}",
        layout.width,
        layout.height,
        state.apps.len()
    );
    Ok(())
}

fn refresh_focus(child: &mut impl Processes, state: &mut Launcher) -> bool {
    match child.poll_focus() {
        Ok(Some(crate::platform::FocusResult::Focused)) => state.status = "APP OPENED".into(),
        Err(error) => state.failed(error),
        Ok(Some(crate::platform::FocusResult::Missing)) => {
            state.finished("Still starting - select the app to retry".into());
        }
        Ok(None) => return false,
    }
    true
}

fn window_focus(
    event: &Event,
    state: &mut Launcher,
    pointer: &mut PointerInput,
    accept_after: &mut Instant,
) -> bool {
    match event {
        Event::Window {
            win_event: WindowEvent::FocusLost,
            ..
        } => {
            state.opening = None;
            state.settings.lost_focus();
            state.app_center.lost_focus();
            pointer.clear();
            true
        }
        Event::Window {
            win_event: WindowEvent::FocusGained,
            ..
        } => {
            state.returned_home();
            pointer.clear();
            *accept_after = Instant::now();
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
    let result = crate::discovery::refresh(config).and_then(|mut catalog| {
        crate::native::inherit_runtime(&mut catalog.apps, &state.apps);
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
    if state.settings.power_transition.is_some() {
        return (None, false);
    }
    if state.app_center.open {
        state.app_center.event(event, layout);
        return (None, true);
    }
    state.settings.network_available = state
        .apps
        .iter()
        .any(|app| app.is_system_settings() && app.unavailable.is_none());
    if state.settings.open {
        let request = state.settings.event(event, layout);
        submit_setting(request, worker, &mut state.settings);
        return (None, true);
    }
    let action = pointer.action(event, layout, state.visible_count());
    if action == Some(Action::System) {
        state.settings.show();
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
    dirty |= settings.updater.poll();
    if let Some(worker) = worker {
        if let Some(update) = worker.update() {
            settings.status = update.status;
            if let Some(result) = update.result {
                if result.is_err() {
                    settings.power_transition = None;
                }
                settings.applying = None;
                let previous = std::mem::take(&mut settings.timezone);
                if let crate::settings::TimezoneState::Applying(index) = previous
                    && result.is_err()
                {
                    settings.timezone = crate::settings::TimezoneState::Authentication(index);
                }
                let reading = matches!(previous, crate::settings::TimezoneState::Reading);
                settings.message = result.map_or_else(
                    |error| error,
                    |()| {
                        if reading {
                            "Time zone refreshed".into()
                        } else {
                            "Saved".into()
                        }
                    },
                );
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
    if settings.power_transition.is_none() && worker.as_ref().is_some_and(|worker| !worker.pending)
    {
        for index in 0..2 {
            if let Some(value) = settings.queued[index].take() {
                let command = if index == 0 {
                    crate::platform::system::Control::Brightness(value)
                } else {
                    crate::platform::system::Control::Volume(value)
                };
                submit_setting(
                    Some(crate::settings::Request::Control(command)),
                    worker,
                    settings,
                );
                dirty = true;
                break;
            }
        }
    }
    dirty
}
fn submit_setting(
    request: Option<crate::settings::Request>,
    worker: &mut Option<crate::platform::system::Worker>,
    settings: &mut crate::settings::Settings,
) {
    if settings.power_transition.is_some() {
        return;
    }
    if let Some(crate::settings::Request::Control(crate::platform::system::Control::Power(power))) =
        request
    {
        settings.power_transition = Some(crate::settings::PowerTransition::Requested(power));
        settings.queued = [None; 2];
        return;
    }
    match request {
        Some(crate::settings::Request::CheckUpdates) => settings.updater.check(),
        Some(crate::settings::Request::InstallUpdate) => settings.updater.install(),
        Some(crate::settings::Request::RelaunchUpdate) => settings.updater.request_relaunch(),
        Some(crate::settings::Request::Calibration) => {
            settings.network = crate::settings::NetworkState::CalibrationRequested;
        }
        Some(crate::settings::Request::Network) => {
            settings.network = crate::settings::NetworkState::Requested;
        }
        Some(crate::settings::Request::Control(command)) => {
            use crate::platform::system::Control;
            let slider = match command {
                Control::Brightness(value) => Some((0, value)),
                Control::Volume(value) => Some((1, value)),
                Control::Power(_)
                | Control::ScreenTimeout(_)
                | Control::Timezone(_)
                | Control::ReadTimezone => None,
            };
            if let Some((index, value)) = slider
                && worker.as_ref().is_some_and(|worker| worker.pending)
            {
                settings.queued[index] = Some(value);
                return;
            }
            let result = worker
                .as_mut()
                .ok_or_else(|| "System controls unavailable".into())
                .and_then(|worker| worker.submit(command));
            if result.is_ok() {
                match command {
                    Control::Timezone(index) => {
                        settings.timezone = crate::settings::TimezoneState::Applying(index);
                    }
                    Control::ReadTimezone => {
                        settings.timezone = crate::settings::TimezoneState::Reading;
                    }
                    _ => {}
                }
                settings.applying = match command {
                    Control::Brightness(value) => Some((0, value)),
                    Control::Volume(value) => Some((1, value)),
                    Control::Power(_)
                    | Control::ScreenTimeout(_)
                    | Control::Timezone(_)
                    | Control::ReadTimezone => None,
                };
            }
            settings.message = result.map_or_else(|error| error, |()| "Applying...".into());
            settings.pending = worker.as_ref().is_some_and(|worker| worker.pending);
        }
        None => {}
    }
}
// Called only after presenting the acknowledgement frame, before any power command.
fn submit_power_after_present(
    worker: &mut Option<crate::platform::system::Worker>,
    settings: &mut crate::settings::Settings,
) -> bool {
    use crate::{platform::system::Control, settings::PowerTransition};
    let Some(PowerTransition::Requested(power)) = settings.power_transition else {
        return false;
    };
    let result = worker
        .as_mut()
        .ok_or_else(|| "System controls unavailable".to_owned())
        .and_then(|worker| worker.submit(Control::Power(power)));
    match result {
        Ok(()) => {
            settings.power_transition = Some(PowerTransition::Submitted(power));
            settings.pending = true;
            false
        }
        Err(error) => {
            settings.power_transition = None;
            settings.open = true;
            settings.message = error;
            true
        }
    }
}

fn open_timezone(state: &mut Launcher, child: &mut impl Processes, index: usize) {
    let result = state
        .settings
        .status
        .timezones
        .get(index)
        .ok_or_else(|| "Time zone unavailable".to_owned())
        .and_then(|zone| crate::platform::linux_handheld::timezone_app(zone))
        .and_then(|app| child.start(&app));
    match result {
        Ok(()) => {
            state.settings.cancel();
            state.settings.network = crate::settings::NetworkState::TimezoneOpen;
            state.opening = Some("Time zone authentication".into());
            state.started();
        }
        Err(error) => state.settings.message = error,
    }
}
fn open_calibration(state: &mut Launcher, child: &mut impl Processes) {
    state.settings.network = crate::settings::NetworkState::Idle;
    let app = crate::app::AppEntry {
        source: crate::app::AppSource::System,
        id: "vitrallis-touch-calibration".into(),
        name: "Touch calibration".into(),
        icon: None,
        unavailable: None,
        manifest: crate::app::AppManifest {
            entry: crate::platform::linux_handheld::CALIBRATION.into(),
            ..crate::app::AppManifest::default()
        },
    };
    match child.start(&app) {
        Ok(()) => {
            state.settings.cancel();
            state.settings.network = crate::settings::NetworkState::CalibrationOpen;
            state.opening = Some(app.name);
            state.started();
        }
        Err(error) => state.settings.message = error,
    }
}
fn open_network(state: &mut Launcher, child: &mut impl Processes) {
    state.settings.network = crate::settings::NetworkState::Idle;
    let Some(mut app) = state
        .apps
        .iter()
        .find(|app| app.is_system_settings())
        .cloned()
    else {
        state.settings.message = "Wi-Fi connection manager unavailable".into();
        return;
    };
    app.id = "vitrallis-network-manager".into();
    app.name = "Wi-Fi networks".into();
    match child.start(&app) {
        Ok(()) => {
            state.settings.cancel();
            state.settings.network = crate::settings::NetworkState::Open;
            state.opening = Some(app.name.clone());
            state.started();
        }
        Err(error) => state.settings.message = error,
    }
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
        && state.opening.is_none()
        && (matches!(action, Action::Activate)
            || matches!(action, Action::SelectAndActivate(index) if index == state.selected))
    {
        // Route both activation paths through ProcessSet::start, which checks
        // for an exit before deciding whether to resume or launch again.
        state.returned_home();
    }
    let before = (
        state.selected,
        state.phase,
        state.error.is_some(),
        state.settings.open,
    );
    let was_ready = state.phase == Phase::Ready;
    if let Some(index) = state.input(action) {
        // Present transition feedback before process creation.
        render(canvas, layout, state, icons)?;
        canvas.present();
        state.settings.network = crate::settings::NetworkState::Idle;
        process::activate(state, child, index);
    }
    Ok(before
        != (
            state.selected,
            state.phase,
            state.error.is_some(),
            state.settings.open,
        )
        || was_ready
            && matches!(
                action,
                Action::Back | Action::Activate | Action::SelectAndActivate(_)
            ))
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
    dirty: bool,
) -> Option<Event> {
    if dirty {
        return events.wait_event_timeout(16);
    }
    if phase == Phase::Running {
        let wait = next_poll
            .saturating_duration_since(Instant::now())
            .as_millis();
        events.wait_event_timeout(u32::try_from(wait).unwrap_or(250).clamp(1, 250))
    } else {
        events.wait_event_timeout(250)
    }
}

fn open_requested(state: &mut Launcher, child: &mut impl Processes) -> bool {
    match state.settings.network {
        crate::settings::NetworkState::Requested => open_network(state, child),
        crate::settings::NetworkState::CalibrationRequested => open_calibration(state, child),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl2::mouse::MouseButton;
    #[test]
    fn escape_terminates_only_the_highlighted_background_app() -> Result<(), String> {
        let mut apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        for app in &mut apps {
            app.manifest = crate::app::AppManifest {
                entry: "/bin/sh".into(),
                args: vec!["-c".into(), "exec sleep 30".into()],
                ..crate::app::AppManifest::default()
            };
        }
        let mut state = Launcher::new(apps, 3, 6)?;
        let mut child = ProcessSet::default();
        child.start(&state.apps[0])?;
        child.start(&state.apps[1])?;
        state.running = child.running_ids();
        let mut event = Event::KeyDown {
            timestamp: 0,
            window_id: 1,
            keycode: Some(Keycode::Home),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        };
        assert!(!terminate_selected(&event, &mut state, &mut child));
        if let Event::KeyDown {
            keycode, repeat, ..
        } = &mut event
        {
            *keycode = Some(Keycode::Escape);
            *repeat = true;
        }
        assert!(!terminate_selected(&event, &mut state, &mut child));
        if let Event::KeyDown { repeat, .. } = &mut event {
            *repeat = false;
        }
        state.settings.open = true;
        assert!(!terminate_selected(&event, &mut state, &mut child));
        state.settings.open = false;
        state.error = Some("dismiss first".into());
        assert!(!terminate_selected(&event, &mut state, &mut child));
        state.error = None;
        state.started();
        assert!(!terminate_selected(&event, &mut state, &mut child));
        state.returned_home();
        state.selected = 2;
        assert!(!terminate_selected(&event, &mut state, &mut child));
        state.selected = 0;
        assert_eq!(child.running_ids().len(), 2);

        assert!(terminate_selected(&event, &mut state, &mut child));
        assert_eq!(state.running, [state.apps[1].id.clone()]);
        assert_eq!(child.running_ids(), state.running);
        assert_eq!(state.selected, 0);
        assert_eq!(state.phase, Phase::Ready);
        assert!(!terminate_selected(&event, &mut state, &mut child));
        Ok(())
    }
    #[test]
    fn pending_slider_input_coalesces_to_the_latest_value() -> Result<(), String> {
        use crate::platform::system::{Control, Percent, Status, System, Worker};
        use crate::settings::{Request, Settings};
        use std::sync::mpsc;
        #[derive(Clone)]
        struct Backend(mpsc::Sender<Control>);
        impl System for Backend {
            fn refresh(&mut self) -> Status {
                Status::default()
            }
            fn control(&mut self, command: Control, _: &mut Status) -> Result<(), String> {
                self.0.send(command).map_err(|error| error.to_string())
            }
        }
        let (send, received) = mpsc::channel();
        let mut worker = Some(Worker::start(Backend(send))?);
        let mut settings = Settings::default();
        for value in [20, 30, 80] {
            submit_setting(
                Some(Request::Control(Control::Brightness(Percent::new(value)?))),
                &mut worker,
                &mut settings,
            );
        }
        assert_eq!(settings.value(0), Some(Percent::new(80)?));
        assert_eq!(
            received
                .recv_timeout(Duration::from_secs(3))
                .map_err(|error| error.to_string())?,
            Control::Brightness(Percent::new(20)?)
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while worker.as_ref().is_some_and(|worker| worker.pending)
            || settings.queued.iter().any(Option::is_some)
        {
            refresh_system(&mut worker, &mut settings);
            if Instant::now() >= deadline {
                return Err("slider worker timeout".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            received
                .recv_timeout(Duration::from_secs(3))
                .map_err(|error| error.to_string())?,
            Control::Brightness(Percent::new(80)?)
        );
        assert!(received.try_recv().is_err());
        Ok(())
    }
    #[test]
    fn loading_survives_spawn_and_missing_window_is_nonfatal() -> Result<(), String> {
        struct Missing;
        impl Processes for Missing {
            fn start(&mut self, _: &crate::app::AppEntry) -> Result<(), String> {
                Ok(())
            }
            fn poll(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
                Ok(None)
            }
            fn poll_focus(&mut self) -> Result<Option<crate::platform::FocusResult>, String> {
                Ok(Some(crate::platform::FocusResult::Missing))
            }
        }
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let mut state = Launcher::new(apps, 3, 6)?;
        state.input(Action::Activate);
        state.started();
        assert!(state.opening.is_some());
        refresh_focus(&mut Missing, &mut state);
        assert!(state.error.is_none());
        assert!(state.opening.is_none());
        assert_eq!(state.phase, Phase::Ready);
        Ok(())
    }
    #[test]
    fn late_focus_loss_preserves_network_return_but_cancels_power_confirmation()
    -> Result<(), String> {
        use std::os::unix::process::ExitStatusExt;
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let mut state = Launcher::new(apps, 3, 6)?;
        state.started();
        state.settings.network = crate::settings::NetworkState::Open;
        app_exited(&mut state, std::process::ExitStatus::from_raw(0), true);
        let mut pointer = PointerInput::default();
        let mut accept_after = Instant::now();
        let event = Event::Window {
            timestamp: 0,
            window_id: 1,
            win_event: WindowEvent::FocusLost,
        };
        window_focus(&event, &mut state, &mut pointer, &mut accept_after);
        assert!(state.settings.open);
        assert_eq!(state.settings.network, crate::settings::NetworkState::Idle);
        state.settings.status.power_controls = true;
        state.settings.input(Action::SelectAndActivate(4));
        assert!(state.settings.confirmation.is_some());
        window_focus(&event, &mut state, &mut pointer, &mut accept_after);
        assert!(state.settings.open);
        assert!(state.settings.confirmation.is_none());
        assert_eq!(state.settings.selected, 0);
        Ok(())
    }
    #[test]
    fn focus_return_accepts_fresh_input_without_accepting_stale_release() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let mut state = Launcher::new(apps, 3, 6)?;
        state.started();
        let mut pointer = PointerInput::default();
        let mut accept_after = Instant::now() + Duration::from_millis(400);
        let focus = Event::Window {
            timestamp: 0,
            window_id: 1,
            win_event: WindowEvent::FocusGained,
        };
        assert!(window_focus(
            &focus,
            &mut state,
            &mut pointer,
            &mut accept_after
        ));
        assert!(Instant::now() >= accept_after);
        let release = Event::MouseButtonUp {
            timestamp: 0,
            window_id: 1,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 80,
            y: 80,
        };
        assert_eq!(
            pointer.action(&release, &layout, state.visible_count()),
            None
        );
        let press = Event::MouseButtonDown {
            timestamp: 0,
            window_id: 1,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 80,
            y: 80,
        };
        assert_eq!(pointer.action(&press, &layout, state.visible_count()), None);
        let action = pointer
            .action(&release, &layout, state.visible_count())
            .ok_or("lost first tap after app return")?;
        assert_eq!(state.input(action), Some(0));
        Ok(())
    }
}

#[cfg(test)]
mod power_tests {
    use super::*;
    use crate::{
        platform::system::{Control, Power, Status, System, Worker},
        settings::{PowerTransition, Request, Settings},
    };
    use std::sync::mpsc;

    #[derive(Clone)]
    struct Backend {
        commands: mpsc::Sender<Control>,
        fail: bool,
    }
    impl System for Backend {
        fn refresh(&mut self) -> Status {
            Status::default()
        }
        fn control(&mut self, command: Control, _: &mut Status) -> Result<(), String> {
            self.commands
                .send(command)
                .map_err(|error| error.to_string())?;
            if self.fail {
                Err("Power request denied".into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn power_waits_for_splash_and_is_submitted_once_with_failure_recovery() -> Result<(), String> {
        for power in [Power::Reboot, Power::Shutdown] {
            for fail in [false, true] {
                let (send, received) = mpsc::channel();
                let mut worker = Some(Worker::start(Backend {
                    commands: send,
                    fail,
                })?);
                let mut settings = Settings::default();
                settings.show();
                submit_setting(
                    Some(Request::Control(Control::Power(power))),
                    &mut worker,
                    &mut settings,
                );
                assert!(matches!(
                    settings.power_transition,
                    Some(PowerTransition::Requested(_))
                ));
                assert!(received.try_recv().is_err());
                assert!(!submit_power_after_present(&mut worker, &mut settings));
                assert_eq!(
                    received
                        .recv_timeout(Duration::from_secs(3))
                        .map_err(|e| e.to_string())?,
                    Control::Power(power)
                );
                submit_setting(
                    Some(Request::Control(Control::Power(power))),
                    &mut worker,
                    &mut settings,
                );
                assert!(!submit_power_after_present(&mut worker, &mut settings));
                let deadline = Instant::now() + Duration::from_secs(3);
                while settings.pending {
                    refresh_system(&mut worker, &mut settings);
                    if Instant::now() >= deadline {
                        return Err("power worker timeout".into());
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(received.try_recv().is_err());
                assert_eq!(settings.power_transition.is_none(), fail);
                if fail {
                    assert_eq!(settings.message, "Power request denied");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn unavailable_power_controls_restore_settings() {
        let mut settings = Settings::default();
        submit_setting(
            Some(Request::Control(Control::Power(Power::Shutdown))),
            &mut None,
            &mut settings,
        );
        assert!(submit_power_after_present(&mut None, &mut settings));
        assert!(settings.power_transition.is_none());
        assert!(settings.open);
        assert_eq!(settings.message, "System controls unavailable");
    }
}
