//! Vitrallis CLEAN surfaces and pixel metrics. Only focused/active controls carry
//! the crystal's cyan and violet edge light; full effects belong to startup.
use sdl2::{pixels::Color, rect::Rect, render::Canvas, video::Window};

pub const BACKGROUND: Color = Color::RGB(9, 13, 27);
pub const PANEL: Color = Color::RGB(17, 25, 48);
pub const SELECTED: Color = Color::RGB(26, 39, 72);
pub const BORDER: Color = Color::RGB(47, 66, 108);
pub const TRACK: Color = Color::RGB(29, 40, 68);
pub const TEXT: Color = Color::RGB(226, 227, 255);
pub const MUTED: Color = Color::RGB(159, 177, 216);
pub const DISABLED: Color = Color::RGB(155, 163, 193);
pub const ACCENT: Color = Color::RGB(112, 215, 255);
pub const BLUE: Color = Color::RGB(82, 135, 255);
pub const VIOLET: Color = Color::RGB(164, 137, 255);
pub const WARNING: Color = Color::RGB(255, 177, 215);
pub const ERROR_SURFACE: Color = Color::RGB(39, 25, 49);

pub const CELL: i32 = 8;
pub const LINE: i32 = 12;
pub const SPACE: i32 = 4;

/// Integer font scaling for the Shell at its supported display heights.
#[must_use]
pub const fn text_scale(height: i32) -> i32 {
    let scale = height.saturating_add(80_i32) / 272_i32;
    if scale < 1 { 1 } else { scale }
}

/// One crisp panel primitive, including visible focus even on disabled controls.
///
/// The border is exactly one pixel on each of the four sides and nothing else is
/// drawn inside the rectangle. Focus changes the border and fill colour only, so
/// no inset mark can read as a stray line along one edge.
/// # Errors
/// Returns SDL drawing errors.
pub fn card(canvas: &mut Canvas<Window>, bounds: Rect, focused: bool) -> Result<(), String> {
    if bounds.width() == 0 || bounds.height() == 0 {
        return Ok(());
    }
    canvas.set_draw_color(if focused { SELECTED } else { PANEL });
    canvas.fill_rect(bounds)?;
    canvas.set_draw_color(if focused { ACCENT } else { BORDER });
    // SDL line rasterization differs at shared endpoints across backends. Use
    // half-open filled strips so no border pixel can escape the rectangle.
    canvas.fill_rect(Rect::new(bounds.x(), bounds.y(), bounds.width(), 1))?;
    canvas.fill_rect(Rect::new(
        bounds.x(),
        bounds.bottom().saturating_sub(1),
        bounds.width(),
        1,
    ))?;
    canvas.fill_rect(Rect::new(bounds.x(), bounds.y(), 1, bounds.height()))?;
    canvas.fill_rect(Rect::new(
        bounds.right().saturating_sub(1),
        bounds.y(),
        1,
        bounds.height(),
    ))?;
    Ok(())
}

/// Integer palette blend; `amount` 0 keeps `from`, 255 reaches `to`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "Every channel and amount is u8: the signed interpolation intermediate is bounded by 65025 and the result remains in 0..=255"
)]
fn mix(from: Color, to: Color, amount: u8) -> Color {
    let blend = |a: u8, b: u8| {
        let start = i32::from(a);
        let end = i32::from(b);
        u8::try_from(start + (end - start) * i32::from(amount) / 255_i32).unwrap_or(a)
    };
    Color::RGB(
        blend(from.r, to.r),
        blend(from.g, to.g),
        blend(from.b, to.b),
    )
}

/// Continuous cyan to blue to violet ramp, computed once for the whole session.
fn ramp(index: u8) -> Color {
    static RAMP: std::sync::OnceLock<[Color; 256]> = std::sync::OnceLock::new();
    let table = RAMP.get_or_init(|| {
        std::array::from_fn(|i| {
            let step = u8::try_from(i).unwrap_or(0);
            if step < 128 {
                mix(ACCENT, BLUE, step.saturating_mul(2))
            } else {
                mix(BLUE, VIOLET, step.saturating_sub(128).saturating_mul(2))
            }
        })
    });
    table.get(usize::from(index)).copied().unwrap_or(ACCENT)
}

