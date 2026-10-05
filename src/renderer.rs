use vitrallis_native::theme;
mod app_center;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
pub mod performance;
pub use vitrallis_native::renderer as backend;
mod shortcuts;
mod system;
use crate::{
    app::AppEntry,
    launcher::Launcher,
    layout::{Layout, Rect},
    process::AppState,
};
use sdl2::{
    pixels::{Color, PixelFormatEnum},
    render::{Canvas, Texture, TextureCreator},
    surface::Surface,
    video::{Window, WindowContext},
};

pub struct Screen<'a> {
    canvas: Canvas<Window>,
    presentation: backend::PresentationClock,
    font: vitrallis_native::font::Atlas<'a>,
    creator: &'a TextureCreator<WindowContext>,
    center_icons: Vec<(Box<[u8]>, Texture<'a>)>,
    /// Single-entry cache for the shortcut editor's icon preview. `None` keeps
    /// a failed decode cached for the same bytes.
    preview: Option<(Vec<u8>, Option<Texture<'a>>)>,
}
impl<'a> Screen<'a> {
    pub fn new(
        canvas: Canvas<Window>,
        creator: &'a TextureCreator<WindowContext>,
    ) -> Result<Self, String> {
        let presentation = backend::PresentationClock::new(&canvas);
        Ok(Self {
            canvas,
            presentation,
            font: vitrallis_native::font::Atlas::new(creator)?,
            creator,
            center_icons: Vec::new(),
            preview: None,
        })
    }
    pub fn present(&mut self) {
        self.presentation.present(&mut self.canvas);
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.font = vitrallis_native::font::Atlas::new(self.creator)?;
        self.center_icons.clear();
        self.preview = None;
        Ok(())
    }
    fn center_icon(&mut self, pixels: &[u8], bounds: Rect) -> Result<(), String> {
        if pixels.len() != 32 * 32 * 4 {
            return Err("App Center icon must be 32x32 RGBA".into());
        }
        let index = if let Some(index) = self
            .center_icons
            .iter()
            .position(|(key, _)| key.as_ref() == pixels)
        {
            index
        } else {
            // Bounded independently of catalogue size: 512 KiB textures + keys.
            if self.center_icons.len() == 128 {
                drop(self.center_icons.remove(0));
            }
            let mut texture = self
                .creator
                .create_texture_static(PixelFormatEnum::RGBA32, 32, 32)
                .map_err(|e| e.to_string())?;
            texture.set_blend_mode(sdl2::render::BlendMode::Blend);
            texture.set_scale_mode(sdl2::render::ScaleMode::Nearest);
            texture
                .update(None, pixels, 32 * 4)
                .map_err(|e| e.to_string())?;
            #[cfg(test)]
            performance::count(|c| c.uploads = c.uploads.saturating_add(1));
            self.center_icons.push((pixels.into(), texture));
            self.center_icons.len().saturating_sub(1)
        };
        let (_, texture) = self
            .center_icons
            .get(index)
            .ok_or("App Center icon cache entry missing")?;
        self.canvas.copy(texture, None, rect(bounds)?)
    }
    /// Draws the shortcut editor's chosen icon, decoding and uploading it only
    /// when the encoded bytes change. A PNG/BMP decode plus texture upload on
    /// every rendered frame costs tens of milliseconds on `PocketCHIP`-class
    /// hardware; the editor keeps the same bytes while the page is open, so a
    /// single cached entry is enough. Returns `Ok(false)` when the bytes cannot
    /// be decoded, so the caller can draw its placeholder instead; the failed
    /// decode is cached too, and is not retried for unchanged bytes.
    fn icon_preview(&mut self, bytes: &[u8], bounds: Rect) -> Result<bool, String> {
        let changed = self
            .preview
            .as_ref()
            .is_none_or(|(key, _)| key.as_slice() != bytes);
        if changed {
            let texture = match decode_icon(bytes) {
                Ok(surface) => Some(
                    self.creator
                        .create_texture_from_surface(&surface)
                        .map_err(|e| e.to_string())?,
                ),
                Err(_) => None,
            };
            self.preview = Some((bytes.to_vec(), texture));
        }
        let Some((_, Some(texture))) = &self.preview else {
            return Ok(false);
        };
        self.canvas.copy(texture, None, rect(bounds)?)?;
        Ok(true)
    }
}
impl std::ops::Deref for Screen<'_> {
    type Target = Canvas<Window>;
    fn deref(&self) -> &Self::Target {
        &self.canvas
    }
}
impl std::ops::DerefMut for Screen<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.canvas
    }
}
fn rect(r: Rect) -> Result<sdl2::rect::Rect, String> {
    Ok(sdl2::rect::Rect::new(
        r.x,
        r.y,
        u32::try_from(r.w).map_err(|error| format!("negative rectangle width: {error}"))?,
        u32::try_from(r.h).map_err(|error| format!("negative rectangle height: {error}"))?,
    ))
}
fn fill(canvas: &mut Screen, r: Rect, color: Color) -> Result<(), String> {
    if r.w <= 0_i32 || r.h <= 0_i32 {
        return Ok(());
    }
    if canvas.draw_color() != color {
        canvas.set_draw_color(color);
    }
    canvas.fill_rect(rect(r)?)
}
fn card(canvas: &mut Screen, bounds: Rect, selected: bool) -> Result<(), String> {
    theme::card(canvas, rect(bounds)?, selected)
}

fn progress(canvas: &mut Screen, bounds: Rect, filled: i32, warning: bool) -> Result<(), String> {
    theme::progress(canvas, rect(bounds)?, filled.max(0).unsigned_abs(), warning)
}

/// Glyph advance in pixels for one shell text scale. The font atlas is a fixed
/// 8x8 cell, so every measurement is a constant multiplication; no font query,
/// metric table or allocation is involved.
#[must_use]
pub const fn advance(scale: i32) -> i32 {
    theme::CELL.saturating_mul(scale)
}

/// Whole characters that fit in `width` pixels at `scale`. This is the single
/// measurement used by rendering and by the overflow policy below.
#[must_use]
pub fn fit_columns(width: i32, scale: i32) -> usize {
    if scale <= 0_i32 {
        return 0;
    }
    usize::try_from(width.checked_div(advance(scale)).unwrap_or(0_i32)).unwrap_or(0)
}

/// Characters reserved for the explicit truncation marker.
const ELLIPSIS: usize = 3;

/// Legend for the on-screen space key. The full word is the clearest label, but
/// a dense key grid cannot always hold it at the larger text scales; the short
/// conventional form keeps the key readable instead of ellipsizing a legend.
#[must_use]
pub fn space_legend(width: i32, scale: i32) -> &'static str {
    if fit_columns(width, scale) >= 5 {
        "Space"
    } else {
        "Spc"
    }
}

/// Typographic punctuation the 8x8 atlas has no glyph for. Shell labels are
/// authored text, so mapping it onto the ASCII the atlas does contain keeps a
/// real glyph on screen instead of the unsupported-character fallback. Document
/// and terminal content never passes through here.
const fn display_char(character: char) -> char {
    match character {
        '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => '\'',
        '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' => '"',
        '\u{2013}' | '\u{2014}' | '\u{2015}' | '\u{2022}' => '-',
        '\u{2026}' => '.',
        '\u{a0}' => ' ',
        other => other,
    }
}

/// Shorten `value` to `columns` characters with a trailing ellipsis when it does
/// not fit. Returns the visible count and whether the marker is drawn. The probe
/// stops one character past the limit, so measurement stays bounded by the
/// rectangle and never scans an unbounded string.
fn fit_visible(value: &str, columns: usize) -> (usize, bool) {
    if columns == 0 {
        return (0, false);
    }
    let shortened = columns > ELLIPSIS && value.chars().nth(columns).is_some();
    if shortened {
        (columns.saturating_sub(ELLIPSIS), true)
    } else {
        (value.chars().take(columns).count(), false)
    }
}

