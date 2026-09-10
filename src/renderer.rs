use crate::{
    app::App,
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
    apps: &[App],
) -> Vec<Option<Texture<'a>>> {
    apps.iter()
        .map(|app| {
            let path = app.icon.as_ref()?;
            // Bound decoding before loading. Step 1 supports only small BMP assets.
            let load = || -> Result<Texture<'a>, String> {
                let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
                if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                    return Err("icon must be a regular BMP file".into());
                }
                let mut bytes = Vec::new();
                file.take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                validate_bmp(&bytes)?;
                let mut rw = sdl2::rwops::RWops::from_bytes(&bytes)?;
                let surface = Surface::load_bmp_rw(&mut rw)?;
                if surface.width() > 512 || surface.height() > 512 {
                    return Err("icon exceeds 512x512".into());
                }
                creator
                    .create_texture_from_surface(&surface)
                    .map_err(|e| e.to_string())
            };
            match load() {
                Ok(texture) => Some(texture),
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

pub fn render(
    canvas: &mut Screen,
    layout: &Layout,
    state: &Launcher,
    icons: &[Option<Texture<'_>>],
) -> Result<(), String> {
    canvas.set_draw_color(Color::RGB(13, 22, 33));
    canvas.clear();
    text(
        canvas,
        "VITRALLIS",
        layout.title,
        layout.text_scale + 1,
        Color::RGB(93, 218, 201),
    )?;
    for (index, (app, tile)) in state.apps.iter().zip(&layout.tiles).enumerate() {
        fill(
            canvas,
            *tile,
            if index == state.selected {
                Color::RGB(44, 82, 99)
            } else {
                Color::RGB(25, 40, 55)
            },
        )?;
        if index == state.selected {
            canvas.set_draw_color(Color::RGB(93, 218, 201));
            canvas.draw_rect(rect(*tile)?)?;
        }
        let icon = Rect {
            x: tile.x + (tile.w - layout.icon_size) / 2,
            y: tile.y + tile.h / 12,
            w: layout.icon_size,
            h: layout.icon_size,
        };
        if let Some(Some(texture)) = icons.get(index) {
            canvas.copy(texture, None, rect(icon)?)?;
        } else {
            fill(canvas, icon, Color::RGB(57, 115, 137))?;
            let mark = match index {
                0 => ">_",
                1 => "[]",
                2 => "!",
                3 => "?",
                4 => "Aa",
                _ => "+",
            };
            text(
                canvas,
                mark,
                icon,
                layout.text_scale + 1,
                Color::RGB(219, 243, 240),
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
    }
    text(
        canvas,
        &state.status,
        layout.footer,
        layout.text_scale,
        Color::RGB(173, 194, 210),
    )?;
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
