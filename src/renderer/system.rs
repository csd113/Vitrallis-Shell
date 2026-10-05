//! Small native icons and the shared System Settings surface; no runtime assets.
use vitrallis_native::theme;
#[path = "system_storage.rs"]
mod storage;
#[path = "system_tor.rs"]
mod tor;
#[path = "system_wireless.rs"]
mod wireless;
use super::{Screen, advance, card, fill, progress, rect, text, text_left};
use crate::{
    layout::{Layout, Rect},
    platform::system::{Power, Status, Wifi},
    settings::{Page, PanelLayout, Settings},
};
use sdl2::{pixels::Color, render::Texture};

pub(super) const ASSETS: [&[u8]; 3] = [
    include_bytes!("../../assets/system/wifi.png"),
    include_bytes!("../../assets/system/sun.png"),
    include_bytes!("../../assets/system/speaker.png"),
];

const INK: Color = theme::TEXT;
const TEXT: Color = theme::TEXT;
const MUTED: Color = theme::MUTED;
const ACCENT: Color = theme::ACCENT;
const AMBER: Color = theme::WARNING;

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Sun,
    Speaker,
    Wifi,
    Bolt,
}
#[allow(
    clippy::arithmetic_side_effects,
    reason = "Fallback icon dimensions are checked as 1..=4096 before multiplication and division; normalized coordinates stay in -16..=15 and translated positions saturate"
)]
fn icon(
    canvas: &mut Screen,
    r: Rect,
    kind: Icon,
    color: Color,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    let index = match kind {
        Icon::Wifi => Some(0),
        Icon::Sun => Some(1),
        Icon::Speaker => Some(2),
        Icon::Bolt => None,
    };
    if let Some(texture) = index
        .and_then(|slot| textures.get(slot))
        .and_then(Option::as_ref)
    {
        return canvas.copy(texture, None, rect(r)?);
    }
    if r.w <= 0_i32 || r.h <= 0_i32 {
        return Ok(());
    }
    if r.w > 4096_i32 || r.h > 4096_i32 {
        return Err("Fallback icon dimensions exceed screen limits".into());
    }
    canvas.set_draw_color(color);
    for y in 0_i32..r.h {
        for x in 0_i32..r.w {
            let dx = x * 32_i32 / r.w - 16_i32;
            let dy = y * 32_i32 / r.h - 16_i32;
            if pixel(kind, dx, dy) {
                canvas.draw_point((r.x.saturating_add(x), r.y.saturating_add(y)))?;
            }
        }
    }
    Ok(())
}
#[allow(
    clippy::arithmetic_side_effects,
    reason = "The input guard bounds x and y to -16_i32..=16_i32; squared distances, absolute values and fixed offsets stay below 1024"
)]
fn pixel(kind: Icon, x: i32, y: i32) -> bool {
    if !(-16_i32..=16_i32).contains(&x) || !(-16_i32..=16_i32).contains(&y) {
        return false;
    }
    let radius = x * x + y * y;
    match kind {
        Icon::Sun => {
            (25..=64).contains(&radius)
                || ((121..=196).contains(&radius)
                    && (x.abs() <= 1 || y.abs() <= 1 || (x.abs() - y.abs()).abs() <= 1))
        }
        Icon::Speaker => {
            ((-13..=-7).contains(&x) && y.abs() <= 5)
                || ((-7..=0).contains(&x) && y.abs() <= x + 11)
                || (x >= 4
                    && ((49..=81).contains(&radius) || (144..=196).contains(&radius))
                    && y.abs() <= x + 2)
        }
        Icon::Wifi => {
            let dy = y - 11;
            let distance = x * x + dy * dy;
            (dy <= -3
                && x.abs() <= -dy * 2
                && ((64..=100).contains(&distance)
                    || (225..=289).contains(&distance)
                    || (441..=529).contains(&distance)))
                || (x * x + (y - 9) * (y - 9) <= 7)
        }
        Icon::Bolt => polygon(x, y),
    }
}
#[allow(
    clippy::arithmetic_side_effects,
    reason = "The guard bounds the sample to -16_i32..=16_i32 and the six fixed bolt vertices share that range; cross products stay below 2048 and winding within -6..=6"
)]
fn polygon(x: i32, y: i32) -> bool {
    const POINTS: [(i32, i32); 6] = [(6, -14), (-9, 3), (-1, 3), (-5, 14), (10, -5), (2, -5)];
    if !(-16_i32..=16_i32).contains(&x) || !(-16_i32..=16_i32).contains(&y) {
        return false;
    }
    let mut winding = 0_i32;
    for (&(ax, ay), &(bx, by)) in POINTS.iter().zip(POINTS.iter().cycle().skip(1)) {
        let cross = (bx - ax) * (y - ay) - (x - ax) * (by - ay);
        if ay <= y && by > y && cross > 0_i32 {
            winding += 1_i32;
        }
        if ay > y && by <= y && cross < 0_i32 {
            winding -= 1_i32;
        }
    }
    winding != 0
}

