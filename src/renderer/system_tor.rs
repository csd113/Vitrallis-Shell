//! Compact device-sized Tor summary. Rendering never performs service work.
use super::{ACCENT, INK, MUTED, Screen, card, label, panel_footer};
use crate::{
    layout::{Layout, Rect},
    settings::{Page, PanelLayout, Settings},
    tor::{Mode, State},
};

pub(super) fn panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
) -> Result<(), String> {
    let snapshot = &settings.tor;
    if settings.page == Page::TorDetails {
        let rows = PanelLayout::rows(layout, 5);
        let details = [
            "Shared Arti process / loopback SOCKS5".into(),
            "Required apps: isolated network".into(),
            "On demand: stops 30s after last app".into(),
            "Recovery: 3 retries, then manual Start".into(),
            if snapshot.diagnostic.is_empty() {
                "No service errors".into()
            } else {
                snapshot.diagnostic.clone()
            },
        ];
        for (row, value) in rows.into_iter().zip(details) {
            label(canvas, &value, row, layout.text_scale, MUTED)?;
        }
        return panel_footer(canvas, layout, settings);
    }
    let rows = PanelLayout::rows(layout, 4);
    let percent = snapshot
        .progress
        .map_or_else(|| "--".into(), |p| format!("{p}%"));
    let service = match snapshot.state {
        State::Connected | State::Bootstrapping => "Running",
        State::Error => "Error",
        State::Starting => "Starting",
        State::Stopping => "Stopping",
        _ => "Stopped",
    };
    let status = format!(
        "{} / {} / {}",
        if snapshot.mode == Mode::Disabled {
            "Disabled"
        } else {
            "Enabled"
        },
        match snapshot.state {
            State::Connected => "Connected",
            State::Starting | State::Bootstrapping => "Bootstrapping",
            _ => "Offline",
        },
        percent
    );
    let detail = format!("Service: {service}    Apps: {}", snapshot.apps);
    for (bounds, top, bottom) in [
        (
            rows[0],
            status,
            format!("Startup: {}", snapshot.mode.label()),
        ),
        (rows[1], detail, "SOCKS: 127.0.0.1:9150".into()),
    ] {
        card(canvas, bounds, false)?;
        let line = Rect {
            x: bounds.x + 10,
            w: bounds.w - 20,
            h: bounds.h / 2,
            ..bounds
        };
        label(canvas, &top, line, layout.text_scale, INK)?;
        label(
            canvas,
            &bottom,
            Rect {
                y: bounds.y + bounds.h / 2,
                ..line
            },
            layout.text_scale,
            MUTED,
        )?;
    }
    let labels = [
        "Start",
        "Stop",
        "Restart",
        if snapshot.mode == Mode::Disabled {
            "Enable"
        } else {
            "Disable"
        },
        "Startup",
        "Details >",
    ];
    for (index, bounds) in PanelLayout::tor_controls(layout).into_iter().enumerate() {
        card(canvas, bounds, settings.selected == index)?;
        label(
            canvas,
            labels[index],
            Rect {
                x: bounds.x + 8,
                w: bounds.w - 16,
                ..bounds
            },
            layout.text_scale,
            ACCENT,
        )?;
    }
    panel_footer(canvas, layout, settings)
}
