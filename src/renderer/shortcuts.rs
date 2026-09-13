use super::{Screen, fill, text};
use crate::{
    layout::{Layout, Rect},
    shortcuts::screen::Desktop,
};
use sdl2::pixels::Color;

#[cfg(test)]
pub fn qa(canvas: &mut Screen, layout: &Layout, output: &std::path::Path) -> Result<(), String> {
    if layout.width < 480 || layout.height < 272 {
        return Ok(());
    }
    for (name, desktop) in Desktop::qa_samples(layout) {
        panel(canvas, layout, &desktop)?;
        super::screenshot(
            canvas,
            &output.join(format!(
                "shortcut-{name}-{}x{}.bmp",
                layout.width, layout.height
            )),
        )?;
    }
    Ok(())
}

pub fn panel(canvas: &mut Screen, layout: &Layout, desktop: &Desktop) -> Result<(), String> {
    canvas.set_draw_color(Color::RGB(14, 24, 34));
    canvas.clear();
    let width = i32::from(layout.width);
    text(
        canvas,
        desktop.title(),
        Rect {
            x: 40,
            y: 0,
            w: width - 80,
            h: 24,
        },
        1,
        Color::RGB(93, 218, 201),
    )?;
    let description = desktop.description();
    let columns = usize::from(layout.width.saturating_sub(16) / 8);
    let chars: Vec<_> = description.chars().collect();
    let start = if desktop.editing() {
        chars.len().saturating_sub(columns * 2)
    } else {
        0
    };
    for (i, line) in chars[start..].chunks(columns.max(1)).take(2).enumerate() {
        text(
            canvas,
            &line.iter().collect::<String>(),
            Rect {
                x: 8,
                y: 26 + i32::try_from(i).map_err(|_| "Description too long")? * 14,
                w: width - 16,
                h: 14,
            },
            1,
            if desktop.error.is_empty() {
                Color::RGB(190, 204, 218)
            } else {
                Color::RGB(255, 179, 151)
            },
        )?;
    }
    for (i, (_, label, bounds)) in desktop.targets(layout).iter().enumerate() {
        fill(
            canvas,
            *bounds,
            if i == desktop.selected {
                Color::RGB(42, 77, 92)
            } else {
                Color::RGB(33, 45, 58)
            },
        )?;
        if i == desktop.selected {
            canvas.set_draw_color(Color::RGB(120, 240, 220));
            canvas.draw_rect(super::rect(*bounds)?)?;
        }
        let limit = usize::try_from(bounds.w / 8).map_err(|_| "Button width")?;
        let label = if label.chars().count() > limit {
            format!(
                "{}...",
                label
                    .chars()
                    .take(limit.saturating_sub(3))
                    .collect::<String>()
            )
        } else {
            label.clone()
        };
        text(canvas, &label, *bounds, 1, Color::RGB(239, 241, 245))?;
    }
    let preview = Rect {
        x: width - 36,
        y: 0,
        w: 28,
        h: 28,
    };
    if let Some(surface) = desktop
        .preview()
        .and_then(|bytes| super::decode_icon(bytes).ok())
    {
        let creator = canvas.texture_creator();
        let texture = creator
            .create_texture_from_surface(surface)
            .map_err(|e| e.to_string())?;
        canvas.copy(&texture, None, super::rect(preview)?)?;
    } else {
        fill(canvas, preview, Color::RGB(57, 115, 137))?;
        text(canvas, "+", preview, 1, Color::RGB(219, 243, 240))?;
    }
    Ok(())
}