/// Left-aligned single-line label. Measurement, padding and the overflow
/// policy come from the shared renderer helpers so every Settings surface
/// shortens long values the same way.
fn label(
    canvas: &mut Screen,
    value: &str,
    bounds: Rect,
    scale: i32,
    color: Color,
) -> Result<(), String> {
    text_left(canvas, value, bounds, scale, color)
}
#[allow(
    clippy::arithmetic_side_effects,
    reason = "Radius is checked as 0..=128, so squared sums stay at most 32768 and negation is safe; translated coordinates use saturation"
)]
fn circle(canvas: &mut Screen, x: i32, y: i32, radius: i32, color: Color) -> Result<(), String> {
    canvas.set_draw_color(color);
    if !(0_i32..=128_i32).contains(&radius) {
        return Err("Circle radius exceeds screen limits".into());
    }
    for dy in -radius..=radius {
        let dx = (0_i32..=radius)
            .take_while(|dx| dx * dx + dy * dy <= radius * radius)
            .last()
            .unwrap_or(0_i32);
        canvas.draw_line(
            (x.saturating_sub(dx), y.saturating_add(dy)),
            (x.saturating_add(dx), y.saturating_add(dy)),
        )?;
    }
    Ok(())
}

pub(super) fn status(
    canvas: &mut Screen,
    layout: &Layout,
    status: &Status,
    preferences: &crate::preferences::Preferences,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    let scale = layout.text_scale;
    let clock = preferences.clock(status.clock.as_deref());
    let clock_width = i32::try_from(clock.len())
        .map_err(|error| format!("clock length: {error}"))?
        .saturating_mul(advance(scale));
    // Fixed slots from the right edge: battery, charge marker, radio mark, clock.
    let width = 128_i32.saturating_mul(scale).saturating_add(clock_width);
    let x = i32::from(layout.width)
        .saturating_sub(layout.title.x)
        .saturating_sub(width);
    label(
        canvas,
        &status
            .ip
            .map_or_else(|| "IP --".into(), |ip| format!("IP {ip}")),
        Rect {
            x: layout.title.x,
            y: layout.title.h / 2,
            w: x.saturating_sub(layout.title.x)
                .saturating_sub(8_i32.saturating_mul(scale)),
            h: layout.title.h / 2,
        },
        scale,
        MUTED,
    )?;
    let y = layout.title.h.saturating_mul(3_i32) / 4_i32;
    battery(canvas, status, x, y, scale)?;
    if status.charging == Some(true) || status.external_power == Some(true) {
        icon(
            canvas,
            Rect {
                x: x.saturating_add(67_i32.saturating_mul(scale)),
                y: y.saturating_sub(8_i32.saturating_mul(scale)),
                w: 16_i32.saturating_mul(scale),
                h: 16_i32.saturating_mul(scale),
            },
            Icon::Bolt,
            if status.charging == Some(true) {
                ACCENT
            } else {
                MUTED
            },
            textures,
        )?;
    }
    let wifi = Rect {
        x: x.saturating_add(94_i32.saturating_mul(scale)),
        y: y.saturating_sub(8_i32.saturating_mul(scale)),
        w: 18_i32.saturating_mul(scale),
        h: 16_i32.saturating_mul(scale),
    };
    icon(
        canvas,
        wifi,
        Icon::Wifi,
        match status.wifi {
            Some(Wifi::Connected) => ACCENT,
            Some(Wifi::Connecting) => AMBER,
            _ => MUTED,
        },
        textures,
    )?;
    if matches!(status.wifi, None | Some(Wifi::Off | Wifi::Disconnected)) {
        canvas.set_draw_color(MUTED);
        canvas.draw_line(
            (wifi.x, wifi.y.saturating_add(wifi.h)),
            (wifi.x.saturating_add(wifi.w), wifi.y),
        )?;
    }
    label(
        canvas,
        &clock,
        Rect {
            x: x.saturating_add(128_i32.saturating_mul(scale)),
            y: y.saturating_sub(6_i32.saturating_mul(scale)),
            w: clock_width,
            h: 12_i32.saturating_mul(scale),
        },
        scale,
        INK,
    )
}

