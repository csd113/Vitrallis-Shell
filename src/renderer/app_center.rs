//! App Center presentation: a scannable list, one unmistakable state chip per
//! entry and a details page that leads with the information a user needs.
use super::{Screen, fill, text};
use crate::{
    app_center::{Center, Geometry, PageKind, RowState, Target},
    layout::{Layout, Rect},
};
use sdl2::pixels::Color;
use vitrallis_native::theme;

pub fn panel(canvas: &mut Screen, layout: &Layout, center: &Center) -> Result<(), String> {
    let geometry = Geometry::new(layout);
    fill(
        canvas,
        Rect {
            x: 0,
            y: 0,
            w: geometry.width,
            h: geometry.height,
        },
        theme::BACKGROUND,
    )?;
    header(canvas, &geometry, center)?;
    targets(canvas, &geometry, center)?;
    body(canvas, &geometry, center)?;
    footer(canvas, &geometry, center)
}

/// Title, busy progress and the app count, at the top of the screen.
fn header(canvas: &mut Screen, geometry: &Geometry, center: &Center) -> Result<(), String> {
    let scale = geometry.scale;
    text(
        canvas,
        center.title(),
        Rect {
            x: geometry.title.x + 6 * scale,
            w: geometry.title.w - 12 * scale,
            h: geometry.title.h,
            ..geometry.title
        },
        scale,
        theme::ACCENT,
    )?;
    let count = match center.targets_count() {
        0 => String::new(),
        count => format!("{count} apps"),
    };
    if !count.is_empty() {
        let width = i32::try_from(count.chars().count()).unwrap_or(0) * 8 * scale;
        text(
            canvas,
            &count,
            Rect {
                x: geometry.title.x + geometry.title.w - width - 6 * scale,
                w: width,
                h: geometry.title.h,
                ..geometry.title
            },
            scale,
            theme::MUTED,
        )?;
    }
    fill(
        canvas,
        Rect {
            x: geometry.title.x,
            y: geometry.title.h,
            w: geometry.title.w,
            h: scale,
        },
        theme::BORDER,
    )?;
    if center.busy {
        // A real, continuous progress strip: the same ramp as every other bar.
        super::progress(
            canvas,
            Rect {
                x: geometry.title.x,
                y: geometry.title.h + scale,
                w: geometry.title.w,
                h: 2 * scale,
            },
            geometry.title.w,
            false,
        )?;
    }
    Ok(())
}

/// Action bar, search/filter row, list rows and the pinned bottom row.
fn targets(canvas: &mut Screen, geometry: &Geometry, center: &Center) -> Result<(), String> {
    for (index, (target, label, bounds)) in center.targets_of(geometry).into_iter().enumerate() {
        let enabled = center.enabled(&target);
        super::card(canvas, bounds, index == center.selected)?;
        let color = if enabled {
            theme::TEXT
        } else {
            theme::DISABLED
        };
        if let Some((name, description, _)) = center.row_content(&target) {
            if center.row_chosen(&target) {
                fill(
                    canvas,
                    Rect {
                        w: 3 * geometry.scale,
                        ..bounds
                    },
                    theme::ACCENT,
                )?;
            }
            app_row(
                canvas,
                geometry,
                center,
                &target,
                center.row_icon(&target),
                (name, description),
                bounds,
            )?;
        } else {
            text(canvas, &label, bounds, geometry.scale, color)?;
        }
    }
    Ok(())
}

fn body(canvas: &mut Screen, geometry: &Geometry, center: &Center) -> Result<(), String> {
    match center.page_kind() {
        PageKind::Details => details(canvas, geometry, center)?,
        PageKind::Text => text_body(canvas, geometry, center)?,
        PageKind::List => {
            if let Some(message) = center.empty_message() {
                text(
                    canvas,
                    message,
                    Rect {
                        x: geometry.list_top,
                        y: geometry.list_top + 8 * geometry.scale,
                        w: geometry.width - 16 * geometry.scale,
                        h: 24 * geometry.scale,
                    },
                    geometry.scale,
                    theme::MUTED,
                )?;
            }
        }
    }
    Ok(())
}

