use crate::{
    config::Config,
    input::{Action, PointerInput},
    launcher::{Launcher, Phase},
    layout::Layout,
    platform::Platform,
    process::{self, ProcessSet, Processes},
    renderer::{Artwork, Screen, artwork, render, screenshot},
};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
    render::{Texture, TextureCreator},
    video::WindowContext,
};
use std::time::{Duration, Instant};

pub fn run(platform: &impl Platform, config: &Config) -> Result<(), String> {
    let (width, height) = config.size.unwrap_or_else(|| platform.resolution());
    Layout::home(width, height)?;
    sdl2::hint::set("SDL_VIDEO_ALLOW_SCREENSAVER", "1");
    let sdl = sdl2::init().map_err(|e| format!("SDL init: {e}"))?;
    let video = sdl.video().map_err(|e| format!("SDL video: {e}"))?;
    sdl.mouse().show_cursor(false);
    sdl2::hint::set("SDL_TOUCH_MOUSE_EVENTS", "0");
    sdl2::hint::set("SDL_MOUSE_TOUCH_EVENTS", "0");
    // A touch used to focus the launcher must also deliver its matching press.
    // SDL otherwise consumes the first click after window activation.
    sdl2::hint::set("SDL_MOUSE_FOCUS_CLICKTHROUGH", "1");
    let (canvas, info) = crate::renderer::backend::initialize(&video, config.renderer, || {
        let mut window = video.window("Vitrallis", u32::from(width), u32::from(height));
        window.position_centered().hidden();
        if platform.fullscreen() {
            window.fullscreen_desktop();
        }
        window.build().map_err(|error| format!("window: {error}"))
    })?;
    let font_creator = canvas.texture_creator();
    let mut canvas = Screen::new(canvas, &font_creator)?;
    eprintln!("{info}");
    let layout = window_layout(&canvas)?;
    let animated = config.mode == crate::config::Mode::Launch && config.screenshot.is_none();
    let catalog = if config.mode == crate::config::Mode::Launch && config.screenshot.is_none() {
        let Some(catalog) = crate::boot::load(&sdl, &mut canvas, &layout, || {
            crate::discovery::load(config)
        })?
        else {
            return Ok(());
        };
        catalog
    } else {
        crate::discovery::load(config)?
    };
    let boot_frame = if animated {
        Some((
            canvas.output_size()?,
            canvas.read_pixels(None, sdl2::pixels::PixelFormatEnum::RGB24)?,
        ))
    } else {
        None
    };
    // Boot consumes resize events; rebuild hit boxes from the final window size.
    let layout = window_layout(&canvas)?;
    let mut state = Launcher::new(catalog.apps, layout.columns, layout.tiles.len())?;
    state.preferences = catalog.preferences;
    match crate::preferences::Policy::load(state.preferences.ampm) {
        Ok(policy) => state.settings.policy = policy,
        Err(error) => state.status = error,
    }
    state.preferences.ampm = state.settings.policy.ampm;
    match crate::folders::Folders::load() {
        Ok(folders) => state.folders = folders,
        Err(error) => state.status = format!("Folder state unavailable: {error}"),
    }
    state.rebuild_view(None);
    if !catalog.diagnostics.is_empty() {
        state.status = format!("{} APP WARNINGS - SEE LOG", catalog.diagnostics.len());
        if state.apps.is_empty() {
            state.status = "NO APPS - CHECK CONFIG / LOG".into();
        }
    }
    state.renderer_info = Some(info);
    state.settings.renderer = state
        .renderer_info
        .as_ref()
        .map_or_else(String::new, |info| {
            format!("{} {}", info.sdl.name, info.actual.as_str())
        });
    sdl.mouse().show_cursor(state.preferences.show_cursor);
    if let Some(path) = &config.screenshot {
        let creator = canvas.texture_creator();
        let textures = artwork(&creator, &state);
        render(&mut canvas, &layout, &state, &textures)?;
        screenshot(&canvas, path)?;
        return Ok(());
    }
    if let Some(((width, height), pixels)) = boot_frame {
        let creator = canvas.texture_creator();
        let textures = artwork(&creator, &state);
        let mut overlay = creator
            .create_texture_static(sdl2::pixels::PixelFormatEnum::RGB24, width, height)
            .map_err(|e| e.to_string())?;
        overlay
            .update(
                None,
                &pixels,
                usize::try_from(width).map_err(|_| "boot width")? * 3,
            )
            .map_err(|e| e.to_string())?;
        overlay.set_blend_mode(sdl2::render::BlendMode::Blend);
        for opacity in [224, 192, 160, 128, 96, 64, 32, 0] {
            render(&mut canvas, &layout, &state, &textures)?;
            overlay.set_alpha_mod(opacity);
            canvas.copy(&overlay, None, None)?;
            canvas.present();
            std::thread::sleep(Duration::from_millis(16));
        }
    }
    event_loop(&sdl, &mut canvas, &layout, state, platform, config)
}