fn battery(canvas: &mut Screen, status: &Status, x: i32, y: i32, scale: i32) -> Result<(), String> {
    let battery = Rect {
        x,
        y: y.saturating_sub(6_i32.saturating_mul(scale)),
        w: 25_i32.saturating_mul(scale),
        h: 12_i32.saturating_mul(scale),
    };
    let color = if status.battery.is_some_and(|p| p.value() <= 15) {
        AMBER
    } else {
        ACCENT
    };
    canvas.set_draw_color(if status.battery.is_some() {
        color
    } else {
        MUTED
    });
    canvas.draw_rect(rect(battery)?)?;
    fill(
        canvas,
        Rect {
            x: x.saturating_add(battery.w),
            y: y.saturating_sub(2_i32.saturating_mul(scale)),
            w: 2_i32.saturating_mul(scale),
            h: 4_i32.saturating_mul(scale),
        },
        MUTED,
    )?;
    if let Some(value) = status.battery {
        let w = battery
            .w
            .saturating_sub(4_i32.saturating_mul(scale))
            .saturating_mul(i32::from(value.value()))
            / 100_i32;
        if w > 0_i32 {
            fill(
                canvas,
                Rect {
                    x: x.saturating_add(2_i32.saturating_mul(scale)),
                    y: battery.y.saturating_add(2_i32.saturating_mul(scale)),
                    w,
                    h: battery.h.saturating_sub(4_i32.saturating_mul(scale)),
                },
                color,
            )?;
        }
    }
    let percent = status
        .battery
        .map_or_else(|| "--".into(), |p| format!("{}%", p.value()));
    label(
        canvas,
        &percent,
        Rect {
            x: x.saturating_add(34_i32.saturating_mul(scale)),
            y: y.saturating_sub(6_i32.saturating_mul(scale)),
            w: 32_i32.saturating_mul(scale),
            h: 12_i32.saturating_mul(scale),
        },
        scale,
        INK,
    )?;
    Ok(())
}

pub(super) fn panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    if settings.confirmation.is_some() {
        return confirmation(canvas, layout, settings, &PanelLayout::new(layout));
    }
    match settings.page {
        Page::Tor | Page::TorDetails => return tor::panel(canvas, layout, settings),
        Page::Applications => return preferences_panel(canvas, layout, settings),
        Page::Wireless => return wireless::panel(canvas, layout, settings),
        Page::Storage => return storage::panel(canvas, layout, settings),
        Page::Updates => return update_panel(canvas, layout, settings),
        Page::Display => return display_panel(canvas, layout, settings, textures),
        Page::DateTime => return datetime_panel(canvas, layout, settings),
        Page::Device => return device_panel(canvas, layout, settings, textures),
        Page::About => return about_panel(canvas, layout, settings),
        Page::Timezones => return zones_panel(canvas, layout, settings),
        Page::Home => (),
    }
    home_panel(canvas, layout, settings)
}

/// The overview uses simple category labels and one summary for the focused item.
fn home_panel(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    let rows = settings.home_rows();
    for (index, (bounds, (title, _))) in
        PanelLayout::home(layout).into_iter().zip(&rows).enumerate()
    {
        card(canvas, bounds, settings.selected == index)?;
        label(
            canvas,
            title,
            Rect {
                x: bounds.x.saturating_add(8_i32),
                w: bounds.w.saturating_sub(16_i32),
                ..bounds
            },
            layout.text_scale,
            TEXT,
        )?;
    }
    let summary = if settings.message.is_empty() {
        rows.get(settings.selected)
            .map_or("", |(_, detail)| detail.as_str())
    } else {
        &settings.message
    };
    text(
        canvas,
        summary,
        Rect {
            y: layout
                .footer
                .y
                .saturating_sub(12_i32.saturating_mul(layout.text_scale)),
            h: 12_i32.saturating_mul(layout.text_scale),
            ..layout.footer
        },
        layout.text_scale,
        MUTED,
    )?;
    for (position, (bounds, control)) in PanelLayout::footer(layout)
        .into_iter()
        .zip(settings.footer_controls())
        .enumerate()
    {
        let Some((index, title)) = control else {
            continue;
        };
        let inset_bounds = Rect {
            x: bounds.x.saturating_add(2_i32),
            w: bounds.w.saturating_sub(4_i32),
            ..bounds
        };
        card(canvas, inset_bounds, settings.selected == index)?;
        text(
            canvas,
            title,
            inset_bounds,
            layout.text_scale,
            if position == 0 {
                ACCENT
            } else if settings.status.power_controls && !settings.pending {
                AMBER
            } else {
                theme::DISABLED
            },
        )?;
    }
    Ok(())
}