/// Draws one glyph cell at the caller's existing integer position and scale.
fn glyph(
    canvas: &mut Screen,
    character: char,
    x: i32,
    y: i32,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    #[cfg(test)]
    performance::count(|c| c.glyphs = c.glyphs.saturating_add(1));
    #[cfg(test)]
    performance::count(|c| {
        c.text_operations = c
            .text_operations
            .saturating_add(u64::from(!character.is_ascii_control() && character != ' '));
    });
    let glyph_character = if character.is_ascii() { character } else { '?' };
    canvas.font.draw(
        &mut canvas.canvas,
        glyph_character,
        rect(Rect {
            x,
            y,
            w: theme::CELL.saturating_mul(scale),
            h: theme::CELL.saturating_mul(scale),
        })?,
        color,
    )
}

/// A caller-provided text rectangle must be able to contain the glyph cell it
/// centers. Shorter rectangles would place glyph rows above the rectangle, on
/// top of whatever border or label lives there.
#[cfg(debug_assertions)]
fn assert_cell_fits(bounds: Rect, scale: i32) {
    debug_assert!(
        bounds.h >= theme::CELL.saturating_mul(scale) || bounds.h <= 0_i32,
        "{}px text rectangle cannot hold a {}px glyph cell",
        bounds.h,
        theme::CELL.saturating_mul(scale)
    );
}

/// Single-line text, centered inside `bounds`. Text that does not fit is
/// shortened with a trailing ellipsis instead of being cut through a glyph, and
/// control characters are dropped rather than drawn as a fallback.
/// # Errors
/// Reports a renderer error.
pub fn text(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    draw_line(canvas, value, bounds, scale, color, true)
}

/// Single-line text, left-aligned on `bounds.x`. The explicit overflow policy is
/// the same trailing ellipsis as [`text`].
/// # Errors
/// Reports a renderer error.
pub fn text_left(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    draw_line(canvas, value, bounds, scale, color, false)
}

/// One shortened single-line run. `centered` places it in the middle of the
/// rectangle; left alignment keeps `bounds.x` as the text origin.
fn draw_line(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
    centered: bool,
) -> Result<(), String> {
    if scale <= 0_i32 {
        return Err("invalid text scale".into());
    }
    let limit = fit_columns(bounds.w, scale);
    if limit == 0 {
        return Ok(());
    }
    #[cfg(debug_assertions)]
    assert_cell_fits(bounds, scale);
    let (shown, shortened) = fit_visible(value, limit);
    let count = if shortened { limit } else { shown };
    let glyph_count = i32::try_from(count).map_err(|error| format!("text too long: {error}"))?;
    let cell = advance(scale);
    let mut x = if centered {
        bounds
            .x
            .saturating_add(bounds.w.saturating_sub(glyph_count.saturating_mul(cell)) / 2_i32)
    } else {
        bounds.x
    };
    let y = bounds
        .y
        .saturating_add(bounds.h.saturating_sub(cell) / 2_i32);
    for character in value.chars().take(shown) {
        glyph(canvas, display_char(character), x, y, scale, color)?;
        x = x.saturating_add(cell);
    }
    if shortened {
        for _ in 0..ELLIPSIS {
            glyph(canvas, '.', x, y, scale, color)?;
            x = x.saturating_add(cell);
        }
    }
    Ok(())
}

fn chrome(canvas: &mut Screen, layout: &Layout) -> Result<(), String> {
    fill(
        canvas,
        Rect {
            x: 0,
            w: i32::from(layout.width),
            ..layout.title
        },
        theme::BACKGROUND,
    )?;
    fill(
        canvas,
        Rect {
            x: 0,
            w: i32::from(layout.width),
            ..layout.footer
        },
        theme::BACKGROUND,
    )?;
    Ok(())
}

/// Greedy word wrap to `columns` characters. Explicit line breaks start a new
/// line; empty lines are dropped. A word longer than the line is split at the
/// column boundary rather than overflowing it. The line count is bounded so a
/// pathological description cannot allocate without limit.
#[must_use]
pub fn wrap_words(value: &str, columns: usize) -> Vec<String> {
    const MAX_LINES: usize = 256;
    let line_columns = columns.max(1);
    let mut lines = Vec::new();
    for paragraph in value.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty()
                && line
                    .chars()
                    .count()
                    .saturating_add(1)
                    .saturating_add(word.chars().count())
                    > line_columns
            {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            for character in word.chars() {
                if line.chars().count() == line_columns {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(character);
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        if lines.len() >= MAX_LINES {
            break;
        }
    }
    lines.truncate(MAX_LINES);
    lines
}
mod artwork;
pub use artwork::{Artwork, artwork};

pub fn decode_icon(bytes: &[u8]) -> Result<Surface<'static>, String> {
    #[cfg(test)]
    performance::count(|c| c.decodes = c.decodes.saturating_add(1));
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let mut decoder = png::Decoder::new_with_limits(
            std::io::Cursor::new(bytes),
            png::Limits {
                bytes: 8 * 1024 * 1024,
            },
        );
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let info = reader.info();
        if !(1..=512).contains(&info.width)
            || !(1..=512).contains(&info.height)
            || bytes.len() > 1024 * 1024
        {
            return Err("PNG icon exceeds 512x512 or 1 MiB".into());
        }
        let size = reader
            .output_buffer_size()
            .ok_or("PNG output buffer exceeds addressable memory")?;
        let mut pixels = vec![0; size];
        let frame = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
        let channels = frame.color_type.samples();
        let frame_bytes = pixels
            .get(..frame.buffer_size())
            .ok_or("Invalid decoded PNG length")?;
        let capacity = frame_bytes
            .len()
            .checked_div(channels)
            .and_then(|count| count.checked_mul(4))
            .ok_or("Invalid PNG pixel buffer size")?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(capacity)
            .map_err(|error| format!("PNG allocation: {error}"))?;
        let chunks = frame_bytes.chunks_exact(channels);
        if !chunks.remainder().is_empty() {
            return Err("Incomplete PNG pixel channels".into());
        }
        for pixel in chunks {
            let components = match (frame.color_type, pixel) {
                (png::ColorType::Rgb, &[red, green, blue]) => [red, green, blue, 255],
                (png::ColorType::Rgba, &[red, green, blue, alpha]) => [red, green, blue, alpha],
                (png::ColorType::Grayscale, &[value]) => [value, value, value, 255],
                (png::ColorType::GrayscaleAlpha, &[value, alpha]) => [value, value, value, alpha],
                (png::ColorType::Indexed, _) => return Err("unexpanded PNG palette".into()),
                (
                    png::ColorType::Rgb
                    | png::ColorType::Rgba
                    | png::ColorType::Grayscale
                    | png::ColorType::GrayscaleAlpha,
                    _,
                ) => return Err("Invalid PNG pixel channels".into()),
            };
            rgba.extend_from_slice(&components);
        }
        return Surface::from_data(
            &mut rgba,
            frame.width,
            frame.height,
            frame.width.checked_mul(4).ok_or("PNG pitch overflow")?,
            PixelFormatEnum::RGBA32,
        )?
        .convert_format(PixelFormatEnum::RGBA32);
    }
    validate_bmp(bytes)?;
    Surface::load_bmp_rw(&mut sdl2::rwops::RWops::from_bytes(bytes)?)
}