fn window_layout(canvas: &Screen) -> Result<Layout, String> {
    let (width, height) = canvas.window().size();
    Layout::home(
        u16::try_from(width).map_err(|_| "window too wide")?,
        u16::try_from(height).map_err(|_| "window too tall")?,
    )
}

/// Whether the launcher keeps running after one event-loop iteration.
#[derive(PartialEq, Eq)]
enum Flow {
    Continue,
    Exit,
}

/// Long-lived launcher loop state. Frame presentation, input suppression and
/// child-exit ordering stay in [`Heartbeat::step`]; per-operation work lives in
/// the helper functions below.
struct Heartbeat<'c, 's, 't, P: Platform> {
    sdl: &'c sdl2::Sdl,
    canvas: &'c mut Screen<'s>,
    layout: &'c mut Layout,
    state: &'c mut Launcher,
    creator: &'t TextureCreator<WindowContext>,
    textures: &'c mut Artwork<'t>,
    worker: &'c mut Option<crate::platform::system::Worker>,
    events: &'c mut sdl2::EventPump,
    keyboard: &'c mut vitrallis_native::keyboard::Keyboard,
    child: &'c mut ProcessSet<crate::process::NativeProcess>,
    broker: &'c vitrallis_native::ipc::Broker,
    pointer: &'c mut PointerInput,
    desktop_input: &'c mut crate::input::DesktopInput,
    platform: &'c P,
    config: &'c Config,
    smoke: bool,
    accept_after: Instant,
    dirty: bool,
    last_present: Instant,
    last_wait_error: Option<String>,
    next_poll: Instant,
    deadline: Instant,
}

fn event_loop(
    sdl: &sdl2::Sdl,
    canvas: &mut Screen,
    layout: &Layout,
    mut state: Launcher,
    platform: &impl Platform,
    config: &Config,
) -> Result<(), String> {
    let mut current_layout = layout.clone();
    let smoke = config.mode == crate::config::Mode::Smoke;
    let creator = canvas.texture_creator();
    let mut textures = artwork(&creator, &state);
    let mut worker = system_worker(platform, &mut state);
    let mut events = sdl.event_pump()?;
    let mut keyboard = vitrallis_native::keyboard::Keyboard::new(&sdl.video()?);
    let mut child = ProcessSet::<crate::process::NativeProcess>::default();
    if !smoke && let Err(error) = child.tor.initialize() {
        state.settings.message = error;
    }
    let broker = crate::native::broker(&mut state)?;
    let mut pointer = PointerInput::default();
    let mut desktop_input = crate::input::DesktopInput::default();
    if smoke {
        inject_activation(sdl, canvas)?;
    }
    present_initial(canvas, layout, &state, &textures)?;
    let mut heartbeat = Heartbeat {
        sdl,
        canvas,
        layout: &mut current_layout,
        state: &mut state,
        creator: &creator,
        textures: &mut textures,
        worker: &mut worker,
        events: &mut events,
        keyboard: &mut keyboard,
        child: &mut child,
        broker: &broker,
        pointer: &mut pointer,
        desktop_input: &mut desktop_input,
        platform,
        config,
        smoke,
        accept_after: Instant::now(),
        dirty: false,
        last_present: Instant::now(),
        last_wait_error: None,
        next_poll: Instant::now(),
        deadline: Instant::now() + Duration::from_secs(10),
    };
    loop {
        match heartbeat.step()? {
            Flow::Continue => {}
            Flow::Exit => return Ok(()),
        }
    }
}