/// Shared two-line option row used by every Settings category.
fn option_row(
    canvas: &mut Screen,
    layout: &Layout,
    bounds: Rect,
    title: &str,
    detail: &str,
    selected: bool,
    detail_color: Color,
) -> Result<(), String> {
    card(canvas, bounds, selected)?;
    let content = Rect {
        x: bounds.x.saturating_add(12_i32),
        w: bounds.w.saturating_sub(24_i32),
        ..bounds
    };
    label(
        canvas,
        title,
        Rect {
            h: bounds.h / 2,
            ..content
        },
        layout.text_scale,
        if selected { INK } else { TEXT },
    )?;
    label(
        canvas,
        detail,
        Rect {
            y: bounds.y.saturating_add((bounds.h) / 2_i32),
            h: bounds.h / 2,
            ..content
        },
        layout.text_scale,
        detail_color,
    )
}

/// Display & Sound: the two hardware sliders.
fn display_panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    if settings.confirmation.is_some() {
        return confirmation(canvas, layout, settings, &PanelLayout::new(layout));
    }
    let geometry = PanelLayout::new(layout);
    for index in 0..2 {
        slider(canvas, layout, settings, &geometry, index, textures)?;
    }
    panel_footer(canvas, layout, settings)
}

/// Date & Time: clock format and the time-zone listing.
fn datetime_panel(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    let rows = PanelLayout::rows(layout, 2);
    let zone = settings
        .status
        .timezone
        .clone()
        .unwrap_or_else(|| "Unavailable".into());
    for (index, bounds) in rows.into_iter().enumerate() {
        let (title, detail, color) = if index == 0 {
            (
                "Clock format",
                if settings.policy.ampm {
                    "< 12 hour >".into()
                } else {
                    "< 24 hour >".into()
                },
                ACCENT,
            )
        } else {
            (
                "Time zone",
                zone.clone(),
                if zone.starts_with("Unavailable") {
                    MUTED
                } else {
                    ACCENT
                },
            )
        };
        option_row(
            canvas,
            layout,
            bounds,
            title,
            &detail,
            settings.selected == index,
            color,
        )?;
    }
    panel_footer(canvas, layout, settings)
}

/// Applications: background lifetime policy and the keep-running list.
fn preferences_panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
) -> Result<(), String> {
    for (index, (bounds, (title, detail))) in PanelLayout::rows(
        layout,
        i32::try_from(crate::settings::preferences::APP_ROWS).unwrap_or(3_i32),
    )
    .into_iter()
    .zip(settings.preference_rows())
    .enumerate()
    {
        option_row(
            canvas,
            layout,
            bounds,
            &title,
            &detail,
            settings.selected == index,
            if settings.selected == index {
                ACCENT
            } else {
                MUTED
            },
        )?;
    }
    panel_footer(canvas, layout, settings)
}

/// About: read-only system information, no editable controls.
fn about_panel(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    for ((title, value), bounds) in about_rows(settings)
        .into_iter()
        .zip(PanelLayout::rows(layout, 5))
    {
        card(canvas, bounds, false)?;
        let content = Rect {
            x: bounds.x.saturating_add(12_i32),
            w: bounds.w.saturating_sub(24_i32),
            ..bounds
        };
        label(
            canvas,
            &title,
            Rect {
                h: bounds.h / 2,
                ..content
            },
            layout.text_scale,
            MUTED,
        )?;
        label(
            canvas,
            &value,
            Rect {
                y: bounds.y.saturating_add((bounds.h) / 2_i32),
                h: bounds.h / 2,
                ..content
            },
            layout.text_scale,
            INK,
        )?;
    }
    panel_footer(canvas, layout, settings)
}

