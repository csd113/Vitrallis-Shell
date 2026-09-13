mod app_center;
mod shortcuts;
mod system;
use crate::{
    app::AppEntry,
    launcher::Launcher,
    layout::{Layout, Rect},
};
use font8x8::UnicodeFonts;
use sdl2::{
    pixels::{Color, PixelFormatEnum},
    render::{Canvas, Texture, TextureCreator},
    surface::Surface,
    video::{Window, WindowContext},
};
use std::io::{Read, Write};

pub type Screen = Canvas<Window>;
fn rect(r: Rect) -> Result<sdl2::rect::Rect, String> {
    Ok(sdl2::rect::Rect::new(
        r.x,
        r.y,
        u32::try_from(r.w).map_err(|_| "negative rectangle width")?,
        u32::try_from(r.h).map_err(|_| "negative rectangle height")?,
    ))
}
fn fill(canvas: &mut Screen, r: Rect, color: Color) -> Result<(), String> {
    canvas.set_draw_color(color);
    canvas.fill_rect(rect(r)?)
}
fn text(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    canvas.set_draw_color(color);
    let limit = usize::try_from(bounds.w / (8 * scale)).map_err(|_| "invalid text width")?;
    let count = i32::try_from(value.chars().take(limit).count()).map_err(|_| "text too long")?;
    let mut x = bounds.x + (bounds.w - count * 8 * scale) / 2;
    let y = bounds.y + (bounds.h - 8 * scale) / 2;
    for character in value.chars().take(limit) {
        let glyph = font8x8::BASIC_FONTS
            .get(character)
            .or_else(|| font8x8::BASIC_FONTS.get('?'))
            .unwrap_or([0; 8]);
        for (row, bits) in (0_i32..8).zip(glyph) {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    canvas.fill_rect(rect(Rect {
                        x: x + col * scale,
                        y: y + row * scale,
                        w: scale,
                        h: scale,
                    })?)?;
                }
            }
        }
        x += 8 * scale;
    }
    Ok(())
}

pub fn icons<'a>(
    creator: &'a TextureCreator<WindowContext>,
    apps: &[AppEntry],
) -> Vec<Option<Texture<'a>>> {
    // Bound the retained artwork on a 512 MiB device even when a small JSON
    // catalogue refers to thousands of maximum-size images.
    let mut remaining = 16 * 1024 * 1024;
    apps.iter()
        .map(|app| {
            if remaining == 0 {
                return None;
            }
            let custom = crate::shortcuts::icon(app);
            let builtin: Option<&[u8]> = custom
                .as_deref()
                .or_else(|| crate::native::icon(app))
                .or_else(|| {
                    if app.id == crate::app_center::TILE_ID {
                        Some(include_bytes!("../assets/system/apps.png").as_slice())
                    } else if app.is_system_settings() {
                        Some(include_bytes!("../assets/system/gear.png").as_slice())
                    } else {
                        None
                    }
                });
            let result = if let Some(bytes) = builtin {
                decode_icon(bytes).and_then(|surface| {
                    creator
                        .create_texture_from_surface(&surface)
                        .map_err(|e| e.to_string())
                })
            } else {
                load_image(creator, app.icon.as_ref()?)
            };
            match result {
                Ok(mut texture) => {
                    texture.set_scale_mode(sdl2::render::ScaleMode::Linear);
                    let size = texture.query();
                    let bytes = size.width * size.height * 4;
                    if bytes > remaining {
                        remaining = 0;
                        eprintln!("level=warn event=artwork_budget_exhausted");
                        None
                    } else {
                        remaining -= bytes;
                        Some(texture)
                    }
                }
                Err(error) => {
                    eprintln!(
                        "level=warn event=icon_fallback app={} message={error:?}",
                        app.id
                    );
                    None
                }
            }
        })
        .collect()
}

fn load_image<'a>(
    creator: &'a TextureCreator<WindowContext>,
    path: &std::path::Path,
) -> Result<Texture<'a>, String> {
    if !std::fs::metadata(path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("expected a regular file".into());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("icon must be a regular image file".into());
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let surface = decode_icon(&bytes)?;
    creator
        .create_texture_from_surface(&surface)
        .map_err(|e| e.to_string())
}

