//! Native radio switches; no filesystem or subprocess work during rendering.
use super::{ACCENT, INK, MUTED, Screen, card, label, panel_footer, text};
use crate::{
    layout::{Layout, Rect},
    settings::{PanelLayout, Settings, WIRELESS_ROWS},
};

pub(super) fn panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
) -> Result<(), String> {
    let titles = ["Wi-Fi", "Bluetooth", "Wi-Fi connections >", "Tor >"];
    for (index, (bounds, title)) in
        PanelLayout::rows(layout, i32::try_from(WIRELESS_ROWS).unwrap_or(4_i32))
            .into_iter()
            .zip(titles)
            .enumerate()
    {
        card(canvas, bounds, settings.selected == index)?;
        let value = settings.radio_value(index);
        let detail = match index {
            0 if value == Some(true) => super::wifi_label(settings.status.wifi),
            0 | 1 if value == Some(false) => "Off",
            1 if value == Some(true) => "Default controller powered on",
            2 if settings.network_available => "Choose a network / enter password",
            3 => settings.tor.mode.label(),
            _ => "Unavailable on this device",
        };
        let content = Rect {
            x: bounds.x.saturating_add(12_i32),
            w: bounds
                .w
                .saturating_sub(24_i32)
                .saturating_sub(if index < 2 {
                    80_i32.saturating_mul(layout.text_scale)
                } else {
                    0
                }),
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
            INK,
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
            MUTED,
        )?;
        if index < 2 {
            let toggle = Rect {
                x: bounds
                    .x
                    .saturating_add(bounds.w)
                    .saturating_sub(76_i32.saturating_mul(layout.text_scale)),
                y: bounds.y.saturating_add((bounds.h) / 4_i32),
                w: 68_i32.saturating_mul(layout.text_scale),
                h: bounds.h / 2,
            };
            text(
                canvas,
                match value {
                    Some(true) => "[ ON ]",
                    Some(false) => "[ OFF ]",
                    None => "[ -- ]",
                },
                toggle,
                layout.text_scale,
                if value == Some(true) { ACCENT } else { MUTED },
            )?;
        }
    }
    panel_footer(canvas, layout, settings)
}
