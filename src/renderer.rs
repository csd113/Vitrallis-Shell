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
use std::io::Write;

pub struct Screen<'a> {
    canvas: Canvas<Window>,
    presentation: backend::PresentationClock,
    font: vitrallis_native::font::Atlas<'a>,
    creator: &'a TextureCreator<WindowContext>,
    center_icons: Vec<(Box<[u8]>, Texture<'a>)>,
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
        })
    }
    pub fn present(&mut self) {
        self.presentation.present(&mut self.canvas);
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.font = vitrallis_native::font::Atlas::new(self.creator)?;
        self.center_icons.clear();
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
                self.center_icons.remove(0);
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
            performance::count(|c| c.uploads += 1);
            self.center_icons.push((pixels.into(), texture));
            self.center_icons.len() - 1
        };
        self.canvas
            .copy(&self.center_icons[index].1, None, rect(bounds)?)
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
        u32::try_from(r.w).map_err(|_| "negative rectangle width")?,
        u32::try_from(r.h).map_err(|_| "negative rectangle height")?,
    ))
}
fn fill(canvas: &mut Screen, r: Rect, color: Color) -> Result<(), String> {
    if r.w <= 0 || r.h <= 0 {
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
pub fn text(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    if scale <= 0 {
        return Err("invalid text scale".into());
    }
    let limit =
        usize::try_from(bounds.w / (theme::CELL * scale)).map_err(|_| "invalid text width")?;
    let count = i32::try_from(value.chars().take(limit).count()).map_err(|_| "text too long")?;
    let mut x = bounds.x + (bounds.w - count * theme::CELL * scale) / 2;
    let y = bounds.y + (bounds.h - theme::CELL * scale) / 2;
    for character in value.chars().take(limit) {
        #[cfg(test)]
        performance::count(|c| c.glyphs += 1);
        #[cfg(test)]
        performance::count(|c| {
            c.text_operations += u64::from(!character.is_ascii_control() && character != ' ');
        });
        let character = if character.is_ascii() { character } else { '?' };
        canvas.font.draw(
            &mut canvas.canvas,
            character,
            rect(Rect {
                x,
                y,
                w: theme::CELL * scale,
                h: theme::CELL * scale,
            })?,
            color,
        )?;
        x += theme::CELL * scale;
    }
    Ok(())
}

mod artwork;
pub use artwork::artwork;

pub fn decode_icon(bytes: &[u8]) -> Result<Surface<'static>, String> {
    #[cfg(test)]
    performance::count(|c| c.decodes += 1);
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
        let mut rgba = Vec::with_capacity(pixels.len() / channels * 4);
        for pixel in pixels[..frame.buffer_size()].chunks_exact(channels) {
            match frame.color_type {
                png::ColorType::Rgb => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
                png::ColorType::Rgba => rgba.extend_from_slice(pixel),
                png::ColorType::Grayscale => {
                    rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]);
                }
                png::ColorType::GrayscaleAlpha => {
                    rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
                }
                png::ColorType::Indexed => return Err("unexpanded PNG palette".into()),
            }
        }
        return Surface::from_data(
            &mut rgba,
            frame.width,
            frame.height,
            frame.width * 4,
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
        counts.frames += 1;
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
        crate::settings::Page::Wireless => "Wireless Network Controls",
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
    let system_icons = icons.get(state.apps.len() + 1..).unwrap_or(&[]);
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
            state.page_start() / layout.tiles.len() + 1 < state.page_count(),
        ),
    ] {
        text(
            canvas,
            label,
            // Inset the arrow glyphs while keeping the full header touch targets.
            Rect {
                y: bounds.y + 4 * layout.text_scale,
                h: bounds.h / 2,
                ..bounds
            },
            (layout.text_scale + 1).min(bounds.h / 16),
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
        let index = state.page_start() + local;
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

/// Lower-left status area: the launching/exit notifications the Shell already
/// produced, shown without a modal so the menu stays visible and interactive.
fn launcher_status(canvas: &mut Screen, layout: &Layout, state: &Launcher) -> Result<(), String> {
    if !state.status_notice || state.status.is_empty() || state.error.is_some() {
        return Ok(());
    }
    let right = if state.folder.is_some() {
        layout.folder_back.x
    } else {
        layout.desktop_menu.x
    };
    let bounds = Rect {
        x: layout.footer.x,
        y: layout.footer.y + (layout.footer.h - 8 * layout.text_scale) / 2,
        w: right - layout.footer.x - 8,
        h: 8 * layout.text_scale,
    };
    if bounds.w <= 0 {
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

/// Left-aligned text shortened with an ellipsis rather than clipped mid-glyph.
fn clipped(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    let limit = usize::try_from(bounds.w / (8 * scale)).unwrap_or(0);
    if limit == 0 {
        return Ok(());
    }
    let shown: String = if value.chars().count() > limit {
        if limit > 3 {
            format!("{}...", value.chars().take(limit - 3).collect::<String>())
        } else {
            value.chars().take(limit).collect()
        }
    } else {
        value.to_owned()
    };
    let width = i32::try_from(shown.chars().count()).unwrap_or(0) * 8 * scale;
    text(canvas, &shown, Rect { w: width, ..bounds }, scale, color)
}

fn desktop_footer(canvas: &mut Screen, layout: &Layout, state: &Launcher) -> Result<(), String> {
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
        1,
        theme::ACCENT,
    )?;
    if state.folder.is_some() {
        card(
            canvas,
            layout.folder_back,
            focus == Some(layout.folder_back),
        )?;
        text(canvas, "Back to Apps", layout.folder_back, 1, theme::ACCENT)?;
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
    texture: Option<&Texture<'_>>,
) -> Result<(), String> {
    card(canvas, tile, selected)?;
    let icon = Rect {
        x: tile.x + (tile.w - layout.icon_size) / 2,
        y: tile.y + tile.h / 12,
        w: layout.icon_size,
        h: layout.icon_size,
    };
    if let Some(texture) = texture {
        let size = texture.query();
        let width = i32::try_from(size.width).map_err(|_| "icon width")?;
        let height = i32::try_from(size.height).map_err(|_| "icon height")?;
        let w = icon.w.min(icon.h * width / height);
        let h = icon.h.min(icon.w * height / width);
        canvas.copy(
            texture,
            None,
            rect(Rect {
                x: icon.x + (icon.w - w) / 2,
                y: icon.y + (icon.h - h) / 2,
                w,
                h,
            })?,
        )?;
    } else {
        fill(canvas, icon, theme::BORDER)?;
        let mark = if app.unavailable.is_some() { "!" } else { "+" };
        text(canvas, mark, icon, layout.text_scale + 1, theme::TEXT)?;
    }
    if app.unavailable.is_some() && !app.is_system_settings() {
        text(
            canvas,
            "!",
            Rect {
                x: tile.x + tile.w - 20,
                y: tile.y,
                w: 20,
                h: 20,
            },
            layout.text_scale,
            theme::WARNING,
        )?;
    }
    let label_top = icon.y + icon.h;
    text(
        canvas,
        &app.name,
        Rect {
            x: tile.x + theme::SPACE,
            y: label_top,
            w: tile.w - 2 * theme::SPACE,
            h: tile.y + tile.h - label_top,
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
        x: tile.x + tile.w - width - 3 * scale,
        y: tile.y + 3 * scale,
        w: width,
        h: 10 * scale + 2,
    }
}

/// Width of a compact state chip: one cell per character plus padding.
pub fn chip_width(label: &str, scale: i32) -> i32 {
    i32::try_from(label.chars().count()).unwrap_or(0) * 8 * scale + 6 * scale
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
            y: bounds.y + bounds.h - 1,
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
        h: layout.footer.y - layout.title.h,
    };
    fill(canvas, bounds, theme::ERROR_SURFACE)?;
    let line_height = 16 * layout.text_scale;
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
    let columns =
        usize::try_from((bounds.w - 16) / (8 * layout.text_scale)).map_err(|_| "dialog columns")?;
    let rows = usize::try_from(bounds.h / line_height - 1).map_err(|_| "dialog rows")?;
    let chars: Vec<_> = error.chars().collect();
    for (row, chunk) in chars.chunks(columns).take(rows).enumerate() {
        let value: String = chunk.iter().collect();
        text(
            canvas,
            &value,
            Rect {
                x: bounds.x + 8,
                y: bounds.y + (i32::try_from(row).map_err(|_| "dialog row")? + 1) * line_height,
                w: bounds.w - 16,
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
    let (width, height) = canvas.output_size()?;
    let mut pixels = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
    let surface = Surface::from_data(
        &mut pixels,
        width,
        height,
        width * 3,
        PixelFormatEnum::RGB24,
    )?;
    let mut encoded =
        vec![0; usize::try_from(width * height * 4 + 4096).map_err(|_| "screenshot too large")?];
    let length;
    {
        let mut rw = sdl2::rwops::RWops::from_bytes_mut(&mut encoded)?;
        surface.save_bmp_rw(&mut rw)?;
        length = std::io::Seek::stream_position(&mut rw).map_err(|e| e.to_string())?;
    }
    let length = usize::try_from(length).map_err(|_| "screenshot too large")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("screenshot {}: {e}", path.display()))?;
    file.write_all(&encoded[..length])
        .map_err(|e| e.to_string())
}

// Inspect the same bounded bytes passed to SDL, before its native decoder allocates.
fn validate_bmp(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 54 || bytes.len() > 1024 * 1024 || &bytes[..2] != b"BM" {
        return Err("icon must be a BMP between 54 bytes and 1 MiB".into());
    }
    let word = |offset: usize| {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    };
    if word(14) != 40
        || !(1..=512).contains(&word(18))
        || !(1..=512).contains(&word(22))
        || bytes[26..28] != [1, 0]
        || !matches!(u16::from_le_bytes([bytes[28], bytes[29]]), 24 | 32)
        || word(30) != 0
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
        assert!(decode_icon(&bytes[..24]).is_err());
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
        state.settings.input(Action::SelectAndActivate(1));
        render(canvas, layout, state, textures)?;
        screenshot(canvas, &output.join(format!("update-confirm-{w}x{h}.bmp")))?;
        state.settings.input(Action::Back);
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
    #[allow(clippy::too_many_lines, reason = "one QA sweep over every size")]
    fn system_panels_render_at_device_and_scaled_sizes() -> Result<(), String> {
        sdl2::hint::set("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let qa = std::env::var_os("VITRALLIS_QA_DIR").map(std::path::PathBuf::from);
        let output = qa.as_ref().unwrap_or(&scratch.0);
        for (w, h) in [(320, 200), (480, 272), (800, 480), (1280, 720)] {
            let window = video
                .window("system QA", w, h)
                .hidden()
                .build()
                .map_err(|e| e.to_string())?;
            let canvas = window
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
            let creator = canvas.texture_creator();
            let mut canvas = Screen::new(canvas, &creator)?;
            home_samples(&mut canvas, &layout, output)?;
            if w == 480 {
                cache_tests::lifecycle(&creator, &mut canvas)?;
                crate::boot::lifecycle(&sdl, &mut canvas, &layout)?;
            }
            crate::boot::qa(&mut canvas, &layout, output)?;
            power_samples(&mut canvas, &layout, &mut state, output)?;
            state.settings.system_state = crate::settings::SystemState::Ready;
            state.settings.network_available = true;
            state.settings.input(Action::System);
            let creator = canvas.texture_creator();
            let textures = artwork(&creator, &state);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("system-{w}x{h}.bmp")))?;
            state.settings.page(crate::settings::Page::Device);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("device-{w}x{h}.bmp")))?;
            preferences_sample(&mut canvas, &layout, &mut state, &textures, output)?;
            state.settings.page(crate::settings::Page::DateTime);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("datetime-{w}x{h}.bmp")))?;
            state.settings.page(crate::settings::Page::Wireless);
            state.settings.status.wifi_enabled = Some(true);
            state.settings.status.bluetooth = Some(false);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("wireless-{w}x{h}.bmp")))?;
            tor_samples(&mut canvas, &layout, &mut state, &textures, output, (w, h))?;
            state.settings.page(crate::settings::Page::Updates);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("updates-{w}x{h}.bmp")))?;
            update_samples(&mut canvas, &layout, &mut state, &textures, output, (w, h))?;
            state.settings.page(crate::settings::Page::About);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("about-{w}x{h}.bmp")))?;
            state.settings.page(crate::settings::Page::Timezones);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("zones-{w}x{h}.bmp")))?;
            footer_focus_samples(&mut canvas, &layout, &mut state, &textures, output)?;
            // The guarded power confirmation is reachable from Device.
            state.settings.page(crate::settings::Page::Device);
            state.settings.selected = 3;
            state.settings.input(Action::Activate);
            render(&mut canvas, &layout, &state, &[])?;
            screenshot(&canvas, &output.join(format!("confirm-{w}x{h}.bmp")))?;
            assert_eq!(state.settings.selected, 0);
            state.settings.cancel();
            state.settings.status = Status::default();
            state.settings.system_state = crate::settings::SystemState::Unavailable;
            state.settings.input(Action::System);
            render(&mut canvas, &layout, &state, &[])?;
            screenshot(&canvas, &output.join(format!("unavailable-{w}x{h}.bmp")))?;
            state.settings.cancel();
            // Launch feedback is a status line, not a modal loading screen.
            state.opening = Some("Bitcoin CAD".into());
            state.phase = crate::launcher::Phase::Launching;
            state.status = "Bitcoin CAD is launching...".into();
            state.status_notice = true;
            render(&mut canvas, &layout, &state, &[])?;
            screenshot(&canvas, &output.join(format!("launching-{w}x{h}.bmp")))?;
            state.opening = None;
            state.phase = crate::launcher::Phase::Ready;
            app_center::qa(&mut canvas, &layout, output)?;
            shortcuts::qa(&mut canvas, &layout, output)?;
            state.settings.show();
            system::storage_qa(&mut canvas, &layout, &mut state, &[], output)?;
        }
        verify_references(output)
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
    fn home_samples(
        canvas: &mut Screen,
        layout: &Layout,
        output: &std::path::Path,
    ) -> Result<(), String> {
        use crate::app::{AppEntry, AppManifest, AppSource};
        let apps = [
            ("io.vitrallis.terminal", "Terminal", AppSource::Native),
            ("io.vitrallis.notepad", "Notepad", AppSource::Native),
            ("io.vitrallis.files", "Files", AppSource::Native),
            (crate::app_center::TILE_ID, "App Center", AppSource::Native),
            ("vitrallis-wifi-settings", "Settings", AppSource::Native),
            ("folder", "Tools", AppSource::Folder),
        ]
        .into_iter()
        .map(|(id, name, source)| AppEntry {
            id: id.into(),
            name: name.into(),
            source,
            icon: None,
            unavailable: None,
            manifest: AppManifest {
                entry: "/fixture/app".into(),
                ..AppManifest::default()
            },
        })
        .collect();
        let mut state = Launcher::new(apps, layout.columns, layout.tiles.len())?;
        state.app_states.insert(
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
        Ok(())
    }

    fn verify_references(output: &std::path::Path) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        let references: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, String>,
        > = serde_json::from_str(include_str!(
            "../tests/fixtures/renderer/phase1-sha256.json"
        ))
        .map_err(|e| e.to_string())?;
        for (name, expected) in references.get(std::env::consts::OS).into_iter().flatten() {
            let bytes = std::fs::read(output.join(name)).map_err(|e| e.to_string())?;
            assert_eq!(
                Sha256::digest(bytes)
                    .iter()
                    .fold(String::new(), |mut out, byte| {
                        use std::fmt::Write;
                        write!(&mut out, "{byte:02x}").unwrap();
                        out
                    }),
                *expected,
                "Reference pixels changed: {name}"
            );
        }
        Ok(())
    }
}