/// About values reuse the refreshed device status; nothing is fabricated.
fn about_rows(settings: &Settings) -> Vec<(String, String)> {
    let status = &settings.status;
    vec![
        ("Vitrallis Shell".into(), display_version().to_owned()),
        (
            "Display".into(),
            if settings.renderer.is_empty() {
                "Unavailable".into()
            } else {
                settings.renderer.clone()
            },
        ),
        (
            "Network".into(),
            status.ip.map_or_else(
                || format!("Wi-Fi {}", wifi_label(status.wifi).to_lowercase()),
                |ip| format!("{ip}  Wi-Fi {}", wifi_label(status.wifi).to_lowercase()),
            ),
        ),
        (
            "Battery".into(),
            status.battery.map_or_else(
                || "Unavailable".into(),
                |percent| {
                    format!(
                        "{}{}",
                        percent.value(),
                        if status.charging == Some(true) {
                            "% charging"
                        } else {
                            "%"
                        }
                    )
                },
            ),
        ),
        (
            "Date & time".into(),
            format!(
                "{}  {}",
                settings
                    .status
                    .timezone
                    .clone()
                    .unwrap_or_else(|| "Time zone unavailable".into()),
                settings
                    .status
                    .clock
                    .clone()
                    .unwrap_or_else(|| "--:--".into())
            ),
        ),
    ]
}

/// Time zones remain a bounded, paged selector under Date & Time.
fn zones_panel(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    let scale = layout.text_scale;
    // The "current" marker owns the right end of the row, so a long zone name
    // is shortened before it instead of growing into the marker, and the marker
    // itself always has room for its whole word.
    let marker = 60_i32.saturating_mul(scale);
    let inset = 12_i32.saturating_mul(scale);
    let gap = 8_i32.saturating_mul(scale);
    for (index, bounds) in PanelLayout::rows(layout, 5).into_iter().enumerate() {
        let Some(zone) = settings
            .status
            .timezones
            .get(settings.zone_start.saturating_add(index))
        else {
            continue;
        };
        card(canvas, bounds, settings.selected == index)?;
        label(
            canvas,
            zone,
            Rect {
                x: bounds.x.saturating_add(inset),
                w: bounds
                    .w
                    .saturating_sub(inset)
                    .saturating_sub(marker)
                    .saturating_sub(gap),
                ..bounds
            },
            scale,
            INK,
        )?;
        if settings.status.timezone.as_ref() == Some(zone) {
            text(
                canvas,
                "current",
                Rect {
                    x: bounds
                        .x
                        .saturating_add(bounds.w)
                        .saturating_sub(marker)
                        .saturating_sub(gap),
                    w: marker,
                    ..bounds
                },
                scale,
                ACCENT,
            )?;
        }
    }
    panel_footer(canvas, layout, settings)
}

fn panel_footer(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    let storage_hint = storage::hint(settings);
    let hint = match settings.page {
        Page::Home => "Enter: open   Esc: close",
        Page::Display => "Left/right: adjust   Esc: back",
        Page::DateTime => "Left/right: change   Enter: select",
        Page::Device => "Left/right: timeout   Enter: select",
        Page::Wireless if settings.selected < 2 => "Left: off   Right: on   Enter: toggle",
        Page::Wireless => "Enter: open   Esc: back",
        Page::Applications => "Enter: change   Esc: back",
        Page::About => "Esc: back",
        Page::Timezones => "Arrows: select   Enter: apply",
        Page::Storage => &storage_hint,
        Page::Updates | Page::Tor | Page::TorDetails => "Esc: back   Enter: select",
    };
    let display_hint = if matches!(
        settings.page,
        Page::Display | Page::Device | Page::DateTime | Page::Wireless | Page::Applications
    ) {
        if settings.pending {
            "Applying setting..."
        } else {
            match settings.system_state {
                crate::settings::SystemState::Loading => "Reading device information...",
                crate::settings::SystemState::Unavailable => "Device information unavailable",
                crate::settings::SystemState::Stale => {
                    "Device information is stale; waiting for refresh"
                }
                crate::settings::SystemState::Ready => hint,
            }
        }
    } else {
        hint
    };
    let half = layout.footer.h / 2_i32;
    text(
        canvas,
        if settings.message.is_empty() {
            display_hint
        } else {
            &settings.message
        },
        Rect {
            h: half,
            ..layout.footer
        },
        layout.text_scale,
        MUTED,
    )?;
    for (bounds, control) in PanelLayout::footer(layout)
        .into_iter()
        .zip(settings.footer_controls())
    {
        let Some((index, label)) = control else {
            continue;
        };
        let footer_bounds = Rect {
            y: bounds.y.saturating_add(half),
            h: half,
            ..bounds
        };
        if settings.selected == index {
            card(canvas, footer_bounds, true)?;
        }
        text(canvas, label, footer_bounds, layout.text_scale, ACCENT)?;
    }
    Ok(())
}

