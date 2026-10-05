//! Event-driven SDL rendering and shared keyboard/touch dialogs.
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::{Keycode, Mod},
    mouse::MouseButton,
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{Canvas, TextureCreator},
    video::{Window, WindowContext},
};
use std::{
    io::{Seek, Write},
    path::PathBuf,
};

use crate::theme::{self, ACCENT, BACKGROUND, MUTED, PANEL, SELECTED, TEXT};

/// Common native command-line options; paths stay in their original OS encoding.
#[derive(Debug, Default)]
pub struct Options {
    pub renderer: crate::renderer::RendererMode,
    pub size: Option<(u32, u32)>,
    pub fullscreen: bool,
    pub smoke: bool,
    pub screenshot: Option<PathBuf>,
    pub path: Option<PathBuf>,
}
impl Options {
    /// # Errors
    /// Rejects unknown options, extra paths, and impractical display dimensions.
    pub fn parse(name: &str) -> Result<Option<Self>, String> {
        Self::parse_args(name, std::env::args_os().skip(1))
    }

    /// Parse common options from an application's argument iterator.
    /// # Errors
    /// Rejects unknown options, extra paths, and impractical display dimensions.
    pub fn parse_args(
        name: &str,
        mut args: impl Iterator<Item = std::ffi::OsString>,
    ) -> Result<Option<Self>, String> {
        let mut options = Self::default();
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--version") => {
                    println!("{name} {}", env!("CARGO_PKG_VERSION"));
                    return Ok(None);
                }
                Some("--help") => {
                    println!(
                        "{name} [--renderer auto|hardware|software] [--size WIDTHxHEIGHT] [--fullscreen] [--smoke-test] [--screenshot NEW.bmp] [--] [PATH]"
                    );
                    return Ok(None);
                }
                Some("--renderer") => {
                    options.renderer = args
                        .next()
                        .ok_or("--renderer requires auto, hardware or software")?
                        .to_str()
                        .ok_or("Invalid renderer encoding")?
                        .parse()?;
                }
                Some("--size") => {
                    let value = args.next().ok_or("Missing display size")?;
                    let (w, h) = value
                        .to_str()
                        .and_then(|s| s.split_once('x'))
                        .ok_or("Use WIDTHxHEIGHT")?;
                    let width = w
                        .parse()
                        .map_err(|error| format!("Invalid width: {error}"))?;
                    let height = h
                        .parse()
                        .map_err(|error| format!("Invalid height: {error}"))?;
                    if !(320..=4096).contains(&width) || !(200..=2160).contains(&height) {
                        return Err("Display must be 320x200 to 4096x2160".into());
                    }
                    options.size = Some((width, height));
                }
                Some("--fullscreen") => options.fullscreen = true,
                Some("--smoke-test") => options.smoke = true,
                Some("--screenshot") => {
                    options.screenshot = Some(args.next().ok_or("Missing screenshot path")?.into());
                }
                Some("--") => {
                    options.path = args.next().map(PathBuf::from);
                    if args.next().is_some() {
                        return Err("Too many paths".into());
                    }
                    break;
                }
                Some(s) if s.starts_with('-') => return Err(format!("Unknown option: {s}")),
                _ if options.path.is_none() => options.path = Some(arg.into()),
                _ => return Err("Only one path is accepted".into()),
            }
        }
        Ok(Some(options))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Key(Keycode, Mod),
    Text(String),
    Click(i32, i32),
    Scroll(i32),
    Resize,
    Wake,
    Close,
    Ignore,
}

