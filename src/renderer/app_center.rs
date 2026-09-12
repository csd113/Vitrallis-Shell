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
        Color::RGB(19, 28, 39),
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
    let targets = center.targets(layout);
    for (i, (target, label, bounds)) in targets.iter().enumerate() {
        let enabled = center.enabled(target);
        fill(
            canvas,
            *bounds,
            if i == center.selected {
                Color::RGB(49, 100, 117)
            } else {
                Color::RGB(33, 45, 58)
            },
        )?;
        if i == center.selected {
            canvas.set_draw_color(Color::RGB(120, 240, 220));
            canvas.draw_rect(super::rect(*bounds)?)?;
        }
        text(
            canvas,
            label,
            *bounds,
            1,
            if enabled {
                Color::RGB(239, 241, 245)
            } else {
                Color::RGB(139, 149, 159)
            },
        )?;
    }
    let lines = center.lines(usize::from(layout.width) / 8 - 2);
    let (y, count) = if center.full_details() {
        (68, usize::from(layout.height.saturating_sub(124) / 12))
    } else if center.editing() {
        (68, 3)
    } else {
        (height - 82, 2)
    };
    for (i, line) in lines
        .iter()
        .skip(center.detail_start())
        .take(count)
        .enumerate()
    {
        text(
            canvas,
            line,
            Rect {
                x: 8,
                y: y + i32::try_from(i).map_err(|_| "Too many lines")? * 12,
                w: width - 16,
                h: 12,
            },
            1,
            Color::RGB(220, 224, 231),
        )?;
    }
    {
        text(
            canvas,
            &center.message,
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