fn slider(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
    geometry: &PanelLayout,
    index: usize,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    let r = *geometry
        .controls
        .get(index)
        .ok_or("Missing slider control geometry")?;
    let scale = layout.text_scale;
    let track = *geometry
        .tracks
        .get(index)
        .ok_or("Missing slider track geometry")?;
    let available = settings.value(index).is_some();
    card(canvas, r, settings.selected == index)?;
    let size = r.h.saturating_mul(3_i32) / 5_i32;
    icon(
        canvas,
        Rect {
            x: r.x.saturating_add((r.h.saturating_sub(size)) / 2_i32),
            y: r.y.saturating_add((r.h.saturating_sub(size)) / 2_i32),
            w: size,
            h: size,
        },
        if index == 0 { Icon::Sun } else { Icon::Speaker },
        if available { ACCENT } else { MUTED },
        textures,
    )?;
    label(
        canvas,
        &format!(
            "{}{}",
            if index == 0 { "Brightness" } else { "Volume" },
            if available {
                ""
            } else if settings.system_state == crate::settings::SystemState::Loading {
                " / loading"
            } else {
                " / unavailable"
            }
        ),
        Rect {
            x: track.x,
            y: r.y.saturating_add(3_i32.saturating_mul(scale)),
            w: track.w,
            h: r.h / 2,
        },
        scale,
        INK,
    )?;
    let value = settings.value(index);
    let display = if index == 1 && settings.status.muted == Some(true) {
        "Muted".into()
    } else {
        value.map_or_else(|| "--".into(), |p| format!("{}%", p.value()))
    };
    text(
        canvas,
        &display,
        Rect {
            x: track
                .x
                .saturating_add(track.w)
                .saturating_add(10_i32.saturating_mul(scale)),
            y: r.y,
            w: 48_i32.saturating_mul(scale),
            h: r.h,
        },
        scale,
        if available { INK } else { MUTED },
    )?;
    let value_width = value.map_or(0_i32, |percent| {
        track.w.saturating_mul(i32::from(percent.value())) / 100_i32
    });
    progress(canvas, track, value_width, false)?;
    if value.is_some() {
        let width = value_width;
        circle(
            canvas,
            track.x.saturating_add(width),
            track.y.saturating_add((track.h) / 2_i32),
            5_i32.saturating_mul(scale),
            INK,
        )?;
        circle(
            canvas,
            track.x.saturating_add(width),
            track.y.saturating_add((track.h) / 2_i32),
            2_i32.saturating_mul(scale),
            ACCENT,
        )?;
    }
    Ok(())
}
fn confirmation(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
    geometry: &PanelLayout,
) -> Result<(), String> {
    let shutdown = settings
        .confirmation
        .is_some_and(|(power, _)| power == Power::Shutdown);
    let bounds = Rect {
        x: layout.title.x,
        y: layout.title.h.saturating_add(8_i32),
        w: layout.title.w,
        h: geometry.controls[2]
            .y
            .saturating_sub(layout.title.h)
            .saturating_sub(16_i32),
    };
    text(
        canvas,
        if shutdown {
            "Power off this device?"
        } else {
            "Restart this device?"
        },
        Rect {
            h: bounds.h / 2,
            ..bounds
        },
        layout.text_scale,
        INK,
    )?;
    text(
        canvas,
        "Apps will close. Save your work.",
        Rect {
            y: bounds.y.saturating_add((bounds.h) / 2_i32),
            h: bounds.h / 2,
            ..bounds
        },
        layout.text_scale,
        MUTED,
    )?;
    for (index, r) in geometry.confirmation.iter().copied().enumerate() {
        card(canvas, r, settings.selected == index)?;
        text(
            canvas,
            if index == 0 {
                "Cancel"
            } else if shutdown {
                "Power off"
            } else {
                "Restart"
            },
            r,
            layout.text_scale,
            if index == 0 { INK } else { AMBER },
        )?;
    }
    text(
        canvas,
        "Esc: back   Enter: select",
        layout.footer,
        layout.text_scale,
        MUTED,
    )
}