/// One SDL event queue and canvas. No timers, animation thread, or idle repaint.
pub struct Ui<'a> {
    presentation: crate::renderer::PresentationClock,
    font: crate::font::Atlas<'a>,
    creator: &'a TextureCreator<WindowContext>,
    pub canvas: Canvas<Window>,
    pub sdl: sdl2::Sdl,
    events: sdl2::EventPump,
    keyboard: crate::keyboard::Keyboard,
    press: Option<(i64, i32, i32)>,
    focus_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    released_from: Option<(i32, i32)>,
    pub width: i32,
    pub height: i32,
    pub scale: i32,
}
/// SDL initialization owner. Keep its texture creator alive around `Ui`.
pub struct Session {
    pub canvas: Canvas<Window>,
    sdl: sdl2::Sdl,
    events: sdl2::EventPump,
    keyboard: crate::keyboard::Keyboard,
}
impl Session {
    /// # Errors
    /// Reports SDL/video/window initialization errors.
    pub fn new(title: &str, options: &Options) -> Result<Self, String> {
        hint("SDL_VIDEO_ALLOW_SCREENSAVER", "1");
        hint("SDL_TOUCH_MOUSE_EVENTS", "0");
        hint("SDL_MOUSE_TOUCH_EVENTS", "0");
        hint("SDL_MOUSE_FOCUS_CLICKTHROUGH", "1");
        // Handle the window close once. SDL's additional last-window Quit would
        // immediately cancel the unsaved-document or terminal confirmation.
        hint("SDL_QUIT_ON_LAST_WINDOW_CLOSE", "0");
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let session = std::env::var_os("VITRALLIS_SESSION").is_some();
        let (width, height) = options
            .size
            .unwrap_or(if session { (480, 272) } else { (800, 480) });
        let (canvas, info) = crate::renderer::initialize(&video, options.renderer, || {
            let mut builder = video.window(title, width, height);
            let _window_options = builder.position_centered().resizable().hidden();
            if options.fullscreen || (session && options.size.is_none()) {
                let _fullscreen_options = builder.fullscreen_desktop();
            }
            let mut window = builder.build().map_err(|e| e.to_string())?;
            window
                .set_minimum_size(320, 200)
                .map_err(|e| e.to_string())?;
            Ok(window)
        })?;
        eprintln!("{info} app={title:?}");
        video.text_input().start();
        let events = sdl.event_pump()?;
        let keyboard = crate::keyboard::Keyboard::new(&video);
        Ok(Self {
            canvas,
            sdl,
            events,
            keyboard,
        })
    }
}
impl<'a> Ui<'a> {
    /// Bind persistent textures to this session's renderer.
    /// # Errors
    /// Reports atlas allocation and display-size errors.
    pub fn new(
        session: Session,
        creator: &'a TextureCreator<WindowContext>,
    ) -> Result<Self, String> {
        let Session {
            canvas,
            sdl,
            events,
            keyboard,
        } = session;
        let presentation = crate::renderer::PresentationClock::new(&canvas);
        let mut ui = Self {
            presentation,
            font: crate::font::Atlas::new(creator)?,
            creator,
            canvas,
            sdl,
            events,
            keyboard,
            press: None,
            focus_requested: std::sync::Arc::default(),
            released_from: None,
            width: 0,
            height: 0,
            scale: 1,
        };
        ui.resize()?;
        Ok(ui)
    }
    /// Attach the optional launcher inbox; focus works even while a dialog is open.
    /// # Errors
    /// Reports private socket or SDL event initialization errors.
    pub fn inbox(&self, name: &str) -> Result<Option<crate::ipc::Inbox>, String> {
        crate::ipc::Inbox::new(
            name,
            self.sdl.event()?.event_sender(),
            std::sync::Arc::clone(&self.focus_requested),
        )
        .map_err(|e| e.to_string())
    }
    fn resize(&mut self) -> Result<(), String> {
        let (w, h) = self.canvas.output_size()?;
        self.width = i32::try_from(w).map_err(|error| format!("Display too wide: {error}"))?;
        self.height = i32::try_from(h).map_err(|error| format!("Display too tall: {error}"))?;
        self.scale = (self.width / 640_i32)
            .min(self.height / 400_i32)
            .clamp(1_i32, 3_i32);
        Ok(())
    }
    #[must_use]
    pub const fn line(&self) -> i32 {
        theme::LINE.saturating_mul(self.scale)
    }
    #[must_use]
    pub const fn cell(&self) -> i32 {
        let cell = theme::CELL.saturating_mul(self.scale);
        if cell > 0 { cell } else { 1 }
    }
    #[must_use]
    pub const fn header_height(&self) -> i32 {
        28_i32.saturating_mul(self.scale)
    }
    #[must_use]
    pub const fn footer_height(&self) -> i32 {
        28_i32.saturating_mul(self.scale)
    }
    #[must_use]
    pub fn rows(&self, top: i32) -> usize {
        text_rows(self.height, self.footer_height(), self.line(), top)
    }
    pub fn clear(&mut self) {
        self.canvas.set_draw_color(BACKGROUND);
        self.canvas.clear();
    }
    /// # Errors
    /// Reports a renderer error.
    pub fn fill(&mut self, rect: Rect, color: Color) -> Result<(), String> {
        self.canvas.set_draw_color(color);
        self.canvas.fill_rect(rect)
    }
    /// Draws only fitting glyphs. Unsupported glyphs are displayed as '?', never changed in a document.
    /// # Errors
    /// Reports a renderer error.
    pub fn text(
        &mut self,
        value: &str,
        x: i32,
        y: i32,
        width: i32,
        color: Color,
    ) -> Result<(), String> {
        let limit = self.columns(width);
        for (index, ch) in value.chars().take(limit).enumerate() {
            let glyph_x = i32::try_from(index)
                .map_err(|error| format!("Text width: {error}"))?
                .checked_mul(self.cell())
                .and_then(|offset| x.checked_add(offset))
                .ok_or("Text coordinate overflow")?;
            self.glyph(ch, glyph_x, y, color)?;
        }
        Ok(())
    }
    /// # Errors
    /// Reports a renderer error.
    pub fn glyph(&mut self, ch: char, x: i32, y: i32, color: Color) -> Result<(), String> {
        self.font.draw(
            &mut self.canvas,
            ch,
            Rect::new(
                x,
                y,
                theme::CELL
                    .unsigned_abs()
                    .saturating_mul(self.scale.unsigned_abs()),
                theme::CELL
                    .unsigned_abs()
                    .saturating_mul(self.scale.unsigned_abs()),
            ),
            color,
        )
    }
    /// # Errors
    /// Reports a renderer error.
    pub fn header(&mut self, title: &str, detail: &str) -> Result<(), String> {
        self.fill(
            Rect::new(
                0,
                0,
                self.width.unsigned_abs(),
                self.header_height().unsigned_abs(),
            ),
            PANEL,
        )?;
        self.text(title, 8, 5, self.width.saturating_sub(16), ACCENT)?;
        self.text(
            detail,
            8,
            5_i32.saturating_add(self.line()),
            self.width.saturating_sub(16),
            MUTED,
        )
    }
    /// Header whose second line is a filesystem path. The path is trimmed from
    /// the left so the file name stays visible in deep directories.
    /// # Errors
    /// Reports a renderer error.
    pub fn header_path(&mut self, title: &str, path: &str) -> Result<(), String> {
        let columns = self.columns(self.width.saturating_sub(16));
        let shown = tail(path, columns);
        self.header(title, &shown)
    }
    /// Whole characters that fit in `width` pixels at this session's scale.
    #[must_use]
    pub fn columns(&self, width: i32) -> usize {
        usize::try_from(width.max(0).checked_div(self.cell()).unwrap_or(0)).unwrap_or(0)
    }
    #[must_use]
    pub fn button_rect(&self, index: usize, count: usize) -> Rect {
        button_bounds(self.height, self.footer_height(), self.width, index, count)
    }
    /// # Errors
    /// Reports a renderer error.
    pub fn buttons(&mut self, labels: &[&str], focus: Option<usize>) -> Result<(), String> {
        for (index, label) in labels.iter().enumerate() {
            let rect = self.button_rect(index, labels.len());
            theme::card(&mut self.canvas, rect, focus == Some(index))?;
            self.text(
                label,
                rect.x().saturating_add(6),
                rect.y().saturating_add(
                    self.footer_height()
                        .saturating_sub(8_i32.saturating_mul(self.scale))
                        / 2_i32,
                ),
                i32::try_from(rect.width())
                    .unwrap_or(0_i32)
                    .saturating_sub(12),
                if focus == Some(index) { ACCENT } else { TEXT },
            )?;
        }
        Ok(())
    }
    #[must_use]
    pub fn button_at(&self, x: i32, y: i32, count: usize) -> Option<usize> {
        (0..count).find(|&i| {
            let rect = self.button_rect(i, count);
            rect.contains_point((x, y))
                && self
                    .released_from
                    .is_none_or(|start| rect.contains_point(start))
        })
    }
    /// A released row must match the pressed row; crossing adjacent rows never activates one.
    #[must_use]
    pub fn row_at(&self, y: i32, top: i32, height: i32, count: usize) -> Option<usize> {
        if y < top || height <= 0_i32 {
            return None;
        }
        let row_value = y.checked_sub(top)?.checked_div(height)?;
        let row = usize::try_from(row_value).ok()?;
        (row < count
            && self.released_from.is_none_or(|(_, start)| {
                start >= top
                    && start
                        .checked_sub(top)
                        .and_then(|offset| offset.checked_div(height))
                        == Some(row_value)
            }))
        .then_some(row)
    }
    pub fn present(&mut self) {
        self.presentation.present(&mut self.canvas);
    }
    /// Block until an event arrives. There is no idle timeout.
    /// # Errors
    /// Reports a display resize error.
    pub fn wait(&mut self) -> Result<Input, String> {
        loop {
            let event = self.events.wait_event();
            let input = self.translate(event)?;
            if input != Input::Ignore {
                return Ok(input);
            }
        }
    }
    /// An advisory close may only consume an idle wake, never queued user input.
    /// A failed queue query or a foreground window conservatively vetoes closing.
    #[must_use]
    pub fn idle_for_auto_close(&self, input: &Input) -> bool {
        *input == Input::Wake
            && !self.canvas.window().has_input_focus()
            && self
                .sdl
                .event()
                .is_ok_and(|events| events.peek_events::<Vec<Event>>(1).is_empty())
    }
    /// Drain a bounded batch in applications with high output rates.
    /// # Errors
    /// Reports a display resize error.
    pub fn poll(&mut self) -> Result<Option<Input>, String> {
        self.events
            .poll_event()
            .map(|e| self.translate(e))
            .transpose()
    }
    /// Modifiers belonging to the most recently delivered text input.
    #[must_use]
    pub const fn text_modifiers(&self) -> Mod {
        self.keyboard.text_modifiers()
    }
    #[allow(
        clippy::rest_pattern_accessible_field,
        clippy::wildcard_enum_match_arm,
        reason = "SDL dispatch ignores timestamps and device fields not used by this UI, and events outside its supported input/window set"
    )]
    fn translate(&mut self, mut event: Event) -> Result<Input, String> {
        self.keyboard.event(&mut event);
        if self
            .focus_requested
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            self.canvas.window_mut().raise();
        }
        Ok(match event {
            Event::Quit { .. }
            | Event::Window {
                win_event: WindowEvent::Close,
                ..
            } => Input::Close,
            Event::KeyDown {
                keycode: Some(key),
                keymod,
                ..
            } => Input::Key(key, keymod),
            Event::TextInput { text, .. } => Input::Text(text),
            Event::MouseWheel { y, .. } => Input::Scroll(y),
            Event::MouseButtonDown {
                which,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if which != u32::MAX => {
                self.press = Some((i64::from(which), x, y));
                Input::Ignore
            }
            Event::MouseButtonUp {
                which,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if which != u32::MAX => self.release(i64::from(which), x, y),
            Event::FingerDown {
                finger_id, x, y, ..
            } if x.is_finite() && y.is_finite() => {
                let (pixel_x, pixel_y) = self.touch(x, y);
                self.press = Some((finger_id, pixel_x, pixel_y));
                Input::Ignore
            }
            Event::FingerUp {
                finger_id, x, y, ..
            } if x.is_finite() && y.is_finite() => {
                let (pixel_x, pixel_y) = self.touch(x, y);
                self.release(finger_id, pixel_x, pixel_y)
            }
            Event::Window {
                win_event: WindowEvent::SizeChanged(..) | WindowEvent::Resized(..),
                ..
            } => {
                self.resize()?;
                self.press = None;
                Input::Resize
            }
            Event::Window {
                win_event: WindowEvent::FocusLost,
                ..
            } => {
                self.press = None;
                Input::Ignore
            }
            Event::Window {
                win_event: WindowEvent::Exposed | WindowEvent::FocusGained,
                ..
            }
            | Event::RenderTargetsReset { .. } => Input::Resize,
            Event::RenderDeviceReset { .. } => {
                self.font = crate::font::Atlas::new(self.creator)?;
                Input::Resize
            }
            Event::User { .. } => Input::Wake,
            _ => Input::Ignore,
        })
    }
    // SDL finger coordinates are normalized; clamp before the bounded pixel conversion.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::as_conversions,
        reason = "Normalized finite touch positions are clamped to a display bounded by SDL"
    )]
    fn touch(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (f64::from(x).clamp(0., 1.) * f64::from(self.width)) as i32,
            (f64::from(y).clamp(0., 1.) * f64::from(self.height)) as i32,
        )
    }
    const fn release(&mut self, id: i64, x: i32, y: i32) -> Input {
        match self.press.take() {
            Some((old, px, py)) if old == id && x.abs_diff(px) < 12 && y.abs_diff(py) < 12 => {
                self.released_from = Some((px, py));
                Input::Click(x, y)
            }
            _ => Input::Ignore,
        }
    }
    /// # Errors
    /// Refuses overwrites and reports screenshot encoding/I/O errors.
    pub fn finish_preview(&mut self, options: &Options) -> Result<bool, String> {
        // Accelerated backbuffers may be invalidated by present. Read first.
        if let Some(path) = &options.screenshot {
            save_screenshot(&self.canvas, path)?;
            self.present();
            return Ok(true);
        }
        self.present();
        Ok(options.smoke)
    }
    /// Shared modal selection; index zero is always the safe default.
    /// # Errors
    /// Reports SDL/render errors.
    pub fn choose(&mut self, title: &str, body: &str, labels: &[&str]) -> Result<usize, String> {
        // The contract names index zero as the safe default; with no labels there
        // is nothing to select, and the navigation arithmetic would divide by zero.
        if labels.is_empty() {
            return Ok(0);
        }
        let mut selected = 0;
        let mut offset = 0;
        let lines = wrap(body, self.columns(self.width.saturating_sub(16_i32)).max(1));
        loop {
            self.clear();
            self.header(title, "Arrows / Tab select   Enter activates")?;
            let top = self.header_height().saturating_add(8);
            for (i, line) in lines.iter().skip(offset).take(self.rows(top)).enumerate() {
                self.text(
                    line,
                    8,
                    top.saturating_add(
                        i32::try_from(i)
                            .unwrap_or(0_i32)
                            .saturating_mul(self.line()),
                    ),
                    self.width.saturating_sub(16),
                    TEXT,
                )?;
            }
            self.buttons(labels, Some(selected))?;
            self.present();
            match self.wait()? {
                Input::Close | Input::Key(Keycode::Escape, _) => return Ok(0),
                Input::Key(Keycode::Return | Keycode::KpEnter, _) => return Ok(selected),
                Input::Key(Keycode::Tab | Keycode::Right, _) => {
                    selected = selected
                        .checked_add(1)
                        .filter(|index| *index < labels.len())
                        .unwrap_or(0);
                }
                Input::Key(Keycode::Left, _) => {
                    selected = selected
                        .checked_sub(1)
                        .unwrap_or_else(|| labels.len().saturating_sub(1));
                }
                Input::Key(Keycode::Down | Keycode::PageDown, _) | Input::Scroll(-1) => {
                    offset = offset.saturating_add(1).min(lines.len().saturating_sub(1));
                }
                Input::Key(Keycode::Up | Keycode::PageUp, _) | Input::Scroll(1) => {
                    offset = offset.saturating_sub(1);
                }
                Input::Click(x, y) => {
                    if let Some(i) = self.button_at(x, y, labels.len()) {
                        return Ok(i);
                    }
                }
                Input::Key(_, _)
                | Input::Text(_)
                | Input::Scroll(_)
                | Input::Resize
                | Input::Wake
                | Input::Ignore => {}
            }
        }
    }
    /// # Errors
    /// Reports SDL/render errors.
    pub fn error(&mut self, error: &str) -> Result<(), String> {
        self.choose("Unable to complete", error, &["Back"])
            .map(|_| ())
    }
    /// Small bounded UTF-8 path/name field shared by file dialogs.
    /// # Errors
    /// Reports SDL/render errors.
    pub fn prompt(&mut self, title: &str, initial: &str) -> Result<Option<String>, String> {
        let mut value = initial.to_owned();
        let mut focus: usize = 0;
        loop {
            self.clear();
            self.header(title, "Type a path or name   Tab selects controls")?;
            let field = Rect::new(
                8,
                self.header_height().saturating_add(12),
                self.width.saturating_sub(16).unsigned_abs(),
                28_i32.saturating_mul(self.scale).unsigned_abs(),
            );
            let visible = self.columns(self.width.saturating_sub(24)).max(1);
            let count = value.chars().count();
            let tail: String = value.chars().skip(count.saturating_sub(visible)).collect();
            self.fill(field, if focus == 0 { SELECTED } else { PANEL })?;
            // The value sits on the field's own vertical centre, with the same
            // inset the caret uses horizontally.
            let text_y = self.header_height().saturating_add(12).saturating_add(
                i32::try_from(field.height())
                    .unwrap_or(0_i32)
                    .saturating_sub(8_i32.saturating_mul(self.scale))
                    / 2_i32,
            );
            self.text(&tail, 12, text_y, self.width.saturating_sub(24), TEXT)?;
            self.buttons(&["Cancel", "OK"], focus.checked_sub(1))?;
            self.present();
            match self.wait()? {
                Input::Close | Input::Key(Keycode::Escape, _) => return Ok(None),
                Input::Key(Keycode::Tab, _) => focus = focus.saturating_add(1) % 3,
                Input::Key(Keycode::Return | Keycode::KpEnter, _) => {
                    return Ok((focus != 1).then_some(value));
                }
                Input::Key(Keycode::Backspace, _) if focus == 0 => {
                    let _removed_character = value.pop();
                }
                Input::Key(Keycode::A, m) if ctrl(m) && focus == 0 => value.clear(),
                Input::Text(text)
                    if focus == 0
                        && value
                            .len()
                            .checked_add(text.len())
                            .is_some_and(|length| length <= 4096) =>
                {
                    value.extend(text.chars().filter(|c| !c.is_control()));
                }
                Input::Click(x, y) => {
                    if let Some(i) = self.button_at(x, y, 2) {
                        return Ok((i == 1).then_some(value));
                    }
                    focus = 0;
                }
                Input::Key(_, _)
                | Input::Text(_)
                | Input::Scroll(_)
                | Input::Resize
                | Input::Wake
                | Input::Ignore => {}
            }
        }
    }
}
/// Encode the current backbuffer and publish a new BMP atomically.
/// # Errors
/// Reports SDL, allocation, path, and I/O failures; refuses existing destinations.
pub fn save_screenshot(canvas: &Canvas<Window>, path: &std::path::Path) -> Result<(), String> {
    let destination = crate::files::checked_path(path).map_err(|error| error.to_string())?;
    let (width, height) = canvas.output_size()?;
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        return Err("Screenshot dimensions exceed supported bounds".into());
    }
    let pitch = width.checked_mul(3).ok_or("Screenshot pitch overflow")?;
    let capacity = width
        .checked_mul(height)
        .and_then(|area| area.checked_mul(4))
        .and_then(|size| size.checked_add(4096))
        .ok_or("Screenshot size overflow")?;
    let mut pixels = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
    let surface = sdl2::surface::Surface::from_data(
        &mut pixels,
        width,
        height,
        pitch,
        PixelFormatEnum::RGB24,
    )?;
    let mut bytes = Vec::new();
    let buffer_capacity =
        usize::try_from(capacity).map_err(|error| format!("Screenshot size: {error}"))?;
    bytes
        .try_reserve_exact(buffer_capacity)
        .map_err(|error| format!("Screenshot allocation: {error}"))?;
    bytes.resize(buffer_capacity, 0);
    let mut rw = sdl2::rwops::RWops::from_bytes_mut(&mut bytes)?;
    surface.save_bmp_rw(&mut rw)?;
    let length = usize::try_from(rw.stream_position().map_err(|e| e.to_string())?)
        .map_err(|error| format!("Screenshot length: {error}"))?;
    drop(rw);
    let (temporary, mut output) =
        crate::files::Temporary::file(destination.parent().ok_or("Missing screenshot directory")?)
            .map_err(|error| error.to_string())?;
    output
        .write_all(bytes.get(..length).ok_or("Invalid screenshot length")?)
        .map_err(|e| e.to_string())?;
    drop(output);
    crate::files::rename_new(&temporary.path, &destination).map_err(|error| error.to_string())
}