/// Read-only catalogue pages still use the wrapped text body.
pub(super) fn text_body(
    canvas: &mut Screen,
    geometry: &Geometry,
    center: &Center,
) -> Result<(), String> {
    let width = usize::try_from(geometry.width / (8 * geometry.scale)).unwrap_or(1);
    let lines = center.lines(width.max(1));
    let capacity =
        usize::try_from((geometry.footer.y - geometry.list_top) / (12 * geometry.scale).max(1))
            .unwrap_or(1);
    for (index, line) in lines
        .iter()
        .skip(center.detail_start())
        .take(capacity)
        .enumerate()
    {
        left(
            canvas,
            line,
            Rect {
                x: geometry.title.x + 2 * geometry.scale,
                y: geometry.list_top + i32::try_from(index).unwrap_or(0) * 12 * geometry.scale,
                w: geometry.title.w - 4 * geometry.scale,
                h: 12 * geometry.scale,
            },
            geometry.scale,
            theme::TEXT,
        )?;
    }
    Ok(())
}

/// Details lead with name, state and description, then the metadata a user
/// actually needs. Backend error text stays out of the primary UI.
fn details(canvas: &mut Screen, geometry: &Geometry, center: &Center) -> Result<(), String> {
    let scale = geometry.scale;
    let mut y = geometry.list_top;
    let icon = 40 * scale;
    let name = center.chosen_row_name().unwrap_or_else(|| "App".into());
    if let Some(pixels) = center.detail_icon() {
        draw_icon(
            canvas,
            pixels,
            Rect {
                x: geometry.title.x,
                y,
                w: icon,
                h: icon,
            },
        )?;
    }
    let name_bounds = Rect {
        x: geometry.title.x + icon + 8 * scale,
        y,
        w: geometry.title.w - icon - 8 * scale,
        h: icon,
    };
    left(canvas, &name, name_bounds, scale + 1, theme::TEXT)?;
    if let Some((state, label)) = center.chosen_state() {
        let (ink, surface) = state_colors(state);
        let width = super::chip_width(&label, scale);
        super::chip(
            canvas,
            Rect {
                x: name_bounds.x,
                y: name_bounds.y + icon - 13 * scale,
                w: width.min(name_bounds.w),
                h: 10 * scale + 2,
            },
            &label,
            scale,
            ink,
            surface,
        )?;
    }
    y += icon + 4 * scale;
    let description = center.detail_description();
    for line in wrap(
        &description,
        usize::try_from(geometry.title.w / (8 * scale)).unwrap_or(1),
    )
    .into_iter()
    .take(2)
    {
        left(
            canvas,
            &line,
            Rect {
                x: geometry.title.x,
                y,
                w: geometry.title.w,
                h: 12 * scale,
            },
            scale,
            theme::MUTED,
        )?;
        y += 11 * scale;
    }
    y += 2 * scale;
    for (label, value) in center.detail_fields() {
        let line = format!("{label}: {value}");
        left(
            canvas,
            &line,
            Rect {
                x: geometry.title.x + 2 * scale,
                y,
                w: geometry.title.w - 4 * scale,
                h: 12 * scale,
            },
            scale,
            if label.starts_with("Last operation") {
                theme::WARNING
            } else {
                theme::TEXT
            },
        )?;
        y += 11 * scale;
    }
    Ok(())
}

const fn state_colors(state: RowState) -> (Color, Color) {
    match state {
        RowState::Running => (theme::BACKGROUND, theme::ACCENT),
        RowState::Updating | RowState::Update => (theme::BACKGROUND, theme::VIOLET),
        RowState::Failed => (theme::BACKGROUND, theme::WARNING),
        RowState::Installed => (theme::TEXT, theme::PANEL),
        RowState::Available => (theme::MUTED, theme::PANEL),
        RowState::Unavailable => (theme::DISABLED, theme::PANEL),
    }
}