impl<P: Platform> Heartbeat<'_, '_, '_, P> {
    /// One iteration: refresh derived state, present when the frame is due,
    /// consume one event, then reap finished children.
    fn step(&mut self) -> Result<Flow, String> {
        if std::mem::take(&mut self.state.view_changed) {
            self.textures.refresh(self.creator, self.state);
            self.dirty = true;
        }
        self.refresh_derived()?;
        // Coalesce queued input before presenting. Rendering every key/text pair
        // makes rapid typing accumulate behind VSync on slow software backends.
        let queued = self.events.poll_event();
        if self.dirty
            && (queued.is_none() || self.last_present.elapsed() >= Duration::from_millis(16))
        {
            self.dirty = present_frame(
                self.canvas,
                self.layout,
                self.state,
                self.textures,
                self.worker,
            )?;
            self.last_present = Instant::now();
        }
        let event = queued
            .or_else(|| wait_event(self.events, self.state.phase, self.next_poll, self.dirty));
        if let Some(event) = event
            && self.input(event)? == Flow::Exit
        {
            return Ok(Flow::Exit);
        }
        if self.reap_children()? == Flow::Exit {
            return Ok(Flow::Exit);
        }
        if self.smoke && Instant::now() >= self.deadline {
            return Err("smoke test timed out".into());
        }
        Ok(Flow::Continue)
    }

    /// Re-derives everything the catalogue, workers and shell state changed.
    fn refresh_derived(&mut self) -> Result<(), String> {
        self.dirty |= refresh_system(self.worker, &mut self.state.settings);
        if refresh_app_center(self.sdl, self.config, self.state, &mut self.dirty)? {
            self.textures.refresh(self.creator, self.state);
            self.dirty = true;
        }
        self.dirty |= refresh_shell(self.state, self.child);
        self.dirty |= refresh_launch(self.state, self.child);
        self.dirty |= launch_from_center(
            self.canvas,
            self.layout,
            self.state,
            self.textures,
            self.child,
        )?;
        self.dirty |= open_native(
            self.broker,
            self.canvas,
            self.layout,
            self.state,
            self.textures,
            self.child,
        )?;
        Ok(())
    }

    /// Applies one SDL event. Input suppression and the shared
    /// pointer/keyboard/touch path stay in one place so a press can never be
    /// delivered twice.
    fn input(&mut self, mut event: Event) -> Result<Flow, String> {
        self.keyboard.event(&mut event);
        if closing(&event) && !self.state.app_center.busy {
            return Ok(Flow::Exit);
        }
        if matches!(event, Event::RenderDeviceReset { .. }) {
            self.canvas.reset()?;
            self.textures.reset(self.creator, self.state);
            self.dirty = true;
        }
        if matches!(
            event,
            Event::Window {
                win_event: WindowEvent::SizeChanged(..) | WindowEvent::Resized(..),
                ..
            }
        ) {
            let (width, height) = self.canvas.window().size();
            if width >= 320 && height >= 200 {
                *self.layout = Layout::home(
                    u16::try_from(width).map_err(|_| "window width")?,
                    u16::try_from(height).map_err(|_| "window height")?,
                )?;
                self.pointer.clear();
                self.desktop_input.clear();
                self.state.settings.clear_pointer();
                self.state.app_center.lost_focus();
            }
        }
        self.dirty |= exposed(&event);
        if matches!(
            event,
            Event::Window {
                win_event: WindowEvent::FocusLost,
                ..
            }
        ) {
            self.child.stop_focus_retry();
        }
        self.dirty |= window_focus(&event, self.state, self.pointer, &mut self.accept_after);
        if Instant::now() >= self.accept_after {
            let (consumed, changed) =
                desktop_event(&event, self.layout, self.state, self.desktop_input);
            if consumed {
                self.pointer.clear();
                if changed {
                    self.textures.refresh(self.creator, self.state);
                }
                self.dirty |= panel_input(&event);
            } else {
                self.dirty |= terminate_selected(&event, self.state, self.child);
                let (action, system_changed) =
                    translate_action(&event, self.layout, self.state, self.pointer, self.worker);
                self.dirty |= system_changed;
                self.dirty |= handle_action(
                    action,
                    self.canvas,
                    self.layout,
                    self.state,
                    self.textures,
                    self.child,
                )?;
                self.dirty |= open_requested(self.state, self.child);
            }
        } else {
            self.pointer.clear();
            self.desktop_input.clear();
        }
        Ok(Flow::Continue)
    }

    /// Reaps finished children and mirrors each exit into the shell state.
    fn reap_children(&mut self) -> Result<Flow, String> {
        let result = poll_children(self.child, &mut self.next_poll);
        match result {
            Ok(Some(status)) => {
                eprintln!("level=info event=app_exited status={status:?}");
                self.state.sync_states(self.child);
                refresh_utility(self.state, self.worker, self.child.exited_active);
                let raise = app_exited(self.state, status, self.child.exited_active);
                let catalog_changed =
                    refresh_exit_catalog(self.sdl, self.config, self.state, self.pointer);
                if catalog_changed {
                    self.textures.refresh(self.creator, self.state);
                }
                self.last_wait_error = None;
                if self.child.exited_active {
                    self.accept_after = Instant::now();
                }
                raise_after_exit(self.canvas, self.platform, raise);
                self.dirty = true;
                if self.smoke {
                    finish_smoke(self.canvas, self.layout, self.state, self.textures, status)?;
                    return Ok(Flow::Exit);
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.dirty |= report_wait_error(error, &mut self.last_wait_error, self.state);
            }
        }
        Ok(Flow::Continue)
    }
}