/// Device: screen timeout and touch calibration.
fn device_panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    let rows = PanelLayout::rows(layout, 2);
    for (index, bounds) in rows.into_iter().enumerate() {
        let (title, detail, color, kind) = match index {
            0 => (
                "Screen timeout",
                timeout_label(settings.status.screen_timeout),
                ACCENT,
                Icon::Sun,
            ),
            _ => (
                "Calibrate touchscreen",
                if settings.status.calibration {
                    "Tap the targets; any key cancels".into()
                } else {
                    "Unavailable on this device".into()
                },
                if settings.status.calibration {
                    ACCENT
                } else {
                    MUTED
                },
                Icon::Bolt,
            ),
        };
        card(canvas, bounds, settings.selected == index)?;
        let size = bounds.h.saturating_mul(3_i32) / 5_i32;
        icon(
            canvas,
            Rect {
                x: bounds.x.saturating_add(10_i32),
                y: bounds
                    .y
                    .saturating_add((bounds.h.saturating_sub(size)) / 2_i32),
                w: size,
                h: size,
            },
            kind,
            ACCENT,
            textures,
        )?;
        let content = Rect {
            x: bounds.x.saturating_add(size).saturating_add(20_i32),
            w: bounds.w.saturating_sub(size).saturating_sub(32_i32),
            ..bounds
        };
        label(
            canvas,
            title,
            Rect {
                h: bounds.h / 2,
                ..content
            },
            layout.text_scale,
            if settings.selected == index {
                INK
            } else {
                TEXT
            },
        )?;
        label(
            canvas,
            &detail,
            Rect {
                y: bounds.y.saturating_add((bounds.h) / 2_i32),
                h: bounds.h / 2,
                ..content
            },
            layout.text_scale,
            color,
        )?;
    }
    panel_footer(canvas, layout, settings)
}

const fn wifi_label(wifi: Option<Wifi>) -> &'static str {
    match wifi {
        Some(Wifi::Connected) => "Connected",
        Some(Wifi::Connecting) => "Connecting",
        Some(Wifi::Off) => "Off",
        Some(Wifi::Disconnected) => "Not connected",
        None => "Unavailable",
    }
}

fn timeout_label(timeout: Option<u16>) -> String {
    timeout.map_or_else(
        || "Unavailable".into(),
        |seconds| match seconds {
            0 => "< Never >".into(),
            30 => "< 30 seconds >".into(),
            60 => "< 1 minute >".into(),
            _ => format!("< {} minutes >", seconds / 60),
        },
    )
}

fn update_panel(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    use crate::{settings::UpdateConfirmation, updater::State};
    let geometry = PanelLayout::new(layout);
    let confirming = settings
        .update_confirmation
        .as_ref()
        .map(|(confirmation, _)| *confirmation);
    let detail = match (confirming, &settings.updater.state) {
        (Some(UpdateConfirmation::Install), State::Available(release)) => format!(
            "Install Vitrallis Shell {}?\nDownload size: {} MB\nShell, Terminal, Notepad, Files and Arti are replaced together.\nRelaunch after installation.",
            release.version,
            release.download_size_mb()
        ),
        (Some(UpdateConfirmation::Install), _) => {
            "Update no longer available. Cancel and check again.".into()
        }
        (Some(UpdateConfirmation::Restore), _) => "Restore the retained previous build?\nThe active build is kept as the new previous build.\nApps and user data are not changed.\nRelaunch Shell to start the restored build.".into(),
        (None, _) => settings.updater.detail(),
    };
    let message = format!("Running Vitrallis Shell {}\n{detail}", display_version());
    let top = layout.title.h.saturating_add(8_i32);
    let line_height = 12_i32.saturating_mul(layout.text_scale);
    let capacity = usize::try_from(
        layout
            .title
            .w
            .checked_div(8_i32.saturating_mul(layout.text_scale))
            .unwrap_or(0_i32),
    )
    .map_err(|error| format!("update text width: {error}"))?
    .max(1);
    let downloading = matches!(
        settings.updater.state,
        State::Downloading {
            received: _,
            total: _
        }
    );
    let progress_height = if downloading { 20_i32 } else { 0_i32 };
    let max_lines = usize::try_from(
        geometry.controls[2]
            .y
            .saturating_sub(top)
            .saturating_sub(8_i32)
            .saturating_sub(progress_height)
            .checked_div(line_height)
            .unwrap_or(0_i32),
    )
    .map_err(|error| format!("update text height: {error}"))?;
    let lines = update_lines(&message, capacity);
    for (index, line) in lines.iter().take(max_lines).enumerate() {
        label(
            canvas,
            line,
            Rect {
                x: layout.title.x,
                y: top.saturating_add(
                    i32::try_from(index)
                        .map_err(|error| format!("update line index: {error}"))?
                        .saturating_mul(line_height),
                ),
                w: layout.title.w,
                h: line_height,
            },
            layout.text_scale,
            INK,
        )?;
    }
    if let State::Downloading { received, total } = settings.updater.state {
        update_progress(canvas, layout, &geometry, received, total)?;
    }
    // Once relaunch is committed, input is held while service children finish.
    // Do not advertise Back or another relaunch action during that short wait.
    if settings.updater.relaunch_pending() {
        return Ok(());
    }
    let action = update_action(confirming, &settings.updater.state);
    for (index, bounds) in geometry.confirmation.iter().copied().enumerate() {
        card(canvas, bounds, settings.selected == index)?;
        text(
            canvas,
            if index == 0 {
                if confirming.is_some() {
                    "Cancel"
                } else {
                    "Back"
                }
            } else {
                action
            },
            bounds,
            layout.text_scale,
            if index == 1 && confirming.is_some() {
                AMBER
            } else if index == 1 && settings.updater.state.busy() {
                theme::DISABLED
            } else {
                ACCENT
            },
        )?;
    }
    panel_footer(canvas, layout, settings)
}