pub fn render(
    canvas: &mut Screen,
    layout: &Layout,
    state: &Launcher,
    icons: &[Option<Texture<'_>>],
) -> Result<(), String> {
    #[cfg(test)]
    performance::count(|counts| {
        counts.frames = counts.frames.saturating_add(1);
        counts.last_frame = Some(std::time::Instant::now());
    });
    if let Some(power) = state.settings.power_transition {
        return system::power_splash(canvas, layout, power.message());
    }
    if state.app_center.open {
        return app_center::panel(canvas, layout, &state.app_center);
    }
    if state.desktop.open {
        return shortcuts::panel(canvas, layout, &state.desktop);
    }
    render_launcher(canvas, layout, state, icons)
}

const fn settings_title(page: crate::settings::Page) -> &'static str {
    match page {
        crate::settings::Page::Home => "SETTINGS",
        crate::settings::Page::Display => "SETTINGS / DISPLAY & SOUND",
        crate::settings::Page::DateTime => "SETTINGS / DATE & TIME",
        crate::settings::Page::Applications => "SETTINGS / APPLICATIONS",
        crate::settings::Page::Device => "SETTINGS / DEVICE",
        crate::settings::Page::Timezones => "SETTINGS / TIME ZONE",
        crate::settings::Page::Updates => "SETTINGS / SOFTWARE UPDATES",
        crate::settings::Page::Storage => "SETTINGS / STORAGE",
        crate::settings::Page::Wireless => "SETTINGS / WIRELESS",
        crate::settings::Page::Tor => "SETTINGS / TOR",
        crate::settings::Page::TorDetails => "TOR / DETAILS",
        crate::settings::Page::About => "SETTINGS / ABOUT",
    }
}

