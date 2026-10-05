//! Desktop actions, shortcut editor and picker surfaces. Every rectangle and
//! every glyph uses `layout.text_scale`, so this screen keeps the same text
//! density as the rest of the shell at 480x272, 800x480 and 1280x720.
use super::{Screen, fill, fit_columns, text, text_left};
use crate::{
    layout::{Layout, Rect},
    shortcuts::screen::Desktop,
};
use std::ops::Range;
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

/// Byte range of `count` characters starting at character `start`. Slicing a
/// shared string by character keeps the description window allocation-free.
fn char_span(value: &str, start: usize, count: usize) -> Range<usize> {
    let begin = value
        .char_indices()
        .nth(start)
        .map_or(value.len(), |(index, _)| index);
    let end = value
        .char_indices()
        .nth(start.saturating_add(count))
        .map_or(value.len(), |(index, _)| index);
    begin..end
}

pub fn panel(canvas: &mut Screen, layout: &Layout, desktop: &Desktop) -> Result<(), String> {
    canvas.set_draw_color(theme::BACKGROUND);
    canvas.clear();
    let scale = layout.text_scale.max(1_i32);
    let width = i32::from(layout.width);
    text(
        canvas,
        desktop.title(),
        Rect {
            x: 40_i32.saturating_mul(scale),
            y: 0,
            w: width.saturating_sub(80_i32.saturating_mul(scale)),
            h: 24_i32.saturating_mul(scale),
        },
        scale,
        theme::ACCENT,
    )?;
    let description = desktop.description();
    let columns = fit_columns(width.saturating_sub(16_i32.saturating_mul(scale)), scale).max(1);
    let count = description.chars().count();
    let start = if desktop.editing() {
        count.saturating_sub(columns.saturating_mul(2_usize))
    } else {
        0
    };
    for line in 0_usize..2_usize {
        let span = char_span(
            &description,
            start.saturating_add(line.saturating_mul(columns)),
            columns,
        );
        if span.is_empty() {
            break;
        }
        text_left(
            canvas,
            description
                .get(span)
                .ok_or("Invalid shortcut description character span")?,
            Rect {
                x: 8_i32.saturating_mul(scale),
                y: 26_i32.saturating_mul(scale).saturating_add(
                    i32::try_from(line)
                        .unwrap_or(0_i32)
                        .saturating_mul(14_i32)
                        .saturating_mul(scale),
                ),
                w: width.saturating_sub(16_i32.saturating_mul(scale)),
                h: 14_i32.saturating_mul(scale),
            },
            scale,
            if desktop.error.is_empty() {
                theme::MUTED
            } else {
                theme::WARNING
            },
        )?;
    }
    for (i, (_, label, bounds)) in desktop.targets(layout).iter().enumerate() {
        super::card(canvas, *bounds, i == desktop.selected)?;
        text(canvas, label, *bounds, scale, theme::TEXT)?;
    }
    let preview = Rect {
        x: width.saturating_sub(36_i32.saturating_mul(scale)),
        y: 0,
        w: 28_i32.saturating_mul(scale),
        h: 28_i32.saturating_mul(scale),
    };
    // The preview texture is cached inside Screen; only a changed icon is
    // decoded and uploaded again. Undecodable bytes keep the placeholder.
    let drawn = match desktop.preview() {
        Some(bytes) => canvas.icon_preview(bytes, preview)?,
        None => false,
    };
    if !drawn {
        fill(canvas, preview, theme::BORDER)?;
        text(canvas, "+", preview, scale, theme::TEXT)?;
    }
    Ok(())
}
