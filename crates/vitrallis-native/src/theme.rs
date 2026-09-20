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
pub const INSET: i32 = 8;
pub const BORDER_WIDTH: i32 = 1;

/// Integer font scaling for the Shell at its supported display heights.
#[must_use]
pub const fn text_scale(height: i32) -> i32 {
    let scale = (height + 80) / 272;
    if scale < 1 { 1 } else { scale }
}

/// One crisp panel primitive, including visible focus even on disabled controls.
/// # Errors
/// Returns SDL drawing errors.
pub fn card(canvas: &mut Canvas<Window>, bounds: Rect, focused: bool) -> Result<(), String> {
    canvas.set_draw_color(if focused { SELECTED } else { PANEL });
    canvas.fill_rect(bounds)?;
    canvas.set_draw_color(if focused { ACCENT } else { BORDER });
    canvas.draw_rect(bounds)?;
    if focused && bounds.width() > 6 && bounds.height() > 6 {
        // Inset edges, never an outer blur that can cover neighboring text.
        canvas.set_draw_color(BLUE);
        canvas.draw_line(
            (bounds.x() + 1, bounds.y() + 2),
            (bounds.x() + 1, bounds.bottom() - 3),
        )?;
        canvas.set_draw_color(VIOLET);
        canvas.draw_line(
            (
                bounds.right()
                    - 12.min(i32::try_from(bounds.width()).map_err(|_| "panel width")? - 2),
                bounds.bottom() - 2,
            ),
            (bounds.right() - 2, bounds.bottom() - 2),
        )?;
    }
    Ok(())
}

/// Stepped cyan/blue/violet fill with no per-pixel gradients or animated state.
/// `filled` is clamped to the track; warning capacity bars retain an alert color.
/// # Errors
/// Returns SDL drawing errors.
pub fn progress(
    canvas: &mut Canvas<Window>,
    bounds: Rect,
    filled: u32,
    warning: bool,
) -> Result<(), String> {
    canvas.set_draw_color(TRACK);
    canvas.fill_rect(bounds)?;
    let filled = filled.min(bounds.width());
    for (index, color) in [ACCENT, BLUE, VIOLET].into_iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| "progress segment")?;
        let start = bounds.width() * index / 3;
        let end = (bounds.width() * (index + 1) / 3).min(filled);
        if end > start {
            canvas.set_draw_color(if warning { WARNING } else { color });
            canvas.fill_rect(Rect::new(
                bounds.x() + i32::try_from(start).map_err(|_| "progress width")?,
                bounds.y(),
                end - start,
                bounds.height(),
            ))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(c: Color) -> f64 {
        let component = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
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
                let contrast = (luminance(ink) + 0.05) / (luminance(surface) + 0.05);
                assert!(contrast >= 4.5, "{ink:?} on {surface:?}: {contrast}");
            }
        }
        assert_eq!(text_scale(272), 1);
        assert_eq!(text_scale(480), 2);
    }
}