fn render_launcher(
    canvas: &mut Screen,
    layout: &Layout,
    state: &Launcher,
    icons: &[Option<Texture<'_>>],
) -> Result<(), String> {
    let [red, green, blue] = state.preferences.color;
    canvas.set_draw_color(if state.settings.open {
        theme::BACKGROUND
    } else {
        Color::RGB(red, green, blue)
    });
    canvas.clear();
    if !state.settings.open
        && let Some(Some(wallpaper)) = icons.get(state.apps.len())
    {
        canvas.copy(wallpaper, None, None)?;
    }
    chrome(canvas, layout)?;
    text(
        canvas,
        &if state.settings.open {
            settings_title(state.settings.page).into()
        } else if let Some(name) = state
            .folder
            .as_ref()
            .and_then(|id| state.folders.names.get(id))
        {
            format!("APPS / {name}")
        } else {
            "VITRALLIS".into()
        },
        Rect {
            h: layout.title.h / 2,
            ..layout.title
        },
        layout.text_scale,
        theme::ACCENT,
    )?;
    let system_icons = icons
        .get((state.apps.len()).saturating_add(1_usize)..)
        .unwrap_or(&[]);
    system::status(
        canvas,
        layout,
        &state.settings.status,
        &state.preferences,
        system_icons,
    )?;
    if state.settings.open {
        return system::panel(canvas, layout, &state.settings, system_icons);
    }
    for (bounds, label, enabled) in [
        (layout.previous, "<", state.page_start() > 0),
        (
            layout.next,
            ">",
            ((state.page_start())
                .checked_div(layout.tiles.len())
                .unwrap_or(0_usize))
            .saturating_add(1_usize)
                < state.page_count(),
        ),
    ] {
        text(
            canvas,
            label,
            // Inset the arrow glyphs while keeping the full header touch targets.
            Rect {
                y: (bounds.y).saturating_add((4_i32).saturating_mul(layout.text_scale)),
                h: bounds.h / 2,
                ..bounds
            },
            (layout.text_scale)
                .saturating_add(1_i32)
                .min(bounds.h / 16_i32),
            if enabled {
                theme::ACCENT
            } else {
                theme::DISABLED
            },
        )?;
    }
    for (local, (app, tile)) in state
        .apps
        .iter()
        .skip(state.page_start())
        .zip(&layout.tiles)
        .enumerate()
    {
        let index = (state.page_start()).saturating_add(local);
        render_tile(
            canvas,
            layout,
            app,
            *tile,
            index == state.selected && state.desktop.toolbar.is_none(),
            state.app_state(&app.id),
            icons.get(index).and_then(Option::as_ref),
        )?;
    }
    launcher_status(canvas, layout, state)?;
    error_dialog(canvas, layout, state)?;
    desktop_footer(canvas, layout, state)?;
    Ok(())
}

/// Only active launch progress belongs on Home. Idle diagnostics stay in logs
/// and actionable launch failures retain their dismissible dialog.
fn launcher_status(canvas: &mut Screen, layout: &Layout, state: &Launcher) -> Result<(), String> {
    if state.phase != crate::launcher::Phase::Launching
        || !state.status_notice
        || state.status.is_empty()
        || state.error.is_some()
    {
        return Ok(());
    }
    let right = if state.folder.is_some() {
        layout.folder_back.x
    } else {
        layout.desktop_menu.x
    };
    let bounds = Rect {
        x: layout.footer.x,
        y: (layout.footer.y).saturating_add(
            ((layout.footer.h).saturating_sub((8_i32).saturating_mul(layout.text_scale))) / 2_i32,
        ),
        w: ((right).saturating_sub(layout.footer.x)).saturating_sub(8_i32),
        h: (8_i32).saturating_mul(layout.text_scale),
    };
    if bounds.w <= 0_i32 {
        return Ok(());
    }
    clipped(
        canvas,
        &state.status,
        bounds,
        layout.text_scale,
        theme::ACCENT,
    )
}

/// Left-aligned status text for the launcher footer.
fn clipped(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    text_left(canvas, value, bounds, scale, color)
}

/// Shell labels scale with the display, like every other shell surface. The
/// footer controls are the only chrome that used to stay at the device scale.
fn desktop_footer(canvas: &mut Screen, layout: &Layout, state: &Launcher) -> Result<(), String> {
    let scale = layout.text_scale;
    let focus = state.desktop.toolbar.map(|toolbar| toolbar.bounds(layout));
    card(
        canvas,
        layout.desktop_menu,
        focus == Some(layout.desktop_menu),
    )?;
    text(
        canvas,
        crate::shortcuts::screen::ACTIONS_LABEL,
        layout.desktop_menu,
        scale,
        theme::ACCENT,
    )?;
    if state.folder.is_some() {
        card(
            canvas,
            layout.folder_back,
            focus == Some(layout.folder_back),
        )?;
        text(
            canvas,
            "Back to Apps",
            layout.folder_back,
            scale,
            theme::ACCENT,
        )?;
    }

    Ok(())
}

fn render_tile(
    canvas: &mut Screen,
    layout: &Layout,
    app: &AppEntry,
    tile: Rect,
    selected: bool,
    state: AppState,
    optional_texture: Option<&Texture<'_>>,
) -> Result<(), String> {
    card(canvas, tile, selected)?;
    let icon = Rect {
        x: (tile.x).saturating_add(((tile.w).saturating_sub(layout.icon_size)) / 2_i32),
        y: (tile.y).saturating_add((tile.h) / 12_i32),
        w: layout.icon_size,
        h: layout.icon_size,
    };
    if let Some(texture) = optional_texture {
        let size = texture.query();
        let width = i32::try_from(size.width).map_err(|error| format!("icon width: {error}"))?;
        let height = i32::try_from(size.height).map_err(|error| format!("icon height: {error}"))?;
        let w = icon.w.min(
            ((icon.h).saturating_mul(width))
                .checked_div(height)
                .unwrap_or(0_i32),
        );
        let h = icon.h.min(
            ((icon.w).saturating_mul(height))
                .checked_div(width)
                .unwrap_or(0_i32),
        );
        canvas.copy(
            texture,
            None,
            rect(Rect {
                x: (icon.x).saturating_add(((icon.w).saturating_sub(w)) / 2_i32),
                y: (icon.y).saturating_add(((icon.h).saturating_sub(h)) / 2_i32),
                w,
                h,
            })?,
        )?;
    } else {
        fill(canvas, icon, theme::BORDER)?;
        let mark = if app.unavailable.is_some() { "!" } else { "+" };
        text(
            canvas,
            mark,
            icon,
            (layout.text_scale).saturating_add(1_i32),
            theme::TEXT,
        )?;
    }
    if app.unavailable.is_some() && !app.is_system_settings() {
        text(
            canvas,
            "!",
            Rect {
                x: ((tile.x).saturating_add(tile.w)).saturating_sub(20_i32),
                y: tile.y,
                w: 20,
                h: 20,
            },
            layout.text_scale,
            theme::WARNING,
        )?;
    }
    let label_top = (icon.y).saturating_add(icon.h);
    text(
        canvas,
        &app.name,
        Rect {
            x: (tile.x).saturating_add((theme::SPACE).saturating_mul(layout.text_scale)),
            y: label_top,
            w: (tile.w).saturating_sub(
                ((2_i32).saturating_mul(theme::SPACE)).saturating_mul(layout.text_scale),
            ),
            h: ((tile.y).saturating_add(tile.h)).saturating_sub(label_top),
        },
        layout.text_scale,
        theme::TEXT,
    )?;
    // Unmistakable application state, drawn over the tile corner so the app name
    // always stays readable. One filled chip per state, no animation.
    let (label, ink, surface) = match state {
        AppState::RunningForeground | AppState::RunningBackground => {
            ("RUNNING", theme::BACKGROUND, theme::ACCENT)
        }
        AppState::Launching => ("STARTING", theme::BACKGROUND, theme::VIOLET),
        AppState::Failed => ("FAILED", theme::BACKGROUND, theme::WARNING),
        AppState::Stopped => return Ok(()),
    };
    let bounds = badge_bounds(tile, label, layout.text_scale);
    chip(canvas, bounds, label, layout.text_scale, ink, surface)
}

/// Right-aligned status chip inside `tile`, sized to its label.
fn badge_bounds(tile: Rect, label: &str, scale: i32) -> Rect {
    let width = chip_width(label, scale);
    Rect {
        x: (((tile.x).saturating_add(tile.w)).saturating_sub(width))
            .saturating_sub((3_i32).saturating_mul(scale)),
        y: (tile.y).saturating_add((3_i32).saturating_mul(scale)),
        w: width,
        h: ((10_i32).saturating_mul(scale)).saturating_add(2_i32),
    }
}

/// Width of a compact state chip: one cell per character plus padding.
pub fn chip_width(label: &str, scale: i32) -> i32 {
    (((i32::try_from(label.chars().count()).unwrap_or(0)).saturating_mul(8_i32))
        .saturating_mul(scale))
    .saturating_add((6_i32).saturating_mul(scale))
}

/// Compact opaque state chip: one fill, one underline and one short label.
/// Shared by the desktop tiles and the App Center list so a running app looks
/// identical everywhere.
pub fn chip(
    canvas: &mut Screen,
    bounds: Rect,
    label: &str,
    scale: i32,
    ink: Color,
    surface: Color,
) -> Result<(), String> {
    fill(canvas, bounds, surface)?;
    fill(
        canvas,
        Rect {
            x: bounds.x,
            y: ((bounds.y).saturating_add(bounds.h)).saturating_sub(1_i32),
            w: bounds.w,
            h: 1,
        },
        ink,
    )?;
    text(canvas, label, bounds, scale, ink)
}

fn error_dialog(canvas: &mut Screen, layout: &Layout, state: &Launcher) -> Result<(), String> {
    let Some(error) = &state.error else {
        return Ok(());
    };
    let bounds = Rect {
        x: layout.title.x,
        y: layout.title.h,
        w: layout.title.w,
        h: (layout.footer.y).saturating_sub(layout.title.h),
    };
    fill(canvas, bounds, theme::ERROR_SURFACE)?;
    let line_height = (16_i32).saturating_mul(layout.text_scale);
    text(
        canvas,
        "COULD NOT OPEN APP",
        Rect {
            h: line_height,
            ..bounds
        },
        layout.text_scale,
        theme::WARNING,
    )?;
    let columns = fit_columns((bounds.w).saturating_sub(16_i32), layout.text_scale);
    let rows = usize::try_from(
        ((bounds.h).checked_div(line_height).unwrap_or(0_i32)).saturating_sub(1_i32),
    )
    .map_err(|conversion| format!("dialog rows: {conversion}"))?;
    for (row, value) in wrap_words(error, columns).iter().take(rows).enumerate() {
        text_left(
            canvas,
            value,
            Rect {
                x: (bounds.x).saturating_add(8_i32),
                y: (bounds.y).saturating_add(
                    ((i32::try_from(row)
                        .map_err(|conversion| format!("dialog row: {conversion}"))?)
                    .saturating_add(1_i32))
                    .saturating_mul(line_height),
                ),
                w: (bounds.w).saturating_sub(16_i32),
                h: line_height,
            },
            layout.text_scale,
            theme::TEXT,
        )?;
    }
    Ok(())
}

/// Read the completed backbuffer before `present`, which may invalidate it on
/// accelerated backends. SDL handles pixel conversion and backend orientation.
pub fn screenshot(canvas: &Screen, path: &std::path::Path) -> Result<(), String> {
    vitrallis_native::ui::save_screenshot(&canvas.canvas, path)
}

// Inspect the same bounded bytes passed to SDL, before its native decoder allocates.
fn validate_bmp(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 54 || bytes.len() > 1024 * 1024 || !bytes.starts_with(b"BM") {
        return Err("icon must be a BMP between 54 bytes and 1 MiB".into());
    }
    let word = |offset: usize| -> Result<u32, String> {
        let end = offset.checked_add(4).ok_or("BMP header offset overflow")?;
        let field: [u8; 4] = bytes
            .get(offset..end)
            .ok_or("Incomplete BMP header")?
            .try_into()
            .map_err(|error| format!("BMP header field: {error}"))?;
        Ok(u32::from_le_bytes(field))
    };
    if word(14)? != 40
        || !(1..=512).contains(&word(18)?)
        || !(1..=512).contains(&word(22)?)
        || bytes.get(26..28) != Some([1, 0].as_slice())
        || !matches!(
            u16::from_le_bytes(
                bytes
                    .get(28..30)
                    .ok_or("Incomplete BMP bit depth")?
                    .try_into()
                    .map_err(|error| format!("BMP bit depth: {error}"))?
            ),
            24 | 32
        )
        || word(30)? != 0
    {
        return Err("icon requires a 1..512 x 1..512 uncompressed 24/32-bit Windows BMP".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_and_oversized_bmp_headers_are_rejected_before_decoding() {
        assert!(validate_bmp(b"not a bitmap").is_err());
        let mut header = [0; 54];
        header[..2].copy_from_slice(b"BM");
        header[14..18].copy_from_slice(&40_u32.to_le_bytes());
        header[18..22].copy_from_slice(&32_u32.to_le_bytes());
        header[22..26].copy_from_slice(&32_u32.to_le_bytes());
        header[26] = 1;
        header[28] = 24;
        assert!(validate_bmp(&header).is_ok());
        header[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_bmp(&header).is_err());
    }

    /// Measurement is a constant multiplication because the atlas is a fixed
    /// cell; the overflow policy must agree with it exactly.
    #[test]
    fn measurement_and_overflow_policy_agree_with_the_fixed_cell() {
        for scale in 1_i32..=4_i32 {
            let cell = advance(scale);
            assert_eq!(cell, theme::CELL.saturating_mul(scale));
            assert_eq!(fit_columns(cell * 5, scale), 5);
            assert_eq!(fit_columns(cell * 5 - 1, scale), 4);
            assert_eq!(fit_columns(0, scale), 0);
            assert_eq!(fit_columns(-40, scale), 0);
        }
        // Nothing changes while the value fits.
        assert_eq!(fit_visible("Files", 10), (5, false));
        assert_eq!(fit_visible("", 10), (0, false));
        // Long values lose exactly the marker width.
        assert_eq!(fit_visible("Experimental Rust Application", 17), (14, true));
        // A rectangle too narrow for a marker keeps the leading characters.
        assert_eq!(fit_visible("abcdef", 3), (3, false));
        assert_eq!(fit_visible("abcdef", 2), (2, false));
    }

    #[test]
    fn space_legend_shrinks_only_when_the_key_cannot_hold_the_word() {
        assert_eq!(space_legend(40, 1), "Space");
        assert_eq!(space_legend(39, 1), "Spc");
        assert_eq!(space_legend(70, 2), "Spc");
        assert_eq!(space_legend(80, 2), "Space");
        assert!(fit_columns(40, 1) >= "Space".len());
    }

    #[test]
    fn wrapping_stays_inside_the_column_count() {
        for columns in 1..=12 {
            for value in [
                "one two three four five six seven eight nine ten",
                "supercalifragilisticexpialidocious",
                "first\nsecond\n\nthird",
                "",
            ] {
                for line in wrap_words(value, columns) {
                    assert!(
                        line.chars().count() <= columns,
                        "columns={columns} line={line:?}"
                    );
                }
            }
        }
        assert_eq!(
            wrap_words("a b c", 3),
            vec!["a b".to_string(), "c".to_string()]
        );
        assert_eq!(
            wrap_words("ab\ncd", 8),
            vec!["ab".to_string(), "cd".to_string()]
        );
        assert_eq!(wrap_words("", 8), Vec::<String>::new());
        assert!(wrap_words(&"x".repeat(100_000), 8).len() <= 256);
    }

    /// Every centered and left-aligned label must keep its glyphs inside the
    /// rectangle it was given, at every scale and for any content length.
    #[test]
    fn text_never_draws_outside_its_rectangle() -> Result<(), String> {
        let _guard = crate::test_support::sdl_lock();
        assert!(
            sdl2::hint::set("SDL_VIDEODRIVER", "dummy")
                || sdl2::hint::get("SDL_VIDEODRIVER").as_deref() == Some("dummy"),
            "dummy video must be available for this fixture"
        );
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let window = video
            .window("geometry", 320, 200)
            .hidden()
            .build()
            .map_err(|e| e.to_string())?;
        let raw_canvas = window
            .into_canvas()
            .software()
            .build()
            .map_err(|e| e.to_string())?;
        let creator = raw_canvas.texture_creator();
        let mut canvas = Screen::new(raw_canvas, &creator)?;
        for scale in 1_i32..=3_i32 {
            for value in [
                "",
                "A",
                "Files",
                "Experimental Rust Application",
                "Extremely Long Application Name That Cannot Fit",
                "\u{2019}quoted\u{201d} \u{2014} dash",
            ] {
                for (w, h) in [
                    (16_i32 * scale, 12_i32 * scale),
                    (52_i32 * scale, 10_i32 * scale),
                    (1_i32, 1_i32),
                ] {
                    let bounds = Rect { x: 7, y: 9, w, h };
                    canvas.set_draw_color(theme::BACKGROUND);
                    canvas.clear();
                    text(&mut canvas, value, bounds, scale, theme::TEXT)?;
                    text_left(&mut canvas, value, bounds, scale, theme::ACCENT)?;
                    assert_ink_inside(&mut canvas, bounds, value, scale)?;
                }
            }
        }
        Ok(())
    }

    /// The details body must keep every drawn line above the pinned controls at
    /// every supported size, for any number of fields. A recorded failure keeps
    /// its line even when the field list has to be cut.
    #[test]
    fn app_center_details_never_reach_the_pinned_row() -> Result<(), String> {
        let labels = |index: usize| {
            if index == 7 {
                "Last operation failed".to_string()
            } else {
                format!("Field {index}")
            }
        };
        for (width, height) in [(320, 200), (480, 272), (800, 480), (960, 544), (1280, 720)] {
            let layout = Layout::home(width, height)?;
            let geometry = crate::app_center::Geometry::new(&layout);
            let scale = geometry.scale;
            let top = geometry.list_top + 40_i32 * scale + 4_i32 * scale;
            for description in 0..=3 {
                for count in 0..=12 {
                    let plan = app_center::detail_plan(&geometry, top, description, count);
                    let fields: Vec<(String, String)> = (0..count)
                        .map(|index| (labels(index), "value".to_string()))
                        .collect();
                    let indices = app_center::drawn_fields(&plan, &fields);
                    assert!(indices.len() <= plan.fields);
                    assert!(indices.iter().all(|index| *index < count));
                    if plan.omitted && count > 8 && !indices.is_empty() {
                        assert!(
                            indices.contains(&7),
                            "failure missing at {width}x{height} count={count}"
                        );
                        if plan.fields <= 7 {
                            assert_eq!(indices.last(), Some(&7), "{width}x{height} count={count}");
                        }
                    }
                    let drawn = indices.len() + usize::from(plan.omitted);
                    let pitch = app_center::DETAIL_PITCH * scale;
                    let last = top
                        + i32::try_from(plan.description_lines).unwrap_or(0_i32) * pitch
                        + 2_i32 * scale
                        + i32::try_from(drawn).unwrap_or(0_i32) * pitch;
                    assert!(
                        last <= geometry.pinned.y,
                        "{width}x{height} description={description} fields={count}: \
                         {last} crosses {}",
                        geometry.pinned.y
                    );
                }
            }
        }
        Ok(())
    }

    /// The chip and its underline always contain the label cell, so a state
    /// marker can never spill over the row it belongs to.
    #[test]
    fn state_chip_contains_its_label() {
        for scale in 1_i32..=3_i32 {
            for label in ["RUNNING", "UPDATE 1.2.3", "UNAVAILABLE", "A"] {
                let width = chip_width(label, scale);
                assert!(
                    width >= i32::try_from(label.chars().count()).unwrap_or(0_i32) * advance(scale)
                );
                assert!(fit_columns(width, scale) >= label.chars().count());
                assert!(10_i32 * scale + 2_i32 >= advance(scale));
            }
        }
    }

    /// The shortcut editor preview must decode and upload once per icon, not
    /// once per rendered frame, and a renderer reset must drop the cache.
    #[test]
    fn shortcut_preview_decodes_once_per_icon_and_clears_on_reset() -> Result<(), String> {
        let _guard = crate::test_support::sdl_lock();
        assert!(
            sdl2::hint::set("SDL_VIDEODRIVER", "dummy")
                || sdl2::hint::get("SDL_VIDEODRIVER").as_deref() == Some("dummy"),
            "dummy video must be available for this fixture"
        );
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let window = video
            .window("preview cache", 320, 200)
            .hidden()
            .build()
            .map_err(|e| e.to_string())?;
        let raw_canvas = window
            .into_canvas()
            .software()
            .build()
            .map_err(|e| e.to_string())?;
        let creator = raw_canvas.texture_creator();
        let mut canvas = Screen::new(raw_canvas, &creator)?;
        let icon = include_bytes!("../assets/system/apps.png").as_slice();
        let other = include_bytes!("../assets/system/wifi.png").as_slice();
        let bounds = Rect {
            x: 0,
            y: 0,
            w: 28,
            h: 28,
        };
        performance::reset();
        assert!(canvas.icon_preview(icon, bounds)?);
        assert!(canvas.icon_preview(icon, bounds)?);
        assert_eq!(
            performance::snapshot().decodes,
            1,
            "an unchanged icon must not decode again"
        );
        assert!(canvas.icon_preview(other, bounds)?);
        assert_eq!(performance::snapshot().decodes, 2);
        assert!(
            !canvas.icon_preview(b"not an icon", bounds)?,
            "invalid bytes must keep the placeholder"
        );
        assert_eq!(performance::snapshot().decodes, 3);
        assert!(!canvas.icon_preview(b"not an icon", bounds)?);
        assert_eq!(
            performance::snapshot().decodes,
            3,
            "an unchanged failed decode must not be retried"
        );
        canvas.reset()?;
        assert!(canvas.icon_preview(icon, bounds)?);
        assert_eq!(
            performance::snapshot().decodes,
            4,
            "a reset must clear the cached preview"
        );
        Ok(())
    }

    /// Reads the canvas and asserts that everything that is not the background
    /// sits inside `bounds`.
    fn assert_ink_inside(
        canvas: &mut Screen,
        bounds: Rect,
        value: &str,
        scale: i32,
    ) -> Result<(), String> {
        let pixels = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
        let (width, _) = canvas.output_size()?;
        let pixel_width = usize::try_from(width).map_err(|error| error.to_string())?;
        let background = [
            theme::BACKGROUND.r,
            theme::BACKGROUND.g,
            theme::BACKGROUND.b,
        ];
        let (rgb_pixels, remainder) = pixels.as_chunks::<3>();
        if !remainder.is_empty() {
            return Err("Incomplete RGB24 pixel".into());
        }
        for (index, pixel) in rgb_pixels.iter().enumerate() {
            let x = i32::try_from(index.checked_rem(pixel_width).ok_or("Zero canvas width")?)
                .map_err(|error| error.to_string())?;
            let y = i32::try_from(index.checked_div(pixel_width).ok_or("Zero canvas width")?)
                .map_err(|error| error.to_string())?;
            if !bounds.contains(f64::from(x), f64::from(y)) && *pixel != background {
                return Err(format!(
                    "{value:?} at scale {scale} drew {pixel:?} outside {bounds:?} at {x},{y}"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod png_tests {
    use super::*;

    #[test]
    fn png_color_formats_expand_to_rgba() -> Result<(), Box<dyn std::error::Error>> {
        use png::{BitDepth, ColorType};

        for (color, depth, pixels, expected) in [
            (
                ColorType::Rgb,
                BitDepth::Eight,
                [12, 34, 56].as_slice(),
                [12, 34, 56, 255],
            ),
            (
                ColorType::Grayscale,
                BitDepth::Eight,
                [42].as_slice(),
                [42, 42, 42, 255],
            ),
            (
                ColorType::GrayscaleAlpha,
                BitDepth::Eight,
                [42, 128].as_slice(),
                [42, 42, 42, 128],
            ),
            (
                ColorType::Rgba,
                BitDepth::Sixteen,
                [12, 1, 34, 2, 56, 3, 128, 4].as_slice(),
                [12, 34, 56, 128],
            ),
            (
                ColorType::Indexed,
                BitDepth::One,
                [0].as_slice(),
                [12, 34, 56, 128],
            ),
        ] {
            let mut bytes = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
                encoder.set_color(color);
                encoder.set_depth(depth);
                if color == ColorType::Indexed {
                    encoder.set_palette([12, 34, 56].as_slice());
                    encoder.set_trns([128].as_slice());
                }
                encoder.write_header()?.write_image_data(pixels)?;
            }
            let surface = decode_icon(&bytes)?;
            assert_eq!(surface.without_lock(), Some(expected.as_slice()));
        }
        Ok(())
    }

    #[test]
    fn png_pixels_decode_and_broken_or_large_assets_fall_back()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()?
                .write_image_data(&[12, 34, 56, 255])?;
        }
        let surface = decode_icon(&bytes)?;
        assert_eq!((surface.width(), surface.height()), (1, 1));
        assert_eq!(surface.without_lock(), Some([12, 34, 56, 255].as_slice()));
        assert!(decode_icon(bytes.get(..24).ok_or("Missing fixture element")?).is_err());
        assert!(decode_icon(b"broken asset").is_err());
        bytes.extend(vec![0; 1024 * 1024]);
        assert!(decode_icon(&bytes).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod system_tests {
    use super::*;
    use crate::{
        input::Action,
        platform::system::{Percent, Status, Wifi},
    };

    fn footer_focus_samples(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        textures: &[Option<Texture<'_>>],
        output: &std::path::Path,
    ) -> Result<(), String> {
        use crate::settings::Page;
        let original_page = state.settings.page;
        for page in [
            Page::Display,
            Page::DateTime,
            Page::Timezones,
            Page::Wireless,
            Page::Tor,
            Page::TorDetails,
            Page::Applications,
            Page::Device,
            Page::Updates,
            Page::About,
        ] {
            state.settings.page(page);
            for (index, _) in state.settings.footer_controls().into_iter().flatten() {
                state.settings.selected = index;
                render(canvas, layout, state, textures)?;
                screenshot(
                    canvas,
                    &output.join(format!(
                        "footer-{page:?}-{index}-{}x{}.bmp",
                        layout.width, layout.height
                    )),
                )?;
            }
        }
        state.settings.page(original_page);
        Ok(())
    }

    fn update_samples(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        textures: &[Option<Texture<'_>>],
        output: &std::path::Path,
        (w, h): (u32, u32),
    ) -> Result<(), String> {
        state.settings.updater.state =
            crate::updater::State::Available(crate::updater::tests::release()?);
        render(canvas, layout, state, textures)?;
        screenshot(
            canvas,
            &output.join(format!("update-available-{w}x{h}.bmp")),
        )?;
        assert_eq!(state.settings.input(Action::SelectAndActivate(1)), None);
        render(canvas, layout, state, textures)?;
        screenshot(canvas, &output.join(format!("update-confirm-{w}x{h}.bmp")))?;
        assert_eq!(state.settings.input(Action::Back), None);
        state.settings.updater.state = crate::updater::State::Downloading {
            received: 2_500_000,
            total: 10_000_000,
        };
        render(canvas, layout, state, textures)?;
        screenshot(
            canvas,
            &output.join(format!("update-downloading-{w}x{h}.bmp")),
        )?;
        state.settings.updater.state = crate::updater::State::Failed(
                "Update check failed: Version 1.10.0 available; No shell build is available for this platform".into()
            );
        render(canvas, layout, state, textures)?;
        screenshot(canvas, &output.join(format!("update-error-{w}x{h}.bmp")))?;
        state.settings.updater.state = crate::updater::State::Installed {
            version: semver::Version::new(1, 10, 0),
            durable: true,
            relaunch: crate::platform::update::Relaunch {
                executable: "/fixture/vitrallis".into(),
                sha256: [0; 32],
            },
        };
        render(canvas, layout, state, textures)?;
        screenshot(
            canvas,
            &output.join(format!("update-installed-{w}x{h}.bmp")),
        )?;
        state.settings.updater.request_relaunch();
        assert!(
            state
                .settings
                .updater
                .relaunch_if_requested(false, || Ok(false))
        );
        render(canvas, layout, state, textures)?;
        // Relaunch holds input only while children finish. Its waiting view
        // must not expose actions or key hints that cannot be activated.
        let controls = crate::settings::PanelLayout::new(layout);
        let pixels = canvas.read_pixels(
            Some(rect(Rect {
                x: 0,
                y: controls.confirmation[0].y,
                w: i32::try_from(w).map_err(|e| e.to_string())?,
                h: i32::try_from(h)
                    .map_err(|e| e.to_string())?
                    .saturating_sub(controls.confirmation[0].y),
            })?),
            PixelFormatEnum::RGB24,
        )?;
        let background = [
            theme::BACKGROUND.r,
            theme::BACKGROUND.g,
            theme::BACKGROUND.b,
        ];
        let (rgb_pixels, remainder) = pixels.as_chunks::<3>();
        assert!(remainder.is_empty(), "RGB readback has an incomplete pixel");
        assert!(rgb_pixels.iter().all(|pixel| *pixel == background));
        let waiting = output.join("relaunch-wait");
        std::fs::create_dir_all(&waiting).map_err(|e| e.to_string())?;
        screenshot(canvas, &waiting.join(format!("updates-{w}x{h}.bmp")))?;
        // Later QA samples should not retain this committed relaunch fixture.
        let installed = std::mem::take(&mut state.settings.updater.state);
        state.settings.updater = crate::updater::Updater::default();
        state.settings.updater.state = installed;
        Ok(())
    }

    fn power_samples(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        output: &std::path::Path,
    ) -> Result<(), String> {
        use crate::{platform::system::Power, settings::PowerTransition};
        for (name, power) in [("reboot", Power::Reboot), ("shutdown", Power::Shutdown)] {
            state.settings.power_transition = Some(PowerTransition::Requested(power));
            render(canvas, layout, state, &[])?;
            screenshot(
                canvas,
                &output.join(format!("{name}-{}x{}.bmp", layout.width, layout.height)),
            )?;
        }
        state.settings.power_transition = None;
        Ok(())
    }

    fn tor_samples(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        textures: &[Option<Texture<'_>>],
        output: &std::path::Path,
        (w, h): (u32, u32),
    ) -> Result<(), String> {
        state.settings.page(crate::settings::Page::Tor);
        for (name, tor_state, percent) in [
            ("connected", crate::tor::State::Connected, Some(100)),
            ("bootstrap", crate::tor::State::Bootstrapping, Some(45)),
            ("error", crate::tor::State::Error, None),
            ("disabled", crate::tor::State::Disabled, None),
        ] {
            state.settings.tor.state = tor_state;
            state.settings.tor.progress = percent;
            state.settings.tor.apps = 1;
            state.settings.tor.mode = if name == "disabled" {
                crate::tor::Mode::Disabled
            } else {
                crate::tor::Mode::OnDemand
            };
            render(canvas, layout, state, textures)?;
            screenshot(canvas, &output.join(format!("tor-{name}-{w}x{h}.bmp")))?;
        }
        state.settings.page(crate::settings::Page::TorDetails);
        render(canvas, layout, state, textures)?;
        screenshot(canvas, &output.join(format!("tor-details-{w}x{h}.bmp")))?;
        Ok(())
    }

    #[test]
    fn system_panels_render_at_device_and_scaled_sizes() -> Result<(), String> {
        let _guard = crate::test_support::sdl_lock();
        assert!(
            sdl2::hint::set("SDL_VIDEODRIVER", "dummy")
                || sdl2::hint::get("SDL_VIDEODRIVER").as_deref() == Some("dummy"),
            "dummy video must be available for this fixture"
        );
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let qa = std::env::var_os("VITRALLIS_QA_DIR").map(std::path::PathBuf::from);
        let output = qa.as_ref().unwrap_or(&scratch.0);
        for size in [(320, 200), (480, 272), (800, 480), (1280, 720)] {
            system_panel_samples(&sdl, &video, output, size)?;
        }
        verify_references(output)
    }

    /// One QA sweep over the system panels at a single window size.
    fn system_panel_samples(
        sdl: &sdl2::Sdl,
        video: &sdl2::VideoSubsystem,
        output: &std::path::Path,
        (w, h): (u32, u32),
    ) -> Result<(), String> {
        let window = video
            .window("system QA", w, h)
            .hidden()
            .build()
            .map_err(|e| e.to_string())?;
        let raw_canvas = window
            .into_canvas()
            .software()
            .build()
            .map_err(|e| e.to_string())?;
        let layout = Layout::home(
            u16::try_from(w).map_err(|e| e.to_string())?,
            u16::try_from(h).map_err(|e| e.to_string())?,
        )?;
        let mut state = Launcher::new(Vec::new(), 3, 6)?;
        state.settings.status = Status {
            battery: Some(Percent::new(73)?),
            charging: Some(true),
            external_power: Some(true),
            wifi: Some(Wifi::Connected),
            brightness: Some(Percent::new(44)?),
            volume: Some(Percent::new(90)?),
            clock: Some("12:34".into()),
            ip: Some(std::net::Ipv4Addr::new(10, 0, 0, 137)),
            screen_timeout: Some(600),
            timezone: Some("America/Vancouver".into()),
            timezones: vec!["America/Vancouver".into(), "UTC".into()],
            calibration: true,
            power_controls: true,
            ..Status::default()
        };
        let creator = raw_canvas.texture_creator();
        let mut canvas = Screen::new(raw_canvas, &creator)?;
        home_samples(&mut canvas, &layout, output)?;
        if w == 480 {
            cache_tests::lifecycle(&creator, &mut canvas)?;
            crate::boot::lifecycle(sdl, &mut canvas, &layout)?;
        }
        crate::boot::qa(&mut canvas, &layout, output)?;
        power_samples(&mut canvas, &layout, &mut state, output)?;
        state.settings.system_state = crate::settings::SystemState::Ready;
        state.settings.network_available = true;
        assert_eq!(state.settings.input(Action::System), None);
        let scene_creator = canvas.texture_creator();
        let textures = artwork(&scene_creator, &state);
        capture(&mut canvas, &layout, &state, &textures, output, "system")?;
        state.settings.page(crate::settings::Page::Device);
        capture(&mut canvas, &layout, &state, &textures, output, "device")?;
        preferences_sample(&mut canvas, &layout, &mut state, &textures, output)?;
        state.settings.page(crate::settings::Page::DateTime);
        capture(&mut canvas, &layout, &state, &textures, output, "datetime")?;
        state.settings.page(crate::settings::Page::Wireless);
        state.settings.status.wifi_enabled = Some(true);
        state.settings.status.bluetooth = Some(false);
        capture(&mut canvas, &layout, &state, &textures, output, "wireless")?;
        tor_samples(&mut canvas, &layout, &mut state, &textures, output, (w, h))?;
        state.settings.page(crate::settings::Page::Updates);
        capture(&mut canvas, &layout, &state, &textures, output, "updates")?;
        update_samples(&mut canvas, &layout, &mut state, &textures, output, (w, h))?;
        state.settings.page(crate::settings::Page::About);
        capture(&mut canvas, &layout, &state, &textures, output, "about")?;
        control_focus_samples(&mut canvas, &layout, &mut state, &textures, output)?;
        state.settings.page(crate::settings::Page::Timezones);
        capture(&mut canvas, &layout, &state, &textures, output, "zones")?;
        footer_focus_samples(&mut canvas, &layout, &mut state, &textures, output)?;
        system_overlay_samples(&mut canvas, &layout, &mut state, output)
    }

    /// Each actionable row must show focus at the same bounds used for input.
    fn control_focus_samples(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        textures: &[Option<Texture<'_>>],
        output: &std::path::Path,
    ) -> Result<(), String> {
        use crate::settings::{Page, WIRELESS_ROWS};
        for (page, count) in [
            (Page::Display, 2),
            (Page::Wireless, WIRELESS_ROWS),
            (Page::Tor, 6),
        ] {
            state.settings.page(page);
            for index in 0..count {
                state.settings.selected = index;
                capture(
                    canvas,
                    layout,
                    state,
                    textures,
                    output,
                    &format!("focus-{page:?}-{index}"),
                )?;
            }
        }
        Ok(())
    }

    /// The guarded confirmations, unavailable state and launch feedback that
    /// complete one size sweep.
    fn system_overlay_samples(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        output: &std::path::Path,
    ) -> Result<(), String> {
        // Power actions are directly reachable from the overview.
        state.settings.page(crate::settings::Page::Home);
        state.settings.selected = 14;
        assert_eq!(state.settings.input(Action::Activate), None);
        capture(canvas, layout, state, &[], output, "confirm")?;
        assert_eq!(state.settings.selected, 0);
        state.settings.cancel();
        state.settings.status = Status::default();
        state.settings.system_state = crate::settings::SystemState::Unavailable;
        assert_eq!(state.settings.input(Action::System), None);
        capture(canvas, layout, state, &[], output, "unavailable")?;
        state.settings.cancel();
        // Launch feedback is a status line, not a modal loading screen.
        state.opening = Some("Bitcoin CAD".into());
        state.phase = crate::launcher::Phase::Launching;
        state.status = "Bitcoin CAD is launching...".into();
        state.status_notice = true;
        capture(canvas, layout, state, &[], output, "launching")?;
        state.opening = None;
        state.phase = crate::launcher::Phase::Ready;
        app_center::qa(canvas, layout, output)?;
        shortcuts::qa(canvas, layout, output)?;
        state.settings.show();
        system::storage_qa(canvas, layout, state, &[], output)
    }

    /// Renders and captures one named system panel at the current size.
    fn capture(
        canvas: &mut Screen,
        layout: &Layout,
        state: &Launcher,
        textures: &[Option<Texture<'_>>],
        output: &std::path::Path,
        name: &str,
    ) -> Result<(), String> {
        render(canvas, layout, state, textures)?;
        screenshot(
            canvas,
            &output.join(format!("{name}-{}x{}.bmp", layout.width, layout.height)),
        )
    }

    fn preferences_sample(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        textures: &[Option<Texture<'_>>],
        output: &std::path::Path,
    ) -> Result<(), String> {
        state.settings.page(crate::settings::Page::Applications);
        state.settings.policy_apps = vec![("io.vitrallis.notepad".into(), "Notepad".into())];
        state.settings.policy.background_seconds = 300;
        state.settings.policy.ampm = true;
        let original_clock = state.preferences.ampm;
        state.preferences.ampm = state.settings.policy.ampm;
        render(canvas, layout, state, textures)?;
        screenshot(
            canvas,
            &output.join(format!(
                "preferences-{}x{}.bmp",
                layout.width, layout.height
            )),
        )?;
        state.preferences.ampm = original_clock;
        Ok(())
    }
    fn verify_idle_footer(
        canvas: &mut Screen,
        layout: &Layout,
        state: &mut Launcher,
        textures: &[Option<Texture<'_>>],
    ) -> Result<(), String> {
        // Startup warnings and exit notices must not leak into the idle footer.
        let idle = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
        state.status = "Folder state unavailable: invalid configuration".into();
        state.status_notice = true;
        render(canvas, layout, state, textures)?;
        assert_eq!(canvas.read_pixels(None, PixelFormatEnum::RGB24)?, idle);
        Ok(())
    }

    fn home_samples(
        canvas: &mut Screen,
        layout: &Layout,
        output: &std::path::Path,
    ) -> Result<(), String> {
        use crate::app::{AppEntry, AppManifest, AppSource};
        let entry = |(id, name, source): (&str, &str, AppSource)| AppEntry {
            id: id.into(),
            name: name.into(),
            source,
            icon: None,
            unavailable: None,
            manifest: AppManifest {
                entry: "/fixture/app".into(),
                ..AppManifest::default()
            },
        };
        let mut state = Launcher::new(
            [
                ("io.vitrallis.terminal", "Terminal", AppSource::Native),
                ("io.vitrallis.notepad", "Notepad", AppSource::Native),
                ("io.vitrallis.files", "Files", AppSource::Native),
                (crate::app_center::TILE_ID, "App Center", AppSource::Native),
                ("vitrallis-wifi-settings", "Settings", AppSource::Native),
                ("folder", "Tools", AppSource::Folder),
            ]
            .into_iter()
            .map(entry)
            .collect(),
            layout.columns,
            layout.tiles.len(),
        )?;
        let _previous_fixture_state = state.app_states.insert(
            "io.vitrallis.notepad".into(),
            crate::process::AppState::RunningBackground,
        );
        let creator = canvas.texture_creator();
        let textures = artwork(&creator, &state);
        for (name, toolbar) in [
            ("tiles", None),
            ("actions", Some(crate::shortcuts::screen::Toolbar::Actions)),
        ] {
            state.desktop.toolbar = toolbar;
            render(canvas, layout, &state, &textures)?;
            screenshot(
                canvas,
                &output.join(format!(
                    "home-{name}-{}x{}.bmp",
                    layout.width, layout.height
                )),
            )?;
        }
        verify_idle_footer(canvas, layout, &mut state, &textures)?;
        // Widest plausible catalogue content exercises the tile label policy;
        // the folder view and the error dialog cover the remaining home
        // surfaces that add their own footer and status bands.
        let mut long = Launcher::new(
            [
                ("io.vitrallis.terminal", "A", AppSource::Native),
                (
                    "io.vitrallis.notepad",
                    "Experimental Rust Application",
                    AppSource::Native,
                ),
                (
                    "io.vitrallis.files",
                    "Extremely Long Application Name",
                    AppSource::Native,
                ),
                (crate::app_center::TILE_ID, "App Center", AppSource::Native),
                ("vitrallis-wifi-settings", "Settings", AppSource::Native),
                ("folder", "Tools", AppSource::Folder),
            ]
            .into_iter()
            .map(entry)
            .collect(),
            layout.columns,
            layout.tiles.len(),
        )?;
        drop(long.folders.names.insert("folder".into(), "Tools".into()));
        for app in long.all_apps.clone() {
            if app.id != "folder" {
                drop(long.folders.members.insert(app.id, "folder".into()));
            }
        }
        long.folder = Some("folder".into());
        long.rebuild_view(None);
        long.status = "Experimental Rust Application is launching...".into();
        long.status_notice = true;
        let long_textures = artwork(&creator, &long);
        render(canvas, layout, &long, &long_textures)?;
        screenshot(
            canvas,
            &output.join(format!(
                "home-folder-{}x{}.bmp",
                layout.width, layout.height
            )),
        )?;
        long.error = Some(
            "Entry point is not executable and no compatible runtime was found for this device."
                .into(),
        );
        render(canvas, layout, &long, &long_textures)?;
        screenshot(
            canvas,
            &output.join(format!("home-error-{}x{}.bmp", layout.width, layout.height)),
        )
    }

    fn verify_references(output: &std::path::Path) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        use std::fmt::Write;
        let references: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, String>,
        > = serde_json::from_str(include_str!(
            "../tests/fixtures/renderer/phase1-sha256.json"
        ))
        .map_err(|e| e.to_string())?;
        // SDL's software image blending uses architecture-specific SIMD rounding.
        // Keep exact reviewed pixels for each host instead of tolerating differences.
        let profile = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let reference_entries = references
            .get(&profile)
            .ok_or_else(|| format!("No renderer references for {profile}"))?;
        let rendered: std::collections::BTreeSet<_> = std::fs::read_dir(output)
            .map_err(|e| e.to_string())?
            .map(|result| result.map(|entry| entry.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|path| path.extension().is_some_and(|extension| extension == "bmp"))
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect();
        let expected_names = reference_entries.keys().cloned().collect();
        if rendered != expected_names {
            return Err(format!(
                "Renderer reference inventory differs: unreviewed {:?}; missing {:?}",
                rendered.difference(&expected_names).collect::<Vec<_>>(),
                expected_names.difference(&rendered).collect::<Vec<_>>()
            ));
        }
        for (name, expected) in reference_entries {
            let bytes = std::fs::read(output.join(name)).map_err(|e| e.to_string())?;
            let digest = Sha256::digest(bytes);
            let mut actual = String::with_capacity(
                digest
                    .len()
                    .checked_mul(2)
                    .ok_or("Digest capacity overflow")?,
            );
            for byte in digest {
                write!(&mut actual, "{byte:02x}").map_err(|e| e.to_string())?;
            }
            assert_eq!(actual, *expected, "Reference pixels changed: {name}");
        }
        Ok(())
    }
}
