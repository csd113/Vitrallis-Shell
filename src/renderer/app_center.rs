//! App Center presentation: a scannable list, one unmistakable state chip per
//! entry and a details page that leads with the information a user needs.
use super::{Screen, advance, fill, fit_columns, text, text_left};
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
            x: geometry.title.x.saturating_add(6_i32.saturating_mul(scale)),
            w: geometry
                .title
                .w
                .saturating_sub(12_i32.saturating_mul(scale)),
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
        let width = i32::try_from(count.chars().count())
            .unwrap_or(0_i32)
            .saturating_mul(advance(scale));
        text(
            canvas,
            &count,
            Rect {
                x: geometry
                    .title
                    .x
                    .saturating_add(geometry.title.w)
                    .saturating_sub(width)
                    .saturating_sub(6_i32.saturating_mul(scale)),
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
                y: geometry.title.h.saturating_add(scale),
                w: geometry.title.w,
                h: 2_i32.saturating_mul(scale),
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
                        w: 3_i32.saturating_mul(geometry.scale),
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
                        x: geometry.title.x,
                        y: geometry
                            .list_top
                            .saturating_add(8_i32.saturating_mul(geometry.scale)),
                        w: geometry
                            .width
                            .saturating_sub(16_i32.saturating_mul(geometry.scale)),
                        h: 24_i32.saturating_mul(geometry.scale),
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
    let width = fit_columns(
        geometry
            .title
            .w
            .saturating_sub(4_i32.saturating_mul(geometry.scale)),
        geometry.scale,
    );
    let lines = center.lines(width.max(1));
    let line = detail_line(geometry.scale);
    let bottom = if center.confirming() {
        geometry
            .pinned
            .y
            .saturating_sub(4_i32.saturating_mul(geometry.scale))
    } else if center.editing() {
        geometry
            .list_top
            .saturating_add(40_i32.saturating_mul(geometry.scale))
    } else {
        geometry.footer.y
    };
    let capacity = usize::try_from(
        bottom
            .saturating_sub(geometry.list_top)
            .max(0_i32)
            .checked_div(line.max(1_i32))
            .unwrap_or(0_i32),
    )
    .unwrap_or(1)
    .max(1);
    for (index, line_text) in lines
        .iter()
        .skip(center.detail_start())
        .take(capacity)
        .enumerate()
    {
        super::text_left(
            canvas,
            line_text,
            Rect {
                x: geometry
                    .title
                    .x
                    .saturating_add(2_i32.saturating_mul(geometry.scale)),
                y: geometry
                    .list_top
                    .saturating_add(i32::try_from(index).unwrap_or(0_i32).saturating_mul(line)),
                w: geometry
                    .title
                    .w
                    .saturating_sub(4_i32.saturating_mul(geometry.scale)),
                h: line,
            },
            geometry.scale,
            theme::TEXT,
        )?;
    }
    Ok(())
}

/// Height of one text body or details line, in pixels.
const fn detail_line(scale: i32) -> i32 {
    DETAIL_LINE.saturating_mul(scale)
}

/// Leading between two details lines, in pixels.
const fn detail_pitch(scale: i32) -> i32 {
    DETAIL_PITCH.saturating_mul(scale)
}

/// Single details/text body line height, in pixels per scale step.
pub(super) const DETAIL_LINE: i32 = 12;
/// Leading between details lines, in pixels per scale step.
pub(super) const DETAIL_PITCH: i32 = 11;

/// Details lead with name, state and description, then the metadata a user
/// actually needs. Backend error text stays out of the primary UI.
///
/// The field list is laid out inside the space between the search row and the
/// pinned action row: detail text can never overlap the buttons. The (repeatable)
/// description yields its lines to the (unique) fields when a screen is too
/// short for both, and anything still left over ends with an explicit ellipsis.
fn details(canvas: &mut Screen, geometry: &Geometry, center: &Center) -> Result<(), String> {
    let scale = geometry.scale;
    let line = detail_line(scale);
    let pitch = detail_pitch(scale);
    let mut y = geometry.list_top;
    let icon = 40_i32.saturating_mul(scale);
    let name = center.chosen_row_name().unwrap_or_else(|| "App".into());
    let icon_bounds = Rect {
        x: geometry.title.x,
        y,
        w: icon,
        h: icon,
    };
    if let Some(pixels) = center.detail_icon() {
        draw_icon(canvas, pixels, icon_bounds)?;
    } else {
        // The reserved icon column keeps the name and every field on one left
        // edge whether or not the catalogue ships an icon.
        super::fill(canvas, icon_bounds, theme::BORDER)?;
        super::text(
            canvas,
            &name.chars().take(1).collect::<String>(),
            icon_bounds,
            scale.saturating_add(1_i32),
            theme::TEXT,
        )?;
    }
    let name_bounds = Rect {
        x: geometry
            .title
            .x
            .saturating_add(icon)
            .saturating_add(8_i32.saturating_mul(scale)),
        y,
        w: geometry
            .title
            .w
            .saturating_sub(icon)
            .saturating_sub(8_i32.saturating_mul(scale)),
        h: icon,
    };
    text_left(
        canvas,
        &name,
        Rect {
            h: 20_i32.saturating_mul(scale),
            ..name_bounds
        },
        scale,
        theme::TEXT,
    )?;
    if let Some((state, label)) = center.chosen_state() {
        let (ink, surface) = state_colors(state);
        let width = super::chip_width(&label, scale);
        super::chip(
            canvas,
            Rect {
                x: name_bounds.x,
                y: name_bounds
                    .y
                    .saturating_add(icon)
                    .saturating_sub(13_i32.saturating_mul(scale)),
                w: width.min(name_bounds.w),
                h: 10_i32.saturating_mul(scale).saturating_add(2_i32),
            },
            &label,
            scale,
            ink,
            surface,
        )?;
    }
    y = y.saturating_add(icon.saturating_add(4_i32.saturating_mul(scale)));
    let description = wrap(
        &center.detail_description(),
        fit_columns(geometry.title.w, scale).max(1),
    );
    let fields = center.detail_fields();
    let plan = detail_plan(geometry, y, description.len(), fields.len());
    for text_line in description.iter().take(plan.description_lines) {
        text_left(
            canvas,
            text_line,
            Rect {
                x: geometry.title.x,
                y,
                w: geometry.title.w,
                h: line,
            },
            scale,
            theme::MUTED,
        )?;
        y = y.saturating_add(pitch);
    }
    y = y.saturating_add(2_i32.saturating_mul(scale));
    detail_fields_body(canvas, geometry, y, &plan, &fields)
}

/// One line per visible field, followed by the shared ellipsis when the list was
/// cut. The field body owns the region above the pinned action row.
fn detail_fields_body(
    canvas: &mut Screen,
    geometry: &Geometry,
    mut y: i32,
    plan: &DetailPlan,
    fields: &[(String, String)],
) -> Result<(), String> {
    let scale = geometry.scale;
    let line = detail_line(scale);
    let pitch = detail_pitch(scale);
    let bounds = Rect {
        x: geometry.title.x.saturating_add(2_i32.saturating_mul(scale)),
        y: 0,
        w: geometry.title.w.saturating_sub(4_i32.saturating_mul(scale)),
        h: line,
    };
    for index in drawn_fields(plan, fields) {
        let (label, value) = fields.get(index).ok_or("Missing App Center detail field")?;
        text_left(
            canvas,
            &format!("{label}: {value}"),
            Rect { y, ..bounds },
            scale,
            if label.starts_with("Last operation") {
                theme::WARNING
            } else {
                theme::TEXT
            },
        )?;
        y = y.saturating_add(pitch);
    }
    if plan.omitted {
        text_left(canvas, "...", Rect { y, ..bounds }, scale, theme::MUTED)?;
    }
    Ok(())
}

/// Which fields the details body draws, in order. When the list is cut, the
/// last operation failure keeps the final visible line: it is the only field
/// that reports an action the user can take, and it is ordered last.
pub(super) fn drawn_fields(plan: &DetailPlan, fields: &[(String, String)]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..fields.len()).take(plan.fields).collect();
    if plan.omitted
        && let Some(failure) = fields
            .iter()
            .position(|(label, _)| label.starts_with("Last operation"))
        && failure >= indices.len()
        && !indices.is_empty()
    {
        let _clipped_character = indices.pop();
        indices.push(failure);
    }
    indices
}