fn footer(canvas: &mut Screen, geometry: &Geometry, center: &Center) -> Result<(), String> {
    let scale = geometry.scale;
    let message = center.status_line();
    if message.is_empty() {
        return Ok(());
    }
    left(
        canvas,
        message,
        Rect {
            x: geometry.footer.x + 2 * scale,
            y: geometry.footer.y,
            w: geometry.footer.w - 4 * scale,
            h: geometry.footer.h,
        },
        scale,
        if center.busy {
            theme::ACCENT
        } else if center.has_visible_error() {
            theme::WARNING
        } else {
            theme::MUTED
        },
    )
}

/// One list entry: icon, name, state chip and description.
fn app_row(
    canvas: &mut Screen,
    geometry: &Geometry,
    center: &Center,
    target: &Target,
    icon: Option<&[u8]>,
    (name, description): (&str, &str),
    bounds: Rect,
) -> Result<(), String> {
    let scale = geometry.scale;
    let icon_bounds = Rect {
        x: bounds.x + 4 * scale,
        y: bounds.y + (bounds.h - 28 * scale) / 2,
        w: 28 * scale,
        h: 28 * scale,
    };
    if let Some(pixels) = icon {
        draw_icon(canvas, pixels, icon_bounds)?;
    } else {
        fill(canvas, icon_bounds, theme::BORDER)?;
        text(
            canvas,
            &name.chars().take(1).collect::<String>(),
            icon_bounds,
            scale + 1,
            theme::TEXT,
        )?;
    }
    let (state, label) = center
        .row_state_for(target)
        .unwrap_or((RowState::Available, String::new()));
    let chip_width = i32::try_from(label.chars().count()).unwrap_or(0) * 8 * scale + 6 * scale;
    let text_bounds = Rect {
        x: icon_bounds.x + icon_bounds.w + 6 * scale,
        y: bounds.y + 3 * scale,
        w: bounds.w - icon_bounds.w - 14 * scale - chip_width,
        h: 10 * scale,
    };
    left(canvas, name, text_bounds, scale + 1, theme::TEXT)?;
    if !label.is_empty() {
        let (ink, surface) = state_colors(state);
        super::chip(
            canvas,
            Rect {
                x: bounds.x + bounds.w - chip_width - 4 * scale,
                y: bounds.y + 3 * scale,
                w: chip_width,
                h: 10 * scale + 2,
            },
            &label,
            scale,
            ink,
            surface,
        )?;
    }
    left(
        canvas,
        description,
        Rect {
            x: text_bounds.x,
            y: bounds.y + bounds.h - 12 * scale,
            w: bounds.w - icon_bounds.w - 14 * scale,
            h: 11 * scale,
        },
        scale,
        theme::MUTED,
    )
}

/// Fit text explicitly so titles and long errors end with a visible ellipsis.
fn left(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    let cell = 8 * scale;
    let limit = usize::try_from(bounds.w / cell).unwrap_or(0);
    let value = value
        .replace(['—', '–'], "-")
        .replace(['’', '‘'], "'")
        .replace(['“', '”'], "\"")
        .replace('•', "-")
        .replace(['\n', '\r', '\t'], " ");
    if limit == 0 {
        return Ok(());
    }
    let shown = if value.chars().count() > limit && limit > 3 {
        format!("{}...", value.chars().take(limit - 3).collect::<String>())
    } else {
        value.chars().take(limit).collect()
    };
    let w = i32::try_from(shown.chars().count()).unwrap_or(0) * cell;
    text(
        canvas,
        &shown,
        Rect {
            w,
            h: bounds.h,
            ..bounds
        },
        scale,
        color,
    )
}

fn wrap(value: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        for character in word.chars() {
            if line.chars().count() == width {
                lines.push(std::mem::take(&mut line));
            }
            line.push(character);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
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