pub fn artwork<'a>(
    creator: &'a TextureCreator<WindowContext>,
    state: &Launcher,
) -> Vec<Option<Texture<'a>>> {
    let mut textures = icons(creator, &state.apps);
    textures.push(state.preferences.wallpaper.as_ref().and_then(|path| {
        load_image(creator, path)
            .map_err(|error| {
                eprintln!("level=warn event=wallpaper_fallback message={error:?}");
            })
            .ok()
    }));
    for bytes in system::ASSETS {
        textures.push(
            decode_icon(bytes)
                .and_then(|surface| {
                    creator
                        .create_texture_from_surface(&surface)
                        .map_err(|e| e.to_string())
                })
                .map(|mut texture| {
                    texture.set_scale_mode(sdl2::render::ScaleMode::Linear);
                    texture
                })
                .ok(),
        );
    }
    textures
}

pub fn decode_icon(bytes: &[u8]) -> Result<Surface<'static>, String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let mut decoder = png::Decoder::new_with_limits(
            bytes,
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
        let mut pixels = vec![0; reader.output_buffer_size()];
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

fn render_launcher(
    canvas: &mut Screen,
    layout: &Layout,
    state: &Launcher,
    icons: &[Option<Texture<'_>>],
) -> Result<(), String> {
    let [red, green, blue] = state.preferences.color;
    canvas.set_draw_color(Color::RGB(red, green, blue));
    canvas.clear();
    if let Some(Some(wallpaper)) = icons.get(state.apps.len()) {
        canvas.copy(wallpaper, None, None)?;
    }
    text(
        canvas,
        &if state.settings.open {
            format!("SYSTEM SETTINGS {}", env!("CARGO_PKG_VERSION"))
        } else {
            "VITRALLIS".into()
        },
        Rect {
            h: layout.title.h / 2,
            ..layout.title
        },
        layout.text_scale,
        Color::RGB(93, 218, 201),
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
                Color::RGB(93, 218, 201)
            } else {
                Color::RGB(64, 78, 90)
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
            state.running.contains(&app.id),
            icons.get(index).and_then(Option::as_ref),
        )?;
    }
    loading_dialog(canvas, layout, state)?;
    error_dialog(canvas, layout, state)?;
    desktop_footer(canvas, layout, state)?;
    Ok(())
}

fn desktop_footer(canvas: &mut Screen, layout: &Layout, state: &Launcher) -> Result<(), String> {
    if let Some(toolbar) = state.desktop.toolbar {
        let bounds = toolbar.bounds(layout);
        fill(canvas, bounds, Color::RGB(42, 77, 92))?;
        canvas.set_draw_color(Color::RGB(120, 240, 220));
        canvas.draw_rect(rect(bounds)?)?;
    }
    text(
        canvas,
        "Settings [Power]",
        Rect {
            w: layout.footer.w / 3,
            ..layout.footer
        },
        layout.text_scale,
        Color::RGB(173, 194, 210),
    )?;
    for (bounds, label) in [
        (layout.add_shortcut, "Add shortcut [F2]"),
        (layout.desktop_menu, "Actions [F10]"),
    ] {
        text(canvas, label, bounds, 1, Color::RGB(93, 218, 201))?;
    }
    Ok(())
}

fn loading_dialog(canvas: &mut Screen, layout: &Layout, state: &Launcher) -> Result<(), String> {
    let Some(name) = &state.opening else {
        return Ok(());
    };
    let bounds = Rect {
        x: layout.title.x + 12,
        y: i32::from(layout.height) / 2 - 40 * layout.text_scale,
        w: layout.title.w - 24,
        h: 80 * layout.text_scale,
    };
    fill(canvas, bounds, Color::RGB(23, 39, 53))?;
    canvas.set_draw_color(Color::RGB(93, 218, 201));
    canvas.draw_rect(rect(bounds)?)?;
    text(
        canvas,
        &format!("Opening {name}..."),
        Rect {
            h: bounds.h / 2,
            ..bounds
        },
        layout.text_scale,
        Color::RGB(232, 241, 247),
    )?;
    text(
        canvas,
        "Please wait",
        Rect {
            y: bounds.y + bounds.h / 2,
            h: bounds.h / 2,
            ..bounds
        },
        layout.text_scale,
        Color::RGB(93, 218, 201),
    )
}

