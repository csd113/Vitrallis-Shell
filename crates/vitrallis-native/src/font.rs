//! Fixed font8x8 atlas shared by software and accelerated SDL renderers.
use font8x8::UnicodeFonts;
use sdl2::{
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{BlendMode, Canvas, ScaleMode, Texture, TextureCreator},
    video::{Window, WindowContext},
};

// 128 basic + 96 Latin + 128 box glyphs in 16 columns. 88 KiB RGBA.
const WIDTH: u32 = 128;
const HEIGHT: u32 = 176;
const GLYPHS: u32 = 352;

fn index(ch: char) -> u32 {
    match u32::from(ch) {
        value @ 0..=127 => value,
        value @ 160..=255 => value.saturating_sub(160).saturating_add(128),
        value @ 0x2500..=0x257f => value.saturating_sub(0x2500).saturating_add(224),
        _ => u32::from(b'?'),
    }
}

fn bitmap(slot: u32) -> [u8; 8] {
    let code = match slot {
        0..=127 => slot,
        128..=223 => slot.saturating_sub(128).saturating_add(160),
        224..=351 => slot.saturating_sub(224).saturating_add(0x2500),
        _ => return [0; 8],
    };
    char::from_u32(code)
        .and_then(|ch| {
            font8x8::BASIC_FONTS
                .get(ch)
                .or_else(|| font8x8::LATIN_FONTS.get(ch))
                .or_else(|| font8x8::BOX_FONTS.get(ch))
        })
        .unwrap_or([0; 8])
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "The immutable 352-glyph atlas is 128x176 RGBA; slots, rows, and columns are bounded by these constants, and offsets fit usize on every supported host"
)]
fn pixels() -> Result<Vec<u8>, String> {
    let width = usize::try_from(WIDTH).map_err(|error| error.to_string())?;
    let height = usize::try_from(HEIGHT).map_err(|error| error.to_string())?;
    let mut pixels = vec![0; width * height * 4];
    for slot in 0..GLYPHS {
        let slot_index = usize::try_from(slot).map_err(|error| error.to_string())?;
        for (row, bits) in bitmap(slot).into_iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    let x = (slot_index % 16) * 8 + col;
                    let y = (slot_index / 16) * 8 + row;
                    let offset = (y * width + x) * 4;
                    pixels
                        .get_mut(offset..offset + 4)
                        .ok_or("Glyph lies outside the font atlas")?
                        .fill(255);
                }
            }
        }
    }
    Ok(pixels)
}

/// One immutable pixel-art texture. The creator must outlive it; no raw GPU handles.
pub struct Atlas<'a> {
    texture: Texture<'a>,
    color: Color,
}
impl<'a> Atlas<'a> {
    /// # Errors
    /// Reports texture allocation or upload errors.
    pub fn new(creator: &'a TextureCreator<WindowContext>) -> Result<Self, String> {
        let mut texture = creator
            .create_texture_static(PixelFormatEnum::RGBA32, WIDTH, HEIGHT)
            .map_err(|e| e.to_string())?;
        texture
            .update(
                None,
                &pixels()?,
                usize::try_from(WIDTH)
                    .map_err(|error| error.to_string())?
                    .checked_mul(4)
                    .ok_or("Font pitch overflow")?,
            )
            .map_err(|e| e.to_string())?;
        texture.set_scale_mode(ScaleMode::Nearest);
        texture.set_blend_mode(BlendMode::Blend);
        Ok(Self {
            texture,
            color: Color::RGBA(255, 255, 255, 255),
        })
    }