/// Ramp position for a pixel at `position` inside a track of `width` pixels.
/// The ramp belongs to the track, so the colour of a pixel never changes with
/// the current value; only the amount of revealed track changes.
fn ramp_index(position: i32, width: i32) -> u8 {
    let span = width.max(2_i32).saturating_sub(1_i32);
    let weighted = i64::from(position.clamp(0_i32, span)).saturating_mul(255_i64);
    u8::try_from(weighted.checked_div(i64::from(span)).unwrap_or(0)).unwrap_or(0)
}

/// Strip width that keeps the ramp visually continuous without drawing one
/// rectangle per pixel. At most 128 strips are drawn for any track width.
fn strip_width(width: i32) -> i32 {
    (width / 128).max(1)
}

/// Fills the revealed part of a track with the continuous ramp.
fn ramp_fill(
    canvas: &mut Canvas<Window>,
    bounds: Rect,
    filled: i32,
    dim: u8,
) -> Result<(), String> {
    let width = i32::try_from(bounds.width()).unwrap_or(i32::MAX);
    let visible = filled.clamp(0_i32, width);
    let step = strip_width(width);
    let mut x = 0_i32;
    while x < visible {
        let strip = step.min(visible.saturating_sub(x));
        let mut color = ramp(ramp_index(x.saturating_add(strip / 2_i32), width));
        if dim > 0 {
            color = mix(color, TRACK, dim);
        }
        canvas.set_draw_color(color);
        canvas.fill_rect(Rect::new(
            bounds.x().saturating_add(x),
            bounds.y(),
            u32::try_from(strip).unwrap_or(0),
            bounds.height(),
        ))?;
        x = x.saturating_add(strip);
    }
    Ok(())
}

/// Continuous cyan/blue/violet fill. `filled` is clamped to the track; warning
/// capacity bars retain one alert color. No per-frame gradient allocation.
/// # Errors
/// Returns SDL drawing errors.
pub fn progress(
    canvas: &mut Canvas<Window>,
    bounds: Rect,
    filled: u32,
    warning: bool,
) -> Result<(), String> {
    if bounds.width() == 0 || bounds.height() == 0 {
        return Ok(());
    }
    let previous = canvas.blend_mode();
    canvas.set_blend_mode(sdl2::render::BlendMode::None);
    let result = progress_opaque(canvas, bounds, filled, warning);
    canvas.set_blend_mode(previous);
    result
}

fn progress_opaque(
    canvas: &mut Canvas<Window>,
    bounds: Rect,
    filled: u32,
    warning: bool,
) -> Result<(), String> {
    canvas.set_draw_color(TRACK);
    canvas.fill_rect(bounds)?;
    let width = i32::try_from(bounds.width()).unwrap_or(i32::MAX);
    let visible = i32::try_from(filled).unwrap_or(i32::MAX).min(width);
    if visible <= 0_i32 {
        return Ok(());
    }
    if warning {
        canvas.set_draw_color(WARNING);
        return canvas.fill_rect(Rect::new(
            bounds.x(),
            bounds.y(),
            u32::try_from(visible).unwrap_or(0),
            bounds.height(),
        ));
    }
    ramp_fill(canvas, bounds, visible, 0)
}

/// Adjustable value track with three distinct visible states.
///
/// The inactive track is the surface colour, the active track uses a dimmed ramp
/// so a slider never competes with a progress bar, and the thumb is a solid
/// marker that brightens with keyboard focus. `value` is a percentage; `None`
/// draws an unavailable track without a thumb.
/// # Errors
/// Returns SDL drawing errors.
pub fn slider(
    canvas: &mut Canvas<Window>,
    bounds: Rect,
    value: Option<u8>,
    focused: bool,
) -> Result<(), String> {
    if bounds.width() == 0 || bounds.height() == 0 {
        return Ok(());
    }
    let previous = canvas.blend_mode();
    canvas.set_blend_mode(sdl2::render::BlendMode::None);
    let result = slider_opaque(canvas, bounds, value, focused);
    canvas.set_blend_mode(previous);
    result
}