const fn update_action(
    confirming: Option<crate::settings::UpdateConfirmation>,
    state: &crate::updater::State,
) -> &'static str {
    use crate::{settings::UpdateConfirmation, updater::State};
    match confirming {
        Some(UpdateConfirmation::Install) => "Confirm Install",
        Some(UpdateConfirmation::Restore) => "Confirm Restore",
        None => match state {
            State::Available(_) => "Install Update",
            State::Checking
            | State::Downloading {
                received: _,
                total: _,
            }
            | State::Installing
            | State::Restoring => "Please wait...",
            State::Installed {
                version: _,
                durable: _,
                relaunch: _,
            }
            | State::Restored {
                version: _,
                durable: _,
                relaunch: _,
            } => "Relaunch Shell",
            State::Idle | State::Current | State::Failed(_) => "Check for Updates",
        },
    }
}

fn update_progress(
    canvas: &mut Screen,
    layout: &Layout,
    geometry: &PanelLayout,
    received: u64,
    total: u64,
) -> Result<(), String> {
    let track = Rect {
        x: layout.title.x,
        y: geometry
            .controls
            .get(2)
            .ok_or("Missing settings footer geometry")?
            .y
            .saturating_sub(24_i32),
        w: layout.title.w,
        h: 12,
    };
    let width = u64::try_from(track.w)
        .map_err(|error| format!("update progress width: {error}"))?
        .saturating_mul(received.min(total))
        .checked_div(total)
        .unwrap_or(0);
    progress(
        canvas,
        track,
        i32::try_from(width).map_err(|error| format!("update progress width: {error}"))?,
        false,
    )
}

fn update_lines(message: &str, capacity: usize) -> Vec<String> {
    super::wrap_words(message, capacity)
}

pub(super) fn power_splash(
    canvas: &mut Screen,
    layout: &Layout,
    message: &str,
) -> Result<(), String> {
    canvas.set_draw_color(theme::BACKGROUND);
    canvas.clear();
    let center = i32::from(layout.height) / 2_i32;
    for (title, y, color) in [
        (
            "VITRALLIS",
            center.saturating_sub(36_i32.saturating_mul(layout.text_scale)),
            ACCENT,
        ),
        (
            message,
            center.saturating_sub(8_i32.saturating_mul(layout.text_scale)),
            INK,
        ),
        (
            "Please wait...",
            center.saturating_add(20_i32.saturating_mul(layout.text_scale)),
            MUTED,
        ),
    ] {
        text(
            canvas,
            title,
            Rect {
                x: layout.title.x,
                y,
                w: layout.title.w,
                h: 16_i32.saturating_mul(layout.text_scale),
            },
            layout.text_scale,
            color,
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) use storage::qa as storage_qa;

// Keep visual regression fixtures independent of release version bumps.
const fn display_version() -> &'static str {
    #[cfg(test)]
    {
        "0.1.0-beta4"
    }
    #[cfg(not(test))]
    {
        crate::updater::VERSION
    }
}
