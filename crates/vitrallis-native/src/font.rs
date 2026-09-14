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

const fn index(ch: char) -> u32 {
    match ch as u32 {
        value @ 0..=127 => value,
        value @ 160..=255 => 128 + value - 160,
        value @ 0x2500..=0x257f => 224 + value - 0x2500,
        _ => b'?' as u32,
    }
}

fn bitmap(slot: u32) -> [u8; 8] {
    let code = match slot {
        0..=127 => slot,
        128..=223 => slot - 128 + 160,
        _ => slot - 224 + 0x2500,
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

fn pixels() -> Vec<u8> {
    let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
    for slot in 0..GLYPHS {
        for (row, bits) in bitmap(slot).into_iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    let x = (slot as usize % 16) * 8 + col;
                    let y = (slot as usize / 16) * 8 + row;
                    let offset = (y * WIDTH as usize + x) * 4;
                    pixels[offset..offset + 4].fill(255);
                }
            }
        }
    }
    pixels
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
            .update(None, &pixels(), WIDTH as usize * 4)
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
            i32::try_from((slot % 16) * 8).map_err(|_| "glyph column")?,
            i32::try_from((slot / 16) * 8).map_err(|_| "glyph row")?,
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
        for scale in 1..=3 {
            let draw = |canvas: &mut Canvas<Window>,
                        atlas: Option<&mut Atlas<'_>>|
             -> Result<Vec<u8>, String> {
                canvas.set_clip_rect(None);
                canvas.set_draw_color(Color::RGB(17, 29, 40));
                canvas.clear();
                canvas.set_clip_rect(Rect::new(3, 5, 440, 650));
                let mut atlas = atlas;
                for slot in 0..GLYPHS {
                    let code = match slot {
                        0..=127 => slot,
                        128..=223 => slot - 128 + 160,
                        _ => slot - 224 + 0x2500,
                    };
                    let ch = char::from_u32(code).ok_or("test code point")?;
                    let x = i32::try_from(slot % 16).map_err(|_| "column")? * 8 * scale - 2;
                    let y = i32::try_from(slot / 16).map_err(|_| "row")? * 8 * scale - 1;
                    let color = if slot % 2 == 0 {
                        Color::RGB(93, 218, 201)
                    } else {
                        Color::RGB(235, 242, 249)
                    };
                    let size = scale.unsigned_abs();
                    if let Some(atlas) = atlas.as_deref_mut() {
                        atlas.draw(canvas, ch, Rect::new(x, y, 8 * size, 8 * size), color)?;
                    } else {
                        canvas.set_draw_color(color);
                        for (row, bits) in (0..8).zip(bitmap(slot)) {
                            for col in 0..8 {
                                if bits & (1 << col) != 0 {
                                    canvas.fill_rect(Rect::new(
                                        x + col * scale,
                                        y + row * scale,
                                        size,
                                        size,
                                    ))?;
                                }
                            }
                        }
                    }
                }
                canvas.set_clip_rect(None);
                canvas.read_pixels(None, PixelFormatEnum::RGBA32)
            };
            let expected = draw(canvas, None)?;
            let actual = draw(canvas, Some(atlas))?;
            assert_eq!(actual, expected, "atlas clipping / scale {scale}");
        }
        Ok(())
    }

    #[test]
    fn atlas_pixels_cover_every_supported_glyph_and_fallback() {
        let pixels = pixels();
        assert_eq!(pixels.len(), 88 * 1024);
        for (first, last) in [(0, 127), (160, 255), (0x2500, 0x257f)] {
            for code in first..=last {
                let ch = char::from_u32(code).unwrap();
                let slot = index(ch);
                let expected = bitmap(slot);
                for (row, bits) in expected.into_iter().enumerate() {
                    for col in 0..8 {
                        let x = slot as usize % 16 * 8 + col;
                        let y = slot as usize / 16 * 8 + row;
                        let offset = (y * WIDTH as usize + x) * 4;
                        let value = if bits & (1 << col) != 0 { 255 } else { 0 };
                        assert_eq!(
                            &pixels[offset..offset + 4],
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
    }
}