fn render_tile(
    canvas: &mut Screen,
    layout: &Layout,
    app: &AppEntry,
    tile: Rect,
    selected: bool,
    running: bool,
    texture: Option<&Texture<'_>>,
) -> Result<(), String> {
    fill(
        canvas,
        tile,
        if selected {
            Color::RGB(44, 82, 99)
        } else {
            Color::RGB(25, 40, 55)
        },
    )?;
    if running {
        text(
            canvas,
            "*",
            Rect {
                x: tile.x,
                y: tile.y,
                w: 16,
                h: 16,
            },
            1,
            Color::RGB(93, 218, 201),
        )?;
    }
    if selected {
        canvas.set_draw_color(Color::RGB(93, 218, 201));
        canvas.draw_rect(rect(tile)?)?;
    }
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
        fill(canvas, icon, Color::RGB(57, 115, 137))?;
        let mark = if app.unavailable.is_some() { "!" } else { "+" };
        text(
            canvas,
            mark,
            icon,
            layout.text_scale + 1,
            Color::RGB(219, 243, 240),
        )?;
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
            Color::RGB(255, 185, 96),
        )?;
    }
    let label_top = icon.y + icon.h;
    text(
        canvas,
        &app.name,
        Rect {
            x: tile.x + 4,
            y: label_top,
            w: tile.w - 8,
            h: tile.y + tile.h - label_top,
        },
        layout.text_scale,
        Color::RGB(235, 242, 249),
    )?;
    Ok(())
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
    fill(canvas, bounds, Color::RGB(48, 32, 35))?;
    let line_height = 16 * layout.text_scale;
    text(
        canvas,
        "COULD NOT OPEN APP",
        Rect {
            h: line_height,
            ..bounds
        },
        layout.text_scale,
        Color::RGB(255, 185, 96),
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
            Color::RGB(235, 242, 249),
        )?;
    }
    Ok(())
}

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
        for page in [Page::General, Page::Device, Page::Timezones, Page::Updates] {
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

    #[test]
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
            let mut canvas = window
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
            power_samples(&mut canvas, &layout, &mut state, output)?;
            state.settings.network_available = true;
            state.settings.input(Action::System);
            let creator = canvas.texture_creator();
            let textures = artwork(&creator, &state);
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("system-{w}x{h}.bmp")))?;
            state.settings.input(Action::SelectAndActivate(5));
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("device-{w}x{h}.bmp")))?;
            state.settings.input(Action::SelectAndActivate(3));
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("updates-{w}x{h}.bmp")))?;
            update_samples(&mut canvas, &layout, &mut state, &textures, output, (w, h))?;
            state.settings.input(Action::Back);
            state.settings.input(Action::SelectAndActivate(1));
            render(&mut canvas, &layout, &state, &textures)?;
            screenshot(&canvas, &output.join(format!("zones-{w}x{h}.bmp")))?;
            footer_focus_samples(&mut canvas, &layout, &mut state, &textures, output)?;
            state.settings.input(Action::Back);
            state.settings.input(Action::Back);
            state.settings.input(Action::SelectAndActivate(4));
            render(&mut canvas, &layout, &state, &[])?;
            screenshot(&canvas, &output.join(format!("confirm-{w}x{h}.bmp")))?;
            assert_eq!(state.settings.selected, 0);
            state.settings.cancel();
            state.settings.status = Status::default();
            state.settings.input(Action::System);
            render(&mut canvas, &layout, &state, &[])?;
            screenshot(&canvas, &output.join(format!("unavailable-{w}x{h}.bmp")))?;
            state.settings.cancel();
            state.opening = Some("Bitcoin CAD".into());
            render(&mut canvas, &layout, &state, &[])?;
            screenshot(&canvas, &output.join(format!("loading-{w}x{h}.bmp")))?;
            app_center::qa(&mut canvas, &layout, output)?;
            shortcuts::qa(&mut canvas, &layout, output)?;
        }
        Ok(())
    }
}