#[must_use]
pub const fn ctrl(mods: Mod) -> bool {
    mods.intersects(Mod::LCTRLMOD.union(Mod::RCTRLMOD))
}
#[must_use]
pub const fn shift(mods: Mod) -> bool {
    mods.intersects(Mod::LSHIFTMOD.union(Mod::RSHIFTMOD))
}

/// Whole text rows that fit between `top` and the footer. Every screen that
/// reserves rows for a status line or a field row derives them here, so a row
/// can never be drawn under the footer.
#[must_use]
pub fn text_rows(height: i32, footer: i32, line: i32, top: i32) -> usize {
    if line <= 0_i32 {
        return 1;
    }
    usize::try_from(
        height
            .saturating_sub(footer)
            .saturating_sub(top)
            .checked_div(line)
            .unwrap_or(0),
    )
    .unwrap_or(1)
    .max(1)
}

/// Equal-width footer controls. The last column absorbs the remainder of a
/// width that does not divide evenly, so the controls always tile the row
/// without overlapping or leaving a gap at the right edge.
#[must_use]
pub fn button_bounds(height: i32, footer: i32, width: i32, index: usize, count: usize) -> Rect {
    let columns = i32::try_from(count).unwrap_or(1_i32).max(1_i32);
    let column = width.checked_div(columns).unwrap_or(0_i32);
    let x = i32::try_from(index).unwrap_or(0_i32).saturating_mul(column);
    let w = if i32::try_from(index).unwrap_or(0_i32).saturating_add(1_i32) == columns {
        width.saturating_sub(x)
    } else {
        column
    };
    Rect::new(
        x,
        height.saturating_sub(footer),
        w.unsigned_abs(),
        footer.unsigned_abs(),
    )
}

