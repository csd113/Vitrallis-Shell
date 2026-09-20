use super::{Screen, fill, text};
use crate::{
    app_center::Center,
    layout::{Layout, Rect},
};
use sdl2::pixels::Color;
use vitrallis_native::theme;
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
        theme::BACKGROUND,
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
        theme::ACCENT,
    )?;
    fill(
        canvas,
        Rect {
            x: theme::INSET,
            y: 25,
            w: width - 2 * theme::INSET,
            h: 1,
        },
        theme::BORDER,
    )?;
    if center.busy {
        // Activity treatment without a fabricated percentage or idle animation.
        super::progress(
            canvas,
            Rect {
                x: theme::INSET,
                y: 25,
                w: width - 2 * theme::INSET,
                h: 2,
            },
            width - 16,
            false,
        )?;
    }
    if let Some(pixels) = center.detail_icon() {
        draw_icon(
            canvas,
            pixels,
            Rect {
                x: theme::INSET,
                y: 0,
                w: 24,
                h: 24,
            },
        )?;
    }
    let targets = center.targets(layout);
    for (i, (target, label, bounds)) in targets.iter().enumerate() {
        let enabled = center.enabled(target);
        super::card(canvas, *bounds, i == center.selected)?;
        let color = if enabled {
            theme::TEXT
        } else {
            theme::DISABLED
        };
        if let Some((name, description, status)) = center.row_content(target) {
            if center.row_chosen(target) {
                fill(canvas, Rect { w: 3, ..*bounds }, theme::ACCENT)?;
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
                x: theme::INSET,
                y: y + i32::try_from(i).map_err(|_| "Too many lines")? * theme::LINE,
                w: width - 2 * theme::INSET,
                h: theme::LINE,
            },
            theme::TEXT,
        )?;
    }
    if let Some(message) = center.empty_message() {
        text(
            canvas,
            message,
            Rect {
                x: theme::INSET,
                y: 130,
                w: width - 2 * theme::INSET,
                h: 32,
            },
            1,
            theme::MUTED,
        )?;
    }
    if center.full_details() || center.editing() {
        text(
            canvas,
            center.footer(),
            Rect {
                x: 4,
                y: height - theme::LINE,
                w: width - 8,
                h: theme::LINE,
            },
            1,
            theme::WARNING,
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
        x: bounds.x + theme::INSET,
        y: bounds.y + theme::INSET,
        w: 32,
        h: 32,
    };
    if let Some(pixels) = icon {
        draw_icon(canvas, pixels, icon_bounds)?;
    } else {
        fill(canvas, icon_bounds, theme::BORDER)?;
        text(
            canvas,
            &name.chars().take(1).collect::<String>(),
            icon_bounds,
            2,
            theme::TEXT,
        )?;
    }
    let text_bounds = Rect {
        x: bounds.x + 50,
        y: bounds.y + 2,
        w: bounds.w - 58,
        h: 14,
    };
    left(canvas, name, text_bounds, theme::TEXT)?;
    left(
        canvas,
        description,
        Rect {
            y: bounds.y + 17,
            ..text_bounds
        },
        theme::MUTED,
    )?;
    left(
        canvas,
        status,
        Rect {
            y: bounds.y + 32,
            ..text_bounds
        },
        if status.starts_with("Update") {
            theme::VIOLET
        } else {
            theme::ACCENT
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
