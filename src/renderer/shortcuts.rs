use super::{Screen, fill, text};
use crate::{
    layout::{Layout, Rect},
    shortcuts::screen::Desktop,
};
use vitrallis_native::theme;

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
    canvas.set_draw_color(theme::BACKGROUND);
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
        theme::ACCENT,
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
                theme::MUTED
            } else {
                theme::WARNING
            },
        )?;
    }
    for (i, (_, label, bounds)) in desktop.targets(layout).iter().enumerate() {
        super::card(canvas, *bounds, i == desktop.selected)?;
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
        text(canvas, &label, *bounds, 1, theme::TEXT)?;
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
        fill(canvas, preview, theme::BORDER)?;
        text(canvas, "+", preview, 1, theme::TEXT)?;
    }
    Ok(())
}