/// How many description lines and field lines the details body can hold above
/// the pinned action row. The (repeatable) description yields its lines to the
/// (unique) fields when a screen is too short for both; anything still left over
/// is reported as omitted so the caller can mark it.
pub(super) struct DetailPlan {
    pub description_lines: usize,
    pub fields: usize,
    pub omitted: bool,
}

pub(super) fn detail_plan(
    geometry: &Geometry,
    top: i32,
    description: usize,
    fields: usize,
) -> DetailPlan {
    let scale = geometry.scale;
    let pitch = detail_pitch(scale);
    let limit = geometry
        .pinned
        .y
        .saturating_sub(2_i32.saturating_mul(scale));
    let description_lines = (0..=description.min(2))
        .rev()
        .find(|lines| {
            let needed = top
                .saturating_add(i32::try_from(*lines).unwrap_or(0_i32).saturating_mul(pitch))
                .saturating_add(2_i32.saturating_mul(scale))
                .saturating_add(i32::try_from(fields).unwrap_or(0_i32).saturating_mul(pitch));
            needed <= limit
        })
        .unwrap_or(0);
    let first_field = top
        .saturating_add(
            i32::try_from(description_lines)
                .unwrap_or(0_i32)
                .saturating_mul(pitch),
        )
        .saturating_add(2_i32.saturating_mul(scale));
    let capacity = usize::try_from(
        limit
            .saturating_sub(first_field)
            .max(0_i32)
            .checked_div(pitch)
            .unwrap_or(0_i32),
    )
    .unwrap_or(0);
    let omitted = fields > capacity;
    DetailPlan {
        description_lines,
        fields: if omitted {
            capacity.saturating_sub(1)
        } else {
            capacity.min(fields)
        },
        omitted,
    }
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
    text_left(
        canvas,
        message,
        Rect {
            x: geometry
                .footer
                .x
                .saturating_add(2_i32.saturating_mul(scale)),
            y: geometry.footer.y,
            w: geometry
                .footer
                .w
                .saturating_sub(4_i32.saturating_mul(scale)),
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
    let name_scale = scale;
    let icon_bounds = Rect {
        x: bounds.x.saturating_add(4_i32.saturating_mul(scale)),
        y: bounds
            .y
            .saturating_add((bounds.h.saturating_sub(22_i32.saturating_mul(scale))) / 2_i32),
        w: 22_i32.saturating_mul(scale),
        h: 22_i32.saturating_mul(scale),
    };
    if let Some(pixels) = icon {
        draw_icon(canvas, pixels, icon_bounds)?;
    } else {
        fill(canvas, icon_bounds, theme::BORDER)?;
        text(
            canvas,
            &name.chars().take(1).collect::<String>(),
            icon_bounds,
            name_scale,
            theme::TEXT,
        )?;
    }
    let (state, label) = center
        .row_state_for(target)
        .unwrap_or((RowState::Available, String::new()));
    let chip_width = super::chip_width(&label, scale);
    let text_bounds = Rect {
        x: icon_bounds
            .x
            .saturating_add(icon_bounds.w)
            .saturating_add(6_i32.saturating_mul(scale)),
        y: bounds.y.saturating_add(3_i32.saturating_mul(scale)),
        // One cell of clearance keeps a shortened name from touching the state
        // chip that shares the row.
        w: bounds
            .w
            .saturating_sub(icon_bounds.w)
            .saturating_sub(14_i32.saturating_mul(scale))
            .saturating_sub(chip_width)
            .saturating_sub(advance(scale))
            .max(0_i32),
        // Names and descriptions use the same readable scale, on separate lines.
        h: theme::CELL.saturating_mul(name_scale),
    };
    text_left(canvas, name, text_bounds, name_scale, theme::TEXT)?;
    if !label.is_empty() {
        let (ink, surface) = state_colors(state);
        super::chip(
            canvas,
            Rect {
                x: bounds
                    .x
                    .saturating_add(bounds.w)
                    .saturating_sub(chip_width)
                    .saturating_sub(4_i32.saturating_mul(scale)),
                y: bounds.y.saturating_add(3_i32.saturating_mul(scale)),
                w: chip_width,
                h: 10_i32.saturating_mul(scale).saturating_add(2_i32),
            },
            &label,
            scale,
            ink,
            surface,
        )?;
    }
    text_left(
        canvas,
        description,
        Rect {
            x: text_bounds.x,
            y: bounds
                .y
                .saturating_add(bounds.h)
                .saturating_sub(detail_line(scale)),
            w: bounds
                .w
                .saturating_sub(icon_bounds.w)
                .saturating_sub(14_i32.saturating_mul(scale)),
            h: detail_pitch(scale),
        },
        scale,
        theme::MUTED,
    )
}

fn wrap(value: &str, width: usize) -> Vec<String> {
    super::wrap_words(value, width)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_text_is_visible_and_never_reaches_buttons() -> Result<(), String> {
        let _guard = crate::test_support::sdl_lock();
        assert!(
            sdl2::hint::set("SDL_VIDEODRIVER", "dummy")
                || sdl2::hint::get("SDL_VIDEODRIVER").as_deref() == Some("dummy"),
            "dummy video must be available for this fixture"
        );
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        for (width, height) in [(480, 272), (800, 480), (1280, 720)] {
            let layout = Layout::home(width, height)?;
            let geometry = Geometry::new(&layout);
            let window = video
                .window("confirmation text", u32::from(width), u32::from(height))
                .hidden()
                .build()
                .map_err(|e| e.to_string())?;
            let raw_canvas = window
                .into_canvas()
                .software()
                .build()
                .map_err(|e| e.to_string())?;
            let creator = raw_canvas.texture_creator();
            let mut canvas = Screen::new(raw_canvas, &creator)?;
            for (name, center) in Center::qa_samples()?
                .into_iter()
                .filter(|(_, center)| center.confirming())
            {
                canvas.set_draw_color(theme::BACKGROUND);
                canvas.clear();
                body(&mut canvas, &geometry, &center)?;
                let pixels = canvas.read_pixels(None, sdl2::pixels::PixelFormatEnum::RGB24)?;
                let background = [
                    theme::BACKGROUND.r,
                    theme::BACKGROUND.g,
                    theme::BACKGROUND.b,
                ];
                let mut ink = 0_i32;
                for (index, pixel) in pixels.as_chunks::<3>().0.iter().enumerate() {
                    if *pixel != background {
                        ink += 1_i32;
                        let y =
                            i32::try_from(index / usize::from(width)).map_err(|e| e.to_string())?;
                        assert!(
                            y >= geometry.list_top && y < geometry.pinned.y,
                            "{name}: text overlaps controls"
                        );
                    }
                }
                assert!(ink > 0_i32, "{name}: confirmation warning is blank");
            }
        }
        Ok(())
    }
}