/// Keep the end of a path or name visible when it does not fit a line. A
/// budget too narrow for a marker keeps its last visible characters, and the
/// result never exceeds `columns` characters.
#[must_use]
pub fn tail(value: &str, columns: usize) -> String {
    let count = value.chars().count();
    if count <= columns {
        return value.to_owned();
    }
    if columns <= 3 {
        return value.chars().skip(count.saturating_sub(columns)).collect();
    }
    let mut out = String::from("...");
    out.extend(
        value
            .chars()
            .skip(count.saturating_sub(columns.saturating_sub(3))),
    );
    out
}

/// SDL's safe event sender transports no raw pointers or heap event payloads.
/// # Errors
/// Returns SDL queue errors. Callers retain their bounded data until consumed.
pub fn wake(sender: &sdl2::event::EventSender) -> Result<(), String> {
    #[allow(
        clippy::as_conversions,
        reason = "SDL's repr(u32) event discriminant is the API's required user-event type code"
    )]
    let type_ = sdl2::event::EventType::User as u32;
    sender.push_event(Event::User {
        timestamp: 0,
        window_id: 0,
        type_,
        code: 0,
        data1: std::ptr::null_mut(),
        data2: std::ptr::null_mut(),
    })
}

pub(crate) fn hint(name: &str, value: &str) {
    if !sdl2::hint::set(name, value) {
        eprintln!("SDL rejected hint {name}={value:?}");
    }
}