fn slider_opaque(
    canvas: &mut Canvas<Window>,
    bounds: Rect,
    value: Option<u8>,
    focused: bool,
) -> Result<(), String> {
    canvas.set_draw_color(TRACK);
    canvas.fill_rect(bounds)?;
    let Some(percentage) = value else {
        return Ok(());
    };
    let width = i32::try_from(bounds.width()).unwrap_or(i32::MAX);
    let height = i32::try_from(bounds.height()).unwrap_or(i32::MAX);
    // A dimmed ramp keeps the handle legible without a fully saturated track.
    let filled =
        i32::try_from(i64::from(width).saturating_mul(i64::from(percentage.min(100))) / 100_i64)
            .unwrap_or(width);
    ramp_fill(canvas, bounds, filled, 112)?;
    let overhang = (height / 2_i32).saturating_add(2_i32).max(2_i32);
    let thumb = Rect::new(
        bounds.x().saturating_add(filled).saturating_sub(1),
        bounds.y().saturating_sub(overhang),
        3,
        u32::try_from(height.saturating_add(2_i32.saturating_mul(overhang))).unwrap_or(0),
    );
    canvas.set_draw_color(BORDER);
    canvas.fill_rect(Rect::new(
        thumb.x().saturating_sub(1),
        thumb.y().saturating_sub(1),
        thumb.width().saturating_add(2),
        thumb.height().saturating_add(2),
    ))?;
    canvas.set_draw_color(if focused { ACCENT } else { TEXT });
    canvas.fill_rect(thumb)?;
    // A one-pixel dark core keeps the marker readable over every ramp colour.
    canvas.set_draw_color(BACKGROUND);
    canvas.fill_rect(Rect::new(
        thumb.x().saturating_add(1),
        thumb.y().saturating_add(1),
        1,
        thumb.height().saturating_sub(2),
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    /// Reads one RGB24 pixel, reporting the coordinates instead of panicking when
    /// the sample cannot be addressed in the captured buffer.
    fn pixel(pixels: &[u8], width: i32, x: i32, y: i32) -> Result<[u8; 3], String> {
        if width <= 0_i32 || !(0_i32..width).contains(&x) || y < 0_i32 {
            return Err(format!("invalid pixel {x},{y} in a {width}-wide buffer"));
        }
        let offset = i64::from(y)
            .checked_mul(i64::from(width))
            .and_then(|row| row.checked_add(i64::from(x)))
            .and_then(|position| position.checked_mul(3))
            .ok_or("pixel offset overflow")?;
        let start =
            usize::try_from(offset).map_err(|error| format!("pixel offset {offset}: {error}"))?;
        let end = start.checked_add(3).ok_or("pixel range overflow")?;
        let rgb = pixels
            .get(start..end)
            .ok_or_else(|| format!("pixel {x},{y} is outside the {width}-wide buffer"))?;
        let [red, green, blue] = rgb
            .try_into()
            .map_err(|error| format!("pixel {x},{y} is not exactly three channels: {error}"))?;
        Ok([red, green, blue])
    }
    pub fn primitive_pixels(canvas: &mut Canvas<Window>) -> Result<(), String> {
        use sdl2::{pixels::PixelFormatEnum, render::BlendMode};
        let bounds = Rect::new(7, 9, 31, 13);
        for amount in [0, 1, 10, 11, 20, 21, 30, 31, 99] {
            for warning in [false, true] {
                canvas.set_draw_color(BACKGROUND);
                canvas.clear();
                canvas.set_blend_mode(BlendMode::Add);
                progress(canvas, bounds, amount, warning)?;
                assert_eq!(canvas.blend_mode(), BlendMode::Add);
                let pixels = canvas.read_pixels(Rect::new(0, 0, 40, 24), PixelFormatEnum::RGB24)?;
                let filled = i32::try_from(amount)
                    .unwrap_or(0_i32)
                    .min(i32::try_from(bounds.width()).unwrap_or(i32::MAX));
                let track = [TRACK.r, TRACK.g, TRACK.b];
                for y in 0_i32..24_i32 {
                    for x in 0_i32..40_i32 {
                        let actual = pixel(&pixels, 40, x, y)?;
                        if !bounds.contains_point((x, y)) {
                            assert_eq!(actual, [BACKGROUND.r, BACKGROUND.g, BACKGROUND.b]);
                            continue;
                        }
                        if x.saturating_sub(bounds.x()) >= filled {
                            assert_eq!(actual, track, "unfilled track pixel {x},{y}");
                        } else if warning {
                            assert_eq!(actual, [WARNING.r, WARNING.g, WARNING.b]);
                        }
                    }
                }
            }
        }
        canvas.set_blend_mode(BlendMode::None);
        stage_border_geometry(canvas, bounds)?;
        stage_ramp_continuity(canvas)?;
        stage_slider_states(canvas)?;
        Ok(())
    }

    /// Every focused and unfocused panel keeps an exactly one-pixel border on all
    /// four sides and a fill without any inset mark that could read as a stray
    /// line along one edge.
    fn stage_border_geometry(canvas: &mut Canvas<Window>, bounds: Rect) -> Result<(), String> {
        use sdl2::pixels::PixelFormatEnum;
        for focused in [false, true] {
            canvas.set_draw_color(BACKGROUND);
            canvas.clear();
            card(canvas, bounds, focused)?;
            let pixels = canvas.read_pixels(Rect::new(0, 0, 40, 24), PixelFormatEnum::RGB24)?;
            let edge = if focused {
                [ACCENT.r, ACCENT.g, ACCENT.b]
            } else {
                [BORDER.r, BORDER.g, BORDER.b]
            };
            let surface = if focused {
                [SELECTED.r, SELECTED.g, SELECTED.b]
            } else {
                [PANEL.r, PANEL.g, PANEL.b]
            };
            for y in 0_i32..24_i32 {
                for x in 0_i32..40_i32 {
                    if !bounds.contains_point((x, y)) {
                        assert_eq!(
                            pixel(&pixels, 40, x, y)?,
                            [BACKGROUND.r, BACKGROUND.g, BACKGROUND.b]
                        );
                        continue;
                    }
                    let border = x == bounds.x()
                        || y == bounds.y()
                        || x == bounds.right().saturating_sub(1)
                        || y == bounds.bottom().saturating_sub(1);
                    let expected = if border { edge } else { surface };
                    assert_eq!(pixel(&pixels, 40, x, y)?, expected, "pixel {x},{y}");
                }
            }
        }
        Ok(())
    }

    /// The ramp must be continuous: neighbouring pixels never jump by more than
    /// a few levels, and a pixel keeps its colour as the value changes.
    fn stage_ramp_continuity(canvas: &mut Canvas<Window>) -> Result<(), String> {
        use sdl2::pixels::PixelFormatEnum;
        let track = Rect::new(3, 4, 300, 6);
        let mut previous: Option<[u8; 3]> = None;
        let mut sampled = Vec::new();
        canvas.set_draw_color(BACKGROUND);
        canvas.clear();
        progress(canvas, track, track.width(), false)?;
        let pixels = canvas.read_pixels(Rect::new(0, 0, 306, 20), PixelFormatEnum::RGB24)?;
        for x in track.x()..track.right() {
            let value = pixel(&pixels, 306, x, track.y().saturating_add(1))?;
            if let Some(prior_pixel) = previous {
                for (channel, prior_channel) in value.into_iter().zip(prior_pixel) {
                    assert!(
                        channel.abs_diff(prior_channel) <= 2,
                        "ramp step {value:?} after {prior_pixel:?}"
                    );
                }
            }
            previous = Some(value);
            sampled.push(value);
        }
        let [first_red, _, first_blue] = sampled.first().ok_or("missing first ramp pixel")?;
        assert!(first_blue > first_red, "ramp starts cyan");
        let [last_red, last_green, _] = sampled.last().ok_or("missing last ramp pixel")?;
        assert!(last_red > last_green, "ramp ends violet");
        // A partial fill reveals the same track-anchored ramp.
        canvas.set_draw_color(BACKGROUND);
        canvas.clear();
        progress(canvas, track, 150, false)?;
        let partial = canvas.read_pixels(Rect::new(0, 0, 306, 20), PixelFormatEnum::RGB24)?;
        for (index, expected) in sampled.iter().enumerate().take(150) {
            let x = track
                .x()
                .saturating_add(i32::try_from(index).unwrap_or(0_i32));
            assert_eq!(
                pixel(&partial, 306, x, track.y().saturating_add(1))?,
                *expected,
                "revealed ramp moved at {x}"
            );
        }
        for x in track.x().saturating_add(150)..track.right() {
            assert_eq!(
                pixel(&partial, 306, x, track.y().saturating_add(1))?,
                [TRACK.r, TRACK.g, TRACK.b]
            );
        }
        Ok(())
    }

    /// Slider states stay distinguishable at minimum, middle and maximum values.
    fn stage_slider_states(canvas: &mut Canvas<Window>) -> Result<(), String> {
        use sdl2::pixels::PixelFormatEnum;
        let track = Rect::new(3, 4, 200, 6);
        for value in [0, 50, 100] {
            for focused in [false, true] {
                canvas.set_draw_color(BACKGROUND);
                canvas.clear();
                slider(canvas, track, Some(value), focused)?;
                let pixels =
                    canvas.read_pixels(Rect::new(0, 0, 210, 20), PixelFormatEnum::RGB24)?;
                let fill = i32::try_from(track.width())
                    .unwrap_or(0_i32)
                    .saturating_mul(i32::from(value))
                    / 100_i32;
                // Unavailable or unfilled track stays the inactive colour.
                if fill == 0_i32 {
                    assert_eq!(
                        pixel(
                            &pixels,
                            210,
                            track.right().saturating_sub(1),
                            track.y().saturating_add(2)
                        )?,
                        [TRACK.r, TRACK.g, TRACK.b]
                    );
                } else {
                    let active = pixel(&pixels, 210, track.x(), track.y().saturating_add(2))?;
                    assert_ne!(active, [TRACK.r, TRACK.g, TRACK.b], "active track hidden");
                }
                let thumb = pixel(
                    &pixels,
                    210,
                    track.x().saturating_add(fill).saturating_sub(1),
                    track.y().saturating_add(2),
                )?;
                let expected = if focused {
                    [ACCENT.r, ACCENT.g, ACCENT.b]
                } else {
                    [TEXT.r, TEXT.g, TEXT.b]
                };
                assert_eq!(thumb, expected, "thumb state at value {value}");
            }
        }
        canvas.set_draw_color(BACKGROUND);
        canvas.clear();
        slider(canvas, track, None, true)?;
        let pixels = canvas.read_pixels(Rect::new(0, 0, 210, 20), PixelFormatEnum::RGB24)?;
        for x in track.x()..track.right() {
            assert_eq!(
                pixel(&pixels, 210, x, track.y().saturating_add(2))?,
                [TRACK.r, TRACK.g, TRACK.b]
            );
        }
        Ok(())
    }
    fn luminance(c: Color) -> f64 {
        let component = |v: u8| {
            let normalized = f64::from(v) / 255.0_f64;
            if normalized <= 0.040_45_f64 {
                normalized / 12.92_f64
            } else {
                ((normalized + 0.055) / 1.055).powf(2.4)
            }
        };
        0.0722_f64.mul_add(
            component(c.b),
            0.2126_f64.mul_add(component(c.r), 0.7152 * component(c.g)),
        )
    }
    #[test]
    fn small_text_and_focus_keep_contrast_on_all_surfaces() {
        for surface in [BACKGROUND, PANEL, SELECTED, ERROR_SURFACE] {
            for ink in [TEXT, MUTED, DISABLED, ACCENT, VIOLET, WARNING] {
                let contrast = (luminance(ink) + 0.05_f64) / (luminance(surface) + 0.05_f64);
                assert!(contrast >= 4.5_f64, "{ink:?} on {surface:?}: {contrast}");
            }
        }
        assert_eq!(text_scale(272), 1_i32);
        assert_eq!(text_scale(480), 2_i32);
    }
}