fn present_frame(
    canvas: &mut Screen,
    layout: &Layout,
    state: &mut Launcher,
    textures: &[Option<Texture<'_>>],
    worker: &mut Option<crate::platform::system::Worker>,
) -> Result<bool, String> {
    render(canvas, layout, state, textures)?;
    canvas.present();
    Ok(submit_power_after_present(worker, &mut state.settings))
}

fn raise_after_exit(canvas: &mut Screen, platform: &impl Platform, raise: bool) {
    if raise && platform.raise_after_exit() {
        canvas.window_mut().raise();
        crate::platform::restore_shell_focus();
    }
}

fn report_wait_error(error: String, last: &mut Option<String>, state: &mut Launcher) -> bool {
    // Retain the process owner until the child can be reaped.
    if last.as_ref() == Some(&error) {
        return false;
    }
    eprintln!("level=error event=wait_failed message={error:?}");
    state.status.clone_from(&error);
    *last = Some(error);
    true
}

fn reorder(state: &mut Launcher, later: bool) -> Result<(), String> {
    let mut order: Vec<_> = state.apps.iter().map(|app| app.id.clone()).collect();
    let selected = state.selected;
    let next = if later {
        (selected + 1).min(order.len().saturating_sub(1))
    } else {
        selected.saturating_sub(1)
    };
    if selected < order.len() {
        let id = order[selected].clone();
        order.swap(selected, next);
        order.extend(
            state
                .folders
                .order
                .iter()
                .filter(|id| !state.apps.iter().any(|app| &app.id == *id))
                .cloned(),
        );
        state.folders = crate::folders::Folders::change(crate::folders::Change::Reorder(order))?;
        state.rebuild_view(Some(&id));
    }
    Ok(())
}

fn desktop_event(
    event: &Event,
    layout: &Layout,
    state: &mut Launcher,
    input: &mut crate::input::DesktopInput,
) -> (bool, bool) {
    use crate::{
        input::DesktopAction,
        shortcuts::{Store, screen::Request},
    };
    if state.phase != Phase::Ready
        || state.settings.open
        || state.app_center.open
        || state.error.is_some()
    {
        input.clear();
        return (false, false);
    }
    if !state.desktop.open {
        match state
            .desktop
            .toolbar_event(event)
            .or_else(|| input.event(event, layout, state.visible_count()))
        {
            Some(DesktopAction::Back) => {
                if state.folder.is_some() {
                    state.leave_folder();
                }
            }
            Some(DesktopAction::Focus) => (),
            Some(DesktopAction::Add) => state.desktop.add(),
            Some(DesktopAction::Menu(index)) => {
                if let Some(index) = index {
                    state.selected = state.page_start() + index;
                }
                state.desktop.folders = state.folders.clone();
                state.desktop.folder_context = state
                    .apps
                    .get(state.selected)
                    .filter(|app| app.source == crate::app::AppSource::Folder)
                    .map(|app| app.id.clone())
                    .or_else(|| state.folder.clone());
                state.desktop.menu(state.apps.get(state.selected).cloned());
            }
            None => return (false, false),
        }
        input.clear();
        return (true, false);
    }
    let Some(request) = state.desktop.event(event, layout) else {
        return (true, false);
    };
    let result = (|| {
        if matches!(request, Request::Uninstall) {
            let app = state.desktop.entry.as_ref().ok_or("No app selected")?;
            state.app_center.uninstall_entry(app)?;
            return Ok(false);
        }
        if let Request::Reorder(later) = request {
            reorder(state, later)?;
            return Ok(true);
        }
        if let Request::Folder(change) = request {
            let selected = state.desktop.entry.as_ref().map(|app| app.id.clone());
            state.folders = crate::folders::Folders::change(change)?;
            state.rebuild_view(selected.as_deref());
            return Ok(true);
        }
        let store = Store::current()?;
        match request {
            Request::Save => {
                store.save(
                    state.desktop.entry.as_ref().map(|app| app.id.as_str()),
                    &state.desktop.draft,
                )?;
            }
            Request::Remove => {
                let app = state.desktop.entry.as_ref().ok_or("No shortcut selected")?;
                if app.source == crate::app::AppSource::Custom {
                    store.remove(app)?;
                } else {
                    store.hide(app)?;
                }
            }
            Request::Uninstall | Request::Folder(_) | Request::Reorder(_) => (),
        }
        let mut catalog = crate::discovery::Catalog {
            apps: state.all_apps.clone(),
            ..crate::discovery::Catalog::default()
        };
        crate::shortcuts::integrate(&mut catalog);
        state.reload(catalog.apps)?;
        Ok::<_, String>(true)
    })();
    match result {
        Ok(changed) => {
            state.desktop.open = false;
            (true, changed)
        }
        Err(error) => {
            state.desktop.error = error;
            (true, false)
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
        sdl.mouse().show_cursor(false);
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
    if state.phase != Phase::Ready {
        return Ok(false);
    }
    match crate::native::requested(broker, state) {
        Ok(Some(index)) => {
            render(
                canvas,
                layout,
                state,
                if state.view_changed { &[] } else { icons },
            )?;
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
    state.preferences.ampm = state.settings.policy.ampm;
    if state.phase == Phase::Ready {
        child.returned_home();
    }
    let stopped = child.background_policy(&state.settings.policy, Instant::now());
    for id in &stopped {
        state.app_states.remove(id);
    }
    if !stopped.is_empty() {
        let message = format!(
            "{} app(s) closed after their background timeout",
            stopped.len()
        );
        if state.phase == Phase::Running {
            // The foreground app is untouched; only the notice changes.
            state.notify(message);
        } else {
            state.finished(message);
        }
    }
    child.sync_tor_apps();
    if let Some(control) = state.settings.tor_control.take()
        && let Err(error) = child.tor.request(control)
    {
        state.settings.message = error;
    }
    let snapshot = child.tor.snapshot();
    let tor_changed = snapshot != state.settings.tor;
    if child.waiting_for_tor() {
        state.status = format!("Waiting for Tor: {}", snapshot.state.label());
    }
    state.settings.tor = snapshot;
    let dirty = (tor_changed && (state.settings.open || child.waiting_for_tor()))
        | refresh_timezone(state, child)
        | refresh_focus(child, state);
    // Restore deliberately waits for a quiescent session: no owned apps, App
    // Center mutation or pending system operation. Both actions take effect on
    // relaunch, so they share the same gate.
    let blocked = state.app_center.busy || child.has_children() || state.settings.pending;
    state.settings.updater.relaunch_if_requested(blocked)
        | state.settings.updater.restore_if_requested(blocked)
        | dirty
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
    if state.phase != Phase::Ready {
        // One start request at a time; the App Center stays open with the reason.
        state.app_center.message = "An application is already starting; try again".into();
        return Ok(true);
    }
    state.reveal(&id);
    if let Some(index) = state.apps.iter().position(|app| app.id == id) {
        state.app_center.open = false;
        state.selected = index;
        let local = index - state.page_start();
        handle_action(
            Some(Action::SelectAndActivate(local)),
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
        match crate::app_center::refresh_apps(&state.all_apps).and_then(|mut apps| {
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
    if (state.app_center.editing() || state.desktop.editing()) && !input.is_active() {
        input.start();
    } else if !state.app_center.editing() && !state.desktop.editing() && input.is_active() {
        input.stop();
    }
    Ok(artwork_changed)
}

fn terminate_selected(event: &Event, state: &mut Launcher, child: &mut impl Processes) -> bool {
    if state.folder.is_some()
        || state.phase != Phase::Ready
        || state.settings.open
        || state.app_center.open
        || state.desktop.open
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
    state.sync_states(child);
    state.finished("APP CLOSED - READY".into());
    true
}

fn refresh_launch(state: &mut Launcher, child: &mut impl Processes) -> bool {
    match child.poll_launch() {
        Some(Ok(id)) => {
            state.sync_states(child);
            // System helpers (time zone, calibration, network manager) are not
            // catalogue entries: keep the name captured when they were started.
            let name = state
                .all_apps
                .iter()
                .find(|app| app.id == id)
                .map_or_else(|| state.opening.clone(), |app| Some(app.name.clone()))
                .unwrap_or_else(|| "APP".into());
            state.launched(&name);
            true
        }
        Some(Err(error)) => {
            // The failure is recorded by the process owner, so indicators stay
            // accurate while the dialog explains what happened.
            state.sync_states(child);
            state.failed(error);
            true
        }
        None => false,
    }
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
            state.finished("Still starting in background".into());
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
        crate::native::inherit_runtime(&mut catalog.apps, &state.all_apps);
        if catalog.apps.is_empty() && !catalog.diagnostics.is_empty() {
            return Err("catalogue unavailable; keeping previous apps".into());
        }
        let changed = state.reload(catalog.apps)?;
        let appearance_changed = state.preferences != catalog.preferences;
        state.preferences = catalog.preferences;
        state.preferences.ampm = state.settings.policy.ampm;
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
        Event::RenderTargetsReset { .. }
            | Event::Window {
                win_event: WindowEvent::Exposed
                    | WindowEvent::Shown
                    | WindowEvent::Restored
                    | WindowEvent::SizeChanged(..)
                    | WindowEvent::Resized(..),
                ..
            }
    )
}
// Pointer motion and unrelated queue traffic cannot change these panels.
// Settings separately compares its slider preview during active drags.
const fn panel_input(event: &Event) -> bool {
    matches!(
        event,
        Event::KeyDown { .. }
            | Event::TextInput { .. }
            | Event::MouseButtonDown { .. }
            | Event::MouseButtonUp { .. }
            | Event::FingerDown { .. }
            | Event::FingerUp { .. }
            | Event::MouseWheel { .. }
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
        return (None, panel_input(event));
    }
    state.settings.network_available = state
        .all_apps
        .iter()
        .any(|app| app.is_system_settings() && app.unavailable.is_none());
    if state.settings.open {
        let pointer_before = state.settings.pointer_visual();
        let request = state.settings.event(event, layout);
        submit_setting(request, worker, &mut state.settings);
        return (
            None,
            panel_input(event) || pointer_before != state.settings.pointer_visual(),
        );
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
    dirty |= settings.poll_storage();
    if let Some(worker) = worker {
        if let Some(update) = worker.update() {
            dirty |= settings.system_state != crate::settings::SystemState::Ready;
            settings.system_state = crate::settings::SystemState::Ready;
            dirty |= settings.status != update.status || update.result.is_some();
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
        }
        dirty |= settings.pending != worker.pending;
        settings.pending = worker.pending;
        if worker.stale() && settings.system_state != crate::settings::SystemState::Stale {
            settings.status = crate::platform::system::Status::default();
            settings.system_state = crate::settings::SystemState::Stale;
            dirty = true;
        }
    } else if settings.system_state != crate::settings::SystemState::Unavailable {
        settings.system_state = crate::settings::SystemState::Unavailable;
        dirty = true;
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
    if settings.open && settings.page == crate::settings::Page::Updates {
        // Read-only pointer and file-shape checks; the restore itself
        // revalidates every file under the update lock. Only a change needs a
        // redraw so the footer control appears or disappears.
        let available = crate::platform::update::restore_available();
        dirty |= settings.updater.restore_available != available;
        settings.updater.restore_available = available;
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
        Some(crate::settings::Request::RestorePrevious) => settings.updater.request_restore(),
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
                | Control::ReadTimezone
                | Control::Radio(_, _) => None,
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
                    | Control::ReadTimezone
                    | Control::Radio(_, _) => None,
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
            state.launching("Time zone authentication");
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
            state.launching(&app.name);
        }
        Err(error) => state.settings.message = error,
    }
}
fn open_network(state: &mut Launcher, child: &mut impl Processes) {
    state.settings.network = crate::settings::NetworkState::Idle;
    let Some(mut app) = state
        .all_apps
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
            state.launching(&app.name);
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
    let before = (
        state.selected,
        state.phase,
        state.error.is_some(),
        state.settings.open,
        state.desktop.toolbar,
    );
    // The acknowledgement frame goes out before any process request, so the
    // selection and the "is launching..." message are visible immediately.
    let activating = state.phase == Phase::Ready
        && matches!(action, Action::Activate | Action::SelectAndActivate(_));
    if let Some(index) = state.input(action) {
        render(
            canvas,
            layout,
            state,
            if state.view_changed { &[] } else { icons },
        )?;
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
            state.desktop.toolbar,
        )
        || activating)
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
    render(
        canvas,
        layout,
        state,
        if state.view_changed { &[] } else { icons },
    )?;
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
    if dirty || phase == Phase::Launching {
        // A start in flight is completed by a worker; poll it promptly so the
        // launching message becomes "running" without a visible pause.
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
    #[ignore = "opt-in real idle-loop measurement; takes four seconds"]
    fn idle_loop_stops_after_startup() -> Result<(), String> {
        measure_idle_loop(false)?;
        measure_idle_loop(true)
    }

    fn measure_idle_loop(tor_panel: bool) -> Result<(), String> {
        sdl2::hint::set("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let window = video
            .window("Idle benchmark", 480, 272)
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
        let sender = sdl.event()?.event_sender();
        let stop = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            sender.push_event(Event::Quit { timestamp: 0 })
        });
        crate::renderer::performance::reset();
        let start = Instant::now();
        let mut state = Launcher::new(Vec::new(), 3, 6)?;
        if tor_panel {
            state.settings.show();
            state.settings.page(crate::settings::Page::Tor);
        }
        let result = event_loop(
            &sdl,
            &mut canvas,
            &layout,
            state,
            &crate::platform::generic::Generic,
            &Config::default(),
        );
        stop.join().map_err(|_| "idle benchmark timer failed")??;
        result?;
        let counts = crate::renderer::performance::snapshot();
        eprintln!(
            "tor_panel={tor_panel} idle_elapsed_ms={} frames={} last_frame_ms={:?}",
            start.elapsed().as_millis(),
            counts.frames,
            counts
                .last_frame
                .map(|last| last.duration_since(start).as_millis())
        );
        // Independent startup workers can finish in separate frames. The idle
        // invariant is no rendering after they settle, not a fixed frame count.
        assert!(counts.frames > 0, "the initial frame must be presented");
        assert!(
            counts
                .last_frame
                .is_some_and(|last| last.duration_since(start) < Duration::from_millis(500)),
            "idle home and Tor Settings must not redraw after startup settles"
        );
        Ok(())
    }

    #[test]
    fn irrelevant_panel_events_do_not_schedule_frames() -> Result<(), String> {
        let layout = Layout::home(480, 272)?;
        let mut state = Launcher::new(Vec::new(), 3, 6)?;
        let mut pointer = PointerInput::default();
        let mut worker = None;
        let motion = Event::MouseMotion {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mousestate: sdl2::mouse::MouseState::from_sdl_state(0),
            x: 20,
            y: 20,
            xrel: 1,
            yrel: 1,
        };
        state.settings.show();
        for _ in 0..200 {
            assert!(!translate_action(&motion, &layout, &mut state, &mut pointer, &mut worker).1);
        }
        state.settings.cancel();
        state.app_center.open = true;
        for _ in 0..200 {
            assert!(!translate_action(&motion, &layout, &mut state, &mut pointer, &mut worker).1);
        }
        assert!(!panel_input(&Event::User {
            timestamp: 0,
            window_id: 0,
            type_: 0,
            code: 0,
            data1: std::ptr::null_mut(),
            data2: std::ptr::null_mut()
        }));
        assert!(exposed(&Event::RenderTargetsReset { timestamp: 0 }));
        Ok(())
    }

    /// Start on the launch worker and wait for the owned child to exist.
    fn started(child: &mut ProcessSet, app: &crate::app::AppEntry) -> Result<(), String> {
        child.start(app)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match child.poll_launch() {
                Some(Ok(_)) => return Ok(()),
                Some(Err(error)) => return Err(error),
                None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
                None => return Err("launch timed out".into()),
            }
        }
    }

    /// Poll the launch result the way the event loop does, with a bound.
    fn wait_launch(state: &mut Launcher, child: &mut impl Processes) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if refresh_launch(state, child) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// One launch, one foreground/background transition, one explicit close.
    #[test]
    fn launch_background_and_explicit_close_follow_one_state_machine() -> Result<(), String> {
        use crate::{
            app::AppEntry,
            process::{AppState, Processes},
        };
        // Only this test uses the counter, and it resets it before starting.
        static STARTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        #[derive(Default)]
        struct Fake;
        impl Processes for Fake {
            fn start(&mut self, _: &AppEntry) -> Result<(), String> {
                STARTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
            fn poll(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
                Ok(None)
            }
            fn focus(&mut self) -> Result<(), String> {
                Ok(())
            }
        }
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let id = apps[0].id.clone();
        let mut state = Launcher::new(apps, 3, 6)?;
        STARTS.store(0, std::sync::atomic::Ordering::SeqCst);
        let mut child = ProcessSet::<Fake>::default();
        let starts = || STARTS.load(std::sync::atomic::Ordering::SeqCst);
        // Launch: acknowledged immediately, completed by the worker.
        assert_eq!(state.input(Action::Activate), Some(0));
        assert_eq!(state.phase, Phase::Launching);
        assert!(state.status.contains("launching"));
        // While the start is in flight activation is refused, but the menu stays
        // responsive: arrows still move the selection.
        assert_eq!(state.input(Action::Activate), None);
        assert_eq!(state.input(Action::SelectAndActivate(0)), None);
        assert_eq!(
            state.input(Action::Move(crate::navigation::Direction::Right)),
            None
        );
        assert_eq!(state.selected, 1);
        process::activate(&mut state, &mut child, 0);
        assert!(wait_launch(&mut state, &mut child));
        assert_eq!(state.phase, Phase::Running);
        assert_eq!(state.app_state(&id), AppState::RunningForeground);
        assert_eq!(starts(), 1);
        // Returning to the main menu backgrounds the app: never terminates it.
        state.input(Action::Back);
        child.returned_home();
        state.sync_states(&child);
        assert_eq!(state.phase, Phase::Ready);
        assert_eq!(state.app_state(&id), AppState::RunningBackground);
        assert!(
            child
                .background_policy(&crate::preferences::Policy::default(), Instant::now())
                .is_empty()
        );
        assert_eq!(child.state(&id), AppState::RunningBackground);
        // Resuming focuses the existing process instead of starting another.
        assert_eq!(state.input(Action::SelectAndActivate(0)), Some(0));
        process::activate(&mut state, &mut child, 0);
        assert!(wait_launch(&mut state, &mut child));
        assert_eq!(state.phase, Phase::Running);
        assert_eq!(starts(), 1);
        // An explicit close still terminates promptly.
        let escape = Event::KeyDown {
            timestamp: 0,
            window_id: 1,
            keycode: Some(Keycode::Escape),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        };
        state.returned_home();
        assert!(terminate_selected(&escape, &mut state, &mut child));
        assert_eq!(state.phase, Phase::Ready);
        assert_eq!(state.app_state(&id), AppState::Stopped);
        assert!(!child.has_children());
        Ok(())
    }

    #[test]
    fn failed_launch_returns_to_a_valid_retryable_state() -> Result<(), String> {
        use crate::{app::AppEntry, process::AppState, process::Processes};
        #[derive(Default)]
        struct Broken;
        impl Processes for Broken {
            fn start(&mut self, _: &AppEntry) -> Result<(), String> {
                Err("no runtime for this app".into())
            }
            fn poll(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
                Ok(None)
            }
        }
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let id = apps[0].id.clone();
        let mut state = Launcher::new(apps, 3, 6)?;
        let mut child = ProcessSet::<Broken>::default();
        assert_eq!(state.input(Action::Activate), Some(0));
        process::activate(&mut state, &mut child, 0);
        // The worker reports the failure; the Shell shows a dismissible dialog.
        assert!(wait_launch(&mut state, &mut child));
        assert_eq!(state.phase, Phase::Ready);
        assert_eq!(state.app_state(&id), AppState::Failed);
        assert!(state.error.is_some());
        assert!(!child.has_children());
        // Dismissing restores normal interaction and a retry is allowed.
        assert_eq!(state.input(Action::Activate), None);
        assert_eq!(state.input(Action::Activate), Some(0));
        assert_eq!(state.phase, Phase::Launching);
        Ok(())
    }

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
        started(&mut child, &state.apps[0])?;
        started(&mut child, &state.apps[1])?;
        state.sync_states(&child);
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
        state.launching("Test");
        state.launched("Test");
        assert!(!terminate_selected(&event, &mut state, &mut child));
        state.returned_home();
        state.selected = 2;
        assert!(!terminate_selected(&event, &mut state, &mut child));
        state.selected = 0;
        assert_eq!(child.running_ids().len(), 2);

        assert!(terminate_selected(&event, &mut state, &mut child));
        assert_eq!(
            state.app_state(&state.apps[1].id),
            crate::process::AppState::RunningForeground
        );
        assert_eq!(child.running_ids(), [state.apps[1].id.clone()]);
        assert!(!state.app_state(&state.apps[0].id).is_running());
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
        assert_eq!(state.phase, Phase::Launching);
        assert!(state.opening.is_some());
        // A window that never appears leaves the launch tracked but reports the
        // outcome without breaking the Shell.
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
        state.launching("Test");
        state.launched("Test");
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
        state.settings.page(crate::settings::Page::Device);
        state.settings.input(Action::SelectAndActivate(3));
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
        state.launching("Test");
        state.launched("Test");
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
