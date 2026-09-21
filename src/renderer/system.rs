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

pub(super) const ASSETS: [&[u8]; 5] = [
    include_bytes!("../../assets/system/wifi.png"),
    include_bytes!("../../assets/system/sun.png"),
    include_bytes!("../../assets/system/speaker.png"),
    include_bytes!("../../assets/system/power.png"),
    include_bytes!("../../assets/system/restart.png"),
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
    Power,
    Restart,
    Bolt,
}
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
        Icon::Power => Some(3),
        Icon::Restart => Some(4),
        Icon::Bolt => None,
    };
    if let Some(texture) = index
        .and_then(|index| textures.get(index))
        .and_then(Option::as_ref)
    {
        return canvas.copy(texture, None, rect(r)?);
    }
    canvas.set_draw_color(color);
    for y in 0..r.h {
        for x in 0..r.w {
            let dx = x * 32 / r.w - 16;
            let dy = y * 32 / r.h - 16;
            if pixel(kind, dx, dy) {
                canvas.draw_point((r.x + x, r.y + y))?;
            }
        }
    }
    Ok(())
}
fn pixel(kind: Icon, x: i32, y: i32) -> bool {
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
        Icon::Power => {
            ((100..=169).contains(&radius) && (y >= -6 || x.abs() >= 6))
                || (x.abs() <= 1 && (-14..=0).contains(&y))
        }
        Icon::Restart => {
            ((81..=144).contains(&radius) && (x < 5 || y > 0))
                || ((3..=12).contains(&x) && (-13..=-4).contains(&y) && y >= x - 16)
        }
        Icon::Bolt => polygon(
            &[(6, -14), (-9, 3), (-1, 3), (-5, 14), (10, -5), (2, -5)],
            x,
            y,
        ),
    }
}
fn polygon(points: &[(i32, i32)], x: i32, y: i32) -> bool {
    let mut winding = 0;
    for (&(ax, ay), &(bx, by)) in points.iter().zip(points.iter().cycle().skip(1)) {
        let cross = (bx - ax) * (y - ay) - (x - ax) * (by - ay);
        if ay <= y && by > y && cross > 0 {
            winding += 1;
        }
        if ay > y && by <= y && cross < 0 {
            winding -= 1;
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
fn circle(canvas: &mut Screen, x: i32, y: i32, radius: i32, color: Color) -> Result<(), String> {
    canvas.set_draw_color(color);
    for dy in -radius..=radius {
        let dx = (0..=radius)
            .take_while(|dx| dx * dx + dy * dy <= radius * radius)
            .last()
            .unwrap_or(0);
        canvas.draw_line((x - dx, y + dy), (x + dx, y + dy))?;
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
    let clock_width = i32::try_from(clock.len()).map_err(|_| "clock length")? * advance(scale);
    // Fixed slots from the right edge: battery, charge marker, radio mark, clock.
    let width = 128 * scale + clock_width;
    let x = i32::from(layout.width) - layout.title.x - width;
    label(
        canvas,
        &status
            .ip
            .map_or_else(|| "IP --".into(), |ip| format!("IP {ip}")),
        Rect {
            x: layout.title.x,
            y: layout.title.h / 2,
            w: x - layout.title.x - 8 * scale,
            h: layout.title.h / 2,
        },
        scale,
        MUTED,
    )?;
    let y = layout.title.h * 3 / 4;
    battery(canvas, status, x, y, scale)?;
    if status.charging == Some(true) || status.external_power == Some(true) {
        icon(
            canvas,
            Rect {
                x: x + 67 * scale,
                y: y - 8 * scale,
                w: 16 * scale,
                h: 16 * scale,
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
        x: x + 94 * scale,
        y: y - 8 * scale,
        w: 18 * scale,
        h: 16 * scale,
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
        canvas.draw_line((wifi.x, wifi.y + wifi.h), (wifi.x + wifi.w, wifi.y))?;
    }
    label(
        canvas,
        &clock,
        Rect {
            x: x + 128 * scale,
            y: y - 6 * scale,
            w: clock_width,
            h: 12 * scale,
        },
        scale,
        INK,
    )
}

fn battery(canvas: &mut Screen, status: &Status, x: i32, y: i32, scale: i32) -> Result<(), String> {
    let battery = Rect {
        x,
        y: y - 6 * scale,
        w: 25 * scale,
        h: 12 * scale,
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
            x: x + battery.w,
            y: y - 2 * scale,
            w: 2 * scale,
            h: 4 * scale,
        },
        MUTED,
    )?;
    if let Some(value) = status.battery {
        let w = (battery.w - 4 * scale) * i32::from(value.value()) / 100;
        if w > 0 {
            fill(
                canvas,
                Rect {
                    x: x + 2 * scale,
                    y: battery.y + 2 * scale,
                    w,
                    h: battery.h - 4 * scale,
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
            x: x + 34 * scale,
            y: y - 6 * scale,
            w: 32 * scale,
            h: 12 * scale,
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

/// Home menu: the same large-option style is reused by every category, so a
/// user always sees one visual system. Two columns by four rows.
fn home_panel(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    for (index, (bounds, (title, detail))) in PanelLayout::home(layout)
        .into_iter()
        .zip(settings.home_rows())
        .enumerate()
    {
        card(canvas, bounds, settings.selected == index)?;
        text(
            canvas,
            &title,
            Rect {
                y: bounds.y + 4 * layout.text_scale,
                h: bounds.h / 2,
                ..bounds
            },
            layout.text_scale,
            if settings.selected == index {
                INK
            } else {
                TEXT
            },
        )?;
        text(
            canvas,
            &detail,
            Rect {
                y: bounds.y + bounds.h / 2,
                h: bounds.h / 2,
                ..bounds
            },
            layout.text_scale,
            MUTED,
        )?;
    }
    panel_footer(canvas, layout, settings)
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
        x: bounds.x + 12,
        w: bounds.w - 24,
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
            y: bounds.y + bounds.h / 2,
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
        i32::try_from(crate::settings::preferences::APP_ROWS).unwrap_or(3),
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
            x: bounds.x + 12,
            w: bounds.w - 24,
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
                y: bounds.y + bounds.h / 2,
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
    let marker = 60 * scale;
    let inset = 12 * scale;
    let gap = 8 * scale;
    for (index, bounds) in PanelLayout::rows(layout, 5).into_iter().enumerate() {
        let Some(zone) = settings.status.timezones.get(settings.zone_start + index) else {
            continue;
        };
        card(canvas, bounds, settings.selected == index)?;
        label(
            canvas,
            zone,
            Rect {
                x: bounds.x + inset,
                w: bounds.w - inset - marker - gap,
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
                    x: bounds.x + bounds.w - marker - gap,
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
        Page::Wireless => "Left: off   Right: on   Enter: toggle",
        Page::Applications => "Enter: change   Esc: back",
        Page::About => "Esc: back",
        Page::Timezones => "Arrows: select   Enter: apply",
        Page::Storage => &storage_hint,
        Page::Updates | Page::Tor | Page::TorDetails => "Esc: back   Enter: select",
    };
    let hint = if matches!(
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
    let half = layout.footer.h / 2;
    text(
        canvas,
        if settings.message.is_empty() {
            hint
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
        let bounds = Rect {
            y: bounds.y + half,
            h: half,
            ..bounds
        };
        if settings.selected == index {
            card(canvas, bounds, true)?;
        }
        text(canvas, label, bounds, layout.text_scale, ACCENT)?;
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
    let r = geometry.controls[index];
    let scale = layout.text_scale;
    let track = geometry.tracks[index];
    let available = settings.value(index).is_some();
    card(canvas, r, settings.selected == index)?;
    let size = r.h * 3 / 5;
    icon(
        canvas,
        Rect {
            x: r.x + (r.h - size) / 2,
            y: r.y + (r.h - size) / 2,
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
            y: r.y + 3 * scale,
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
            x: track.x + track.w + 10 * scale,
            y: r.y,
            w: 48 * scale,
            h: r.h,
        },
        scale,
        if available { INK } else { MUTED },
    )?;
    let value_width = value.map_or(0, |value| track.w * i32::from(value.value()) / 100);
    progress(canvas, track, value_width, false)?;
    if value.is_some() {
        let width = value_width;
        circle(
            canvas,
            track.x + width,
            track.y + track.h / 2,
            5 * scale,
            INK,
        )?;
        circle(
            canvas,
            track.x + width,
            track.y + track.h / 2,
            2 * scale,
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
        y: layout.title.h + 8,
        w: layout.title.w,
        h: geometry.controls[2].y - layout.title.h - 16,
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
            y: bounds.y + bounds.h / 2,
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

/// Device: hardware controls and the two guarded power actions.
fn device_panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
    textures: &[Option<Texture<'_>>],
) -> Result<(), String> {
    if settings.confirmation.is_some() {
        return confirmation(canvas, layout, settings, &PanelLayout::new(layout));
    }
    let rows = PanelLayout::rows(layout, 4);
    for (index, bounds) in rows.into_iter().enumerate() {
        let (title, detail, color, kind) = match index {
            0 => (
                "Screen timeout",
                timeout_label(settings.status.screen_timeout),
                ACCENT,
                Icon::Sun,
            ),
            1 => (
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
            2 => (
                "Restart",
                "Asks for confirmation".into(),
                if settings.status.power_controls {
                    AMBER
                } else {
                    MUTED
                },
                Icon::Restart,
            ),
            _ => (
                "Power off",
                "Asks for confirmation".into(),
                if settings.status.power_controls {
                    AMBER
                } else {
                    MUTED
                },
                Icon::Power,
            ),
        };
        card(canvas, bounds, settings.selected == index)?;
        let size = bounds.h * 3 / 5;
        icon(
            canvas,
            Rect {
                x: bounds.x + 10,
                y: bounds.y + (bounds.h - size) / 2,
                w: size,
                h: size,
            },
            kind,
            if index < 2 || settings.status.power_controls {
                ACCENT
            } else {
                MUTED
            },
            textures,
        )?;
        let content = Rect {
            x: bounds.x + size + 20,
            w: bounds.w - size - 32,
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
                y: bounds.y + bounds.h / 2,
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
            seconds => format!("< {} minutes >", seconds / 60),
        },
    )
}

fn update_panel(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    use crate::updater::State;
    let geometry = PanelLayout::new(layout);
    let confirming = settings.update_confirmation.is_some();
    let detail = if confirming {
        match &settings.updater.state {
            State::Available(release) => format!(
                "Install Vitrallis Shell {}?\nDownload size: {} MB\nOnly the shell executable will be replaced.\nRelaunch after installation.",
                release.version,
                release.download_size_mb()
            ),
            _ => "Update no longer available. Cancel and check again.".into(),
        }
    } else {
        settings.updater.detail()
    };
    let message = format!("Running Vitrallis Shell {}\n{detail}", display_version());
    let top = layout.title.h + 8;
    let line_height = 12 * layout.text_scale;
    let capacity = usize::try_from(layout.title.w / (8 * layout.text_scale))
        .map_err(|_| "update text width")?
        .max(1);
    let downloading = matches!(settings.updater.state, State::Downloading { .. });
    let progress_height = if downloading { 20 } else { 0 };
    let max_lines =
        usize::try_from((geometry.controls[2].y - top - 8 - progress_height) / line_height)
            .map_err(|_| "update text height")?;
    let lines = update_lines(&message, capacity);
    for (index, line) in lines.iter().take(max_lines).enumerate() {
        label(
            canvas,
            line,
            Rect {
                x: layout.title.x,
                y: top + i32::try_from(index).map_err(|_| "update line index")? * line_height,
                w: layout.title.w,
                h: line_height,
            },
            layout.text_scale,
            INK,
        )?;
    }
    if let State::Downloading { received, total } = settings.updater.state {
        let track = Rect {
            x: layout.title.x,
            y: geometry.controls[2].y - 24,
            w: layout.title.w,
            h: 12,
        };
        let width = u64::try_from(track.w)
            .map_err(|_| "update progress width")?
            .saturating_mul(received.min(total))
            .checked_div(total)
            .unwrap_or(0);
        progress(
            canvas,
            track,
            i32::try_from(width).map_err(|_| "update progress width")?,
            false,
        )?;
    }
    let action = if confirming {
        "Confirm Install"
    } else {
        match settings.updater.state {
            State::Available(_) => "Install Update",
            State::Checking | State::Downloading { .. } | State::Installing => "Please wait...",
            State::Installed { .. } => "Relaunch Shell",
            _ => "Check for Updates",
        }
    };
    for (index, bounds) in geometry.confirmation.iter().copied().enumerate() {
        card(canvas, bounds, settings.selected == index)?;
        text(
            canvas,
            if index == 0 {
                if confirming { "Cancel" } else { "Back" }
            } else {
                action
            },
            bounds,
            layout.text_scale,
            if index == 1 && confirming {
                AMBER
            } else {
                ACCENT
            },
        )?;
    }
    panel_footer(canvas, layout, settings)
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
    let center = i32::from(layout.height) / 2;
    for (title, y, color) in [
        ("VITRALLIS", center - 36 * layout.text_scale, ACCENT),
        (message, center - 8 * layout.text_scale, INK),
        ("Please wait...", center + 20 * layout.text_scale, MUTED),
    ] {
        text(
            canvas,
            title,
            Rect {
                x: layout.title.x,
                y,
                w: layout.title.w,
                h: 16 * layout.text_scale,
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