    /// Draw a glyph at the caller's existing integer position and scale.
    /// # Errors
    /// Reports invalid scale or SDL copy errors.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        ch: char,
        destination: Rect,
        color: Color,
    ) -> Result<(), String> {
        let slot = index(ch);
        // Blank characters retain their layout advance without a GPU operation.
        if matches!(slot, 0..=32 | 127 | 128) {
            return Ok(());
        }
        if self.color != color {
            self.texture.set_color_mod(color.r, color.g, color.b);
            self.texture.set_alpha_mod(color.a);
            self.color = color;
        }
        let source = Rect::new(
            i32::try_from((slot % 16).saturating_mul(8))
                .map_err(|error| format!("glyph column: {error}"))?,
            i32::try_from((slot / 16).saturating_mul(8))
                .map_err(|error| format!("glyph row: {error}"))?,
            8,
            8,
        );
        canvas.copy(&self.texture, source, destination)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn pixel_parity(canvas: &mut Canvas<Window>, atlas: &mut Atlas<'_>) -> Result<(), String> {
        for scale in 1_i32..=3_i32 {
            let draw = |target: &mut Canvas<Window>,
                        mut font: Option<&mut Atlas<'_>>|
             -> Result<Vec<u8>, String> {
                target.set_clip_rect(None);
                target.set_draw_color(Color::RGB(17, 29, 40));
                target.clear();
                target.set_clip_rect(Rect::new(3, 5, 440, 650));
                for slot in 0..GLYPHS {
                    let code = match slot {
                        0..=127 => slot,
                        128..=223 => slot.saturating_sub(128).saturating_add(160),
                        _ => slot.saturating_sub(224).saturating_add(0x2500),
                    };
                    let ch = char::from_u32(code).ok_or("test code point")?;
                    let x = i32::try_from(slot % 16)
                        .map_err(|error| format!("column: {error}"))?
                        .saturating_mul(8)
                        .saturating_mul(scale)
                        .saturating_sub(2);
                    let y = i32::try_from(slot / 16)
                        .map_err(|error| format!("row: {error}"))?
                        .saturating_mul(8)
                        .saturating_mul(scale)
                        .saturating_sub(1);
                    let color = if slot % 2 == 0 {
                        Color::RGB(93, 218, 201)
                    } else {
                        Color::RGB(235, 242, 249)
                    };
                    let size = scale.unsigned_abs();
                    if let Some(active_font) = font.as_deref_mut() {
                        active_font.draw(
                            target,
                            ch,
                            Rect::new(x, y, 8_u32.saturating_mul(size), 8_u32.saturating_mul(size)),
                            color,
                        )?;
                    } else {
                        target.set_draw_color(color);
                        for (row, bits) in (0_i32..8_i32).zip(bitmap(slot)) {
                            for col in 0_i32..8_i32 {
                                if bits & (1 << col) != 0 {
                                    target.fill_rect(Rect::new(
                                        x.saturating_add(col.saturating_mul(scale)),
                                        y.saturating_add(row.saturating_mul(scale)),
                                        size,
                                        size,
                                    ))?;
                                }
                            }
                        }
                    }
                }
                target.set_clip_rect(None);
                target.read_pixels(None, PixelFormatEnum::RGBA32)
            };
            let expected = draw(canvas, None)?;
            let actual = draw(canvas, Some(atlas))?;
            assert_eq!(actual, expected, "atlas clipping / scale {scale}");
        }
        Ok(())
    }

    #[test]
    fn atlas_pixels_cover_every_supported_glyph_and_fallback() -> Result<(), String> {
        let pixels = pixels()?;
        assert_eq!(pixels.len(), 88 * 1024);
        for (first, last) in [(0, 127), (160, 255), (0x2500, 0x257f)] {
            for code in first..=last {
                let ch = char::from_u32(code)
                    .ok_or_else(|| format!("U+{code:04X} is not a scalar value"))?;
                let slot = index(ch);
                let expected = bitmap(slot);
                for (row, bits) in expected.into_iter().enumerate() {
                    for col in 0..8 {
                        let slot_index =
                            usize::try_from(slot).map_err(|error| error.to_string())?;
                        let x = (slot_index % 16).saturating_mul(8).saturating_add(col);
                        let y = (slot_index / 16).saturating_mul(8).saturating_add(row);
                        let width = usize::try_from(WIDTH).map_err(|error| error.to_string())?;
                        let offset = y.saturating_mul(width).saturating_add(x).saturating_mul(4);
                        let value = if bits & (1 << col) != 0 { 255 } else { 0 };
                        assert_eq!(
                            pixels
                                .get(offset..offset.saturating_add(4))
                                .ok_or("Missing test glyph pixel")?,
                            &[value; 4],
                            "{ch:?} {row} {col}"
                        );
                    }
                }
                if matches!(slot, 0..=32 | 127 | 128) {
                    assert_eq!(expected, [0; 8]);
                }
            }
        }
        for ch in ['\u{80}', '\u{9f}', '日', '😀', '\u{10ffff}'] {
            assert_eq!(index(ch), index('?'));
        }
        assert_eq!(index('\u{a0}'), 128);
        assert_eq!(index('\u{ff}'), 223);
        assert_eq!(index('\u{2500}'), 224);
        assert_eq!(index('\u{257f}'), GLYPHS - 1);
        Ok(())
    }
}
