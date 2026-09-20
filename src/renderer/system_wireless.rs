//! Native radio switches; no filesystem or subprocess work during rendering.
use super::{ACCENT, INK, MUTED, Screen, card, label, panel_footer, text};
use crate::{
    layout::{Layout, Rect},
    settings::{PanelLayout, Settings},
};

pub(super) fn panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
) -> Result<(), String> {
    for (index, bounds) in PanelLayout::rows(layout, 3).into_iter().enumerate() {
        card(canvas, bounds, settings.selected == index)?;
        let value = settings.radio_value(index);
        let title = ["Wi-Fi", "Bluetooth", "Wi-Fi connections >"][index];
        let detail = match index {
            0 if value == Some(true) => super::wifi_label(settings.status.wifi),
            0 | 1 if value == Some(false) => "Off",
            1 if value == Some(true) => "Default controller powered on",
            2 if settings.network_available => "Choose a network / enter password",
            _ => "Unavailable on this device",
        };
        let content = Rect {
            x: bounds.x + 12,
            w: bounds.w - 24 - if index < 2 { 80 * layout.text_scale } else { 0 },
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
                y: bounds.y + bounds.h / 2,
                h: bounds.h / 2,
                ..content
            },
            layout.text_scale,
            MUTED,
        )?;
        if index < 2 {
            let toggle = Rect {
                x: bounds.x + bounds.w - 76 * layout.text_scale,
                y: bounds.y + bounds.h / 4,
                w: 68 * layout.text_scale,
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