fn wrap(body: &str, columns: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut count = 0_usize;
    for ch in body.chars().take(16 * 1024) {
        if ch == '\n' || count == columns {
            if lines.len() == 256 {
                break;
            }
            lines.push(String::new());
            count = 0;
            if ch == '\n' {
                continue;
            }
        }
        if let Some(line) = lines.last_mut() {
            line.push(ch);
            count = count.saturating_add(1);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared footer controls tile the row exactly and the shared row count
    /// never places a text line under the footer, at any supported size.
    #[test]
    fn shared_control_geometry_never_overlaps_its_own_chrome() {
        for (height, top, line) in [
            (200_i32, 40_i32, 12_i32),
            (272_i32, 40_i32, 12_i32),
            (480_i32, 56_i32, 24_i32),
        ] {
            let footer = 28_i32;
            let rows = text_rows(height, footer, line, top);
            assert!(rows >= 1);
            assert!(
                top + i32::try_from(rows).unwrap_or(0_i32) * line <= height - footer,
                "{height}: rows reach the footer"
            );
            assert!(
                top + i32::try_from(rows + 1).unwrap_or(0_i32) * line > height - footer,
                "{height}: a row was dropped"
            );
        }
        assert_eq!(text_rows(272, 28, 0, 40), 1);
        for (width, height) in [
            (320_i32, 200_i32),
            (480_i32, 272_i32),
            (800_i32, 480_i32),
            (1_280_i32, 720_i32),
        ] {
            for count in 1..=6_usize {
                let mut previous_end = 0_i32;
                for index in 0..count {
                    let rect = button_bounds(height, 28, width, index, count);
                    assert_eq!(rect.y(), height - 28_i32, "{width}x{height}/{count}");
                    assert_eq!(rect.height(), 28, "{width}x{height}/{count}");
                    let x = rect.x();
                    let w = i32::try_from(rect.width()).unwrap_or(0_i32);
                    assert!(w >= 1_i32, "{width}x{height}/{count}");
                    assert!(x >= previous_end, "{width}x{height}/{count}: overlap");
                    assert!(x + w <= width, "{width}x{height}/{count}: past the edge");
                    previous_end = x + w;
                }
                // The controls cover the row: nothing narrower than one column.
                assert!(
                    previous_end > width - i32::try_from(count).unwrap_or(1_i32),
                    "{width}x{height}/{count} leaves {} empty",
                    width - previous_end
                );
            }
        }
    }

    fn key(ui: &Ui, keycode: Keycode) -> Result<(), String> {
        ui.sdl.event()?.push_event(Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        })
    }
    #[test]
    fn dialogs_pointer_guards_and_private_ipc_work_at_native_sizes() -> Result<(), String> {
        for size in [(480, 272), (800, 480), (1280, 720)] {
            hint("SDL_VIDEODRIVER", "dummy");
            let session = Session::new(
                "Native controls test",
                &Options {
                    size: Some(size),
                    ..Options::default()
                },
            )?;
            let creator = session.canvas.texture_creator();
            let mut ui = Ui::new(session, &creator)?;
            crate::theme::tests::primitive_pixels(&mut ui.canvas)?;
            crate::font::tests::pixel_parity(&mut ui.canvas, &mut ui.font)?;
            assert_eq!(
                ui.translate(Event::RenderDeviceReset { timestamp: 0 })?,
                Input::Resize
            );
            crate::font::tests::pixel_parity(&mut ui.canvas, &mut ui.font)?;
            key(&ui, Keycode::Return)?;
            assert_eq!(
                ui.choose("Delete?", "Keep the original", &["Cancel", "Delete"])?,
                0
            );
            key(&ui, Keycode::Tab)?;
            key(&ui, Keycode::Return)?;
            assert_eq!(
                ui.choose("Delete?", "Explicit selection", &["Cancel", "Delete"])?,
                1
            );
            let boundary = ui.width / 2_i32;
            let y = ui.height - 10_i32;
            ui.press = Some((1, boundary - 1_i32, y));
            assert!(matches!(ui.release(1, boundary + 1, y), Input::Click(..)));
            assert_eq!(ui.button_at(boundary + 1, y, 2), None);
            ui.press = Some((1, 10_i32, 49_i32));
            assert_eq!(ui.release(1, 10, 51), Input::Click(10, 51));
            assert_eq!(ui.row_at(51, 30, 20, 8), None);
            ui.press = Some((1, 10_i32, 51_i32));
            assert_eq!(ui.release(2, 10, 51), Input::Ignore);
            assert_eq!(ui.press, None);
            crate::ipc::test_private_inbox(&ui.sdl)?;
            while ui.events.poll_event().is_some() {}
            assert!(!ui.idle_for_auto_close(&Input::Text("pending edit".into())));
            key(&ui, Keycode::A)?;
            assert!(!ui.idle_for_auto_close(&Input::Wake));
            while ui.events.poll_event().is_some() {}
            key(&ui, Keycode::Tab)?;
            // An empty label list must return the safe default instead of dividing by zero.
            assert_eq!(ui.choose("Empty", "No labels", &[])?, 0);
            while ui.events.poll_event().is_some() {}
        }
        assert!(wrap(&"x".repeat(100_000), 10).len() <= 256);
        Ok(())
    }
}
