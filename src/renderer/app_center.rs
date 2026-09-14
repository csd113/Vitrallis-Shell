use super::{Screen, fill, text};
use crate::{
    app_center::Center,
    layout::{Layout, Rect},
};
use sdl2::pixels::Color;
pub fn panel(canvas: &mut Screen, layout: &Layout, center: &Center) -> Result<(), String> {
    let width = i32::from(layout.width);
    let height = i32::from(layout.height);
    fill(
        canvas,
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        },
        Color::RGB(14, 24, 34),
    )?;
    text(
        canvas,
        center.title(),
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: 24,
        },
        1,
        Color::RGB(93, 218, 201),
    )?;
    if let Some(pixels) = center.detail_icon() {
        draw_icon(
            canvas,
            pixels,
            Rect {
                x: 8,
                y: 0,
                w: 24,
                h: 24,
            },
        )?;
    }
    let targets = center.targets(layout);
    for (i, (target, label, bounds)) in targets.iter().enumerate() {
        let enabled = center.enabled(target);
        fill(
            canvas,
            *bounds,
            if i == center.selected {
                Color::RGB(42, 77, 92)
            } else {
                Color::RGB(33, 45, 58)
            },
        )?;
        if i == center.selected {
            canvas.set_draw_color(Color::RGB(120, 240, 220));
            canvas.draw_rect(super::rect(*bounds)?)?;
        }
        let color = if enabled {
            Color::RGB(239, 241, 245)
        } else {
            Color::RGB(139, 149, 159)
        };
        if let Some((name, description, status)) = center.row_content(target) {
            if center.row_chosen(target) {
                fill(canvas, Rect { w: 3, ..*bounds }, Color::RGB(93, 218, 201))?;
            }
            app_row(
                canvas,
                center.row_icon(target),
                (name, description, &status),
                *bounds,
            )?;
        } else {
            text(canvas, label, *bounds, 1, color)?;
        }
    }
    body(canvas, layout, center)
}
fn body(canvas: &mut Screen, layout: &Layout, center: &Center) -> Result<(), String> {
    let width = i32::from(layout.width);
    let height = i32::from(layout.height);
    let lines = center.lines(usize::from(layout.width) / 8 - 2);
    let (y, count) = if center.full_details() {
        (68, usize::from(layout.height.saturating_sub(124) / 12))
    } else if center.editing() {
        (68, 3)
    } else {
        (height - 26, 2)
    };
    for (i, line) in lines
        .iter()
        .skip(center.detail_start())
        .take(count)
        .enumerate()
    {
        left(
            canvas,
            line,
            Rect {
                x: 8,
                y: y + i32::try_from(i).map_err(|_| "Too many lines")? * 12,
                w: width - 16,
                h: 12,
            },
            Color::RGB(220, 224, 231),
        )?;
    }
    if let Some(message) = center.empty_message() {
        text(
            canvas,
            message,
            Rect {
                x: 8,
                y: 130,
                w: width - 16,
                h: 32,
            },
            1,
            Color::RGB(181, 199, 211),
        )?;
    }
    if center.full_details() || center.editing() {
        text(
            canvas,
            center.footer(),
            Rect {
                x: 4,
                y: height - 12,
                w: width - 8,
                h: 12,
            },
            1,
            Color::RGB(239, 206, 129),
        )?;
    }
    Ok(())
}

// Fit text explicitly so titles and long errors end with a visible ellipsis.
fn left(canvas: &mut Screen, value: &str, bounds: Rect, color: Color) -> Result<(), String> {
    let limit = usize::try_from(bounds.w / 8).map_err(|_| "Text width")?;
    let value = value
        .replace(['—', '–'], "-")
        .replace(['’', '‘'], "'")
        .replace(['“', '”'], "\"")
        .replace('•', "-");
    let shown = if value.chars().count() > limit && limit > 3 {
        format!("{}...", value.chars().take(limit - 3).collect::<String>())
    } else {
        value.chars().take(limit).collect()
    };
    let w = i32::try_from(shown.chars().count()).map_err(|_| "Text width")? * 8;
    text(canvas, &shown, Rect { w, ..bounds }, 1, color)
}

fn app_row(
    canvas: &mut Screen,
    icon: Option<&[u8]>,
    (name, description, status): (&str, &str, &str),
    bounds: Rect,
) -> Result<(), String> {
    let icon_bounds = Rect {
        x: bounds.x + 8,
        y: bounds.y + 8,
        w: 32,
        h: 32,
    };
    if let Some(pixels) = icon {
        draw_icon(canvas, pixels, icon_bounds)?;
    } else {
        fill(canvas, icon_bounds, Color::RGB(43, 93, 103))?;
        text(
            canvas,
            &name.chars().take(1).collect::<String>(),
            icon_bounds,
            2,
            Color::RGB(210, 246, 237),
        )?;
    }
    let text_bounds = Rect {
        x: bounds.x + 50,
        y: bounds.y + 2,
        w: bounds.w - 58,
        h: 14,
    };
    left(canvas, name, text_bounds, Color::RGB(245, 249, 251))?;
    left(
        canvas,
        description,
        Rect {
            y: bounds.y + 17,
            ..text_bounds
        },
        Color::RGB(172, 190, 205),
    )?;
    left(
        canvas,
        status,
        Rect {
            y: bounds.y + 32,
            ..text_bounds
        },
        if status.starts_with("Update") {
            Color::RGB(158, 243, 180)
        } else {
            Color::RGB(103, 215, 203)
        },
    )
}
pub(super) fn draw_icon(canvas: &mut Screen, pixels: &[u8], bounds: Rect) -> Result<(), String> {
    canvas.center_icon(pixels, bounds)
}

#[cfg(test)]
pub fn qa(canvas: &mut Screen, layout: &Layout, output: &std::path::Path) -> Result<(), String> {
    if layout.width < 480 {
        return Ok(());
    }
    for (name, center) in Center::qa_samples()? {
        panel(canvas, layout, &center)?;
        super::screenshot(
            canvas,
            &output.join(format!(
                "app-center-{name}-{}x{}.bmp",
                layout.width, layout.height
            )),
        )?;
    }
    Ok(())
}
