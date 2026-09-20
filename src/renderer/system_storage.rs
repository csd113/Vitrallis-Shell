//! Compact storage views; rendering reads snapshots only.
use super::{ACCENT, AMBER, INK, MUTED, Screen, card, label, panel_footer, progress, text};
use crate::{
    layout::{Layout, Rect},
    settings::{PanelLayout, Settings, StorageView},
    storage::format_bytes,
};

pub(super) fn panel(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
) -> Result<(), String> {
    match settings.storage_view {
        StorageView::Overview => overview(canvas, layout, settings)?,
        StorageView::Apps => apps(canvas, layout, settings)?,
        StorageView::App(index) => {
            if let Some(app) = settings
                .storage
                .report
                .as_ref()
                .and_then(|report| report.apps.get(index))
            {
                let mut lines = vec![format!("{}  {}", app.name, app.total.label())];
                lines.extend(
                    crate::app_center::accounting::PARTS
                        .iter()
                        .zip(&app.parts)
                        .map(|(name, size)| format!("{name}: {}", size.label())),
                );
                lines.push("Known locations only; shared files counted once.".into());
                lines.push(
                    app.total.issue.clone().unwrap_or_else(|| {
                        "Data outside managed locations is not included.".into()
                    }),
                );
                body(canvas, layout, &lines)?;
            } else {
                body(
                    canvas,
                    layout,
                    &["App no longer available. Go back and Refresh.".into()],
                )?;
            }
        }
        StorageView::Categories => {
            let mut lines = Vec::new();
            if let Some(report) = &settings.storage.report {
                lines.extend(
                    report
                        .categories
                        .iter()
                        .map(|(name, size)| format!("{name}: {}", size.label())),
                );
                if let Some(Ok(disk)) = &settings.storage.disk {
                    lines.push(format!(
                        "Other / unscanned on /: ~{}",
                        format_bytes(disk.used.saturating_sub(report.root_bytes))
                    ));
                }
                lines.push("Allocated bytes; separate mounts may differ.".into());
            } else {
                lines.push(storage_state(settings).into());
            }
            body(canvas, layout, &lines)?;
        }
    }
    panel_footer(canvas, layout, settings)
}
pub(super) fn hint(settings: &Settings) -> String {
    if settings.storage.busy() {
        return "Scanning... you can leave this page".into();
    }
    if settings.storage.error.is_some() {
        return if settings.storage.report.is_some() {
            "Refresh failed; previous sizes shown"
        } else {
            "Scan failed. Refresh to retry"
        }
        .into();
    }
    if settings.storage_view == StorageView::Apps
        && let Some(report) = &settings.storage.report
    {
        if report.issue.is_some() {
            return "App list incomplete. Refresh to retry".into();
        }
        let count = report.apps.len();
        return format!(
            "Largest first   {}-{} / {count}",
            if count == 0 {
                0
            } else {
                settings.storage_start + 1
            },
            (settings.storage_start + 4).min(count)
        );
    }
    "Arrows: select   Enter: open   Esc: back".into()
}

fn body(canvas: &mut Screen, layout: &Layout, lines: &[String]) -> Result<(), String> {
    let top = layout.title.h + 6;
    let height = (layout.footer.y - top - 6) / 8;
    for (index, line) in lines.iter().take(8).enumerate() {
        label(
            canvas,
            line,
            Rect {
                x: layout.title.x + 4,
                y: top + i32::try_from(index).map_err(|_| "storage line")? * height,
                w: layout.title.w - 8,
                h: height,
            },
            layout.text_scale,
            if index == 0 { INK } else { MUTED },
        )?;
    }
    Ok(())
}
fn storage_state(settings: &Settings) -> &str {
    if settings.storage.busy() {
        "Calculating storage..."
    } else if let Some(error) = &settings.storage.error {
        error
    } else {
        "Storage unavailable. Select Refresh to retry."
    }
}
fn overview(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    let actions = PanelLayout::storage_actions(layout);
    let top = layout.title.h + 5;
    let line_height = ((actions[0].y - top - 15) / 5).max(10);
    let row = |index| Rect {
        x: layout.title.x + 4,
        y: top + index * line_height,
        w: layout.title.w - 8,
        h: line_height,
    };
    match &settings.storage.disk {
        Some(Ok(disk)) => {
            label(
                canvas,
                &format!("{} mounted at {}", disk.filesystem, disk.mount),
                row(0),
                layout.text_scale,
                MUTED,
            )?;
            label(
                canvas,
                &format!(
                    "Total {}   Used {} ({}%)",
                    format_bytes(disk.total),
                    format_bytes(disk.used),
                    disk.percent()
                ),
                row(1),
                layout.text_scale,
                INK,
            )?;
            let critical = settings.storage.thresholds.critical(disk);
            label(
                canvas,
                &format!(
                    "Available {}{}",
                    format_bytes(disk.available),
                    if critical { "  LOW STORAGE" } else { "" }
                ),
                row(2),
                layout.text_scale,
                if critical { AMBER } else { ACCENT },
            )?;
            let track = Rect {
                y: row(3).y + 3,
                h: 5 * layout.text_scale,
                ..row(3)
            };
            progress(
                canvas,
                track,
                track.w * i32::from(disk.percent()) / 100,
                critical,
            )?;
        }
        disk => {
            label(
                canvas,
                if disk.is_none() {
                    "Reading filesystem capacity..."
                } else {
                    "Filesystem capacity unavailable"
                },
                row(0),
                layout.text_scale,
                INK,
            )?;
            if let Some(Err(error)) = disk {
                label(canvas, error, row(1), layout.text_scale, AMBER)?;
            }
        }
    }
    let summary = settings.storage.report.as_ref().map_or_else(
        || storage_state(settings).to_owned(),
        |report| {
            format!(
                "App Manager: {} in {} apps",
                report.app_total.label(),
                report.apps.len()
            )
        },
    );
    label(canvas, &summary, row(4), layout.text_scale, INK)?;
    for (index, bounds) in actions.into_iter().enumerate() {
        card(canvas, bounds, settings.selected == index)?;
        text(
            canvas,
            if index == 0 {
                "App storage >"
            } else {
                "Storage details >"
            },
            bounds,
            layout.text_scale,
            ACCENT,
        )?;
    }
    Ok(())
}
fn apps(canvas: &mut Screen, layout: &Layout, settings: &Settings) -> Result<(), String> {
    let Some(report) = &settings.storage.report else {
        return body(canvas, layout, &[storage_state(settings).into()]);
    };
    if report.apps.is_empty() {
        return body(
            canvas,
            layout,
            &[
                if report.issue.is_some() {
                    "Installed app list unavailable".into()
                } else {
                    "No App Manager apps installed".into()
                },
                report
                    .issue
                    .clone()
                    .unwrap_or_else(|| "Install apps in App Center to see their storage.".into()),
            ],
        );
    }
    for (index, (bounds, app)) in PanelLayout::rows(layout, 4)
        .into_iter()
        .zip(report.apps.iter().skip(settings.storage_start))
        .enumerate()
    {
        card(canvas, bounds, settings.selected == index)?;
        let size = bounds.h.min(32 * layout.text_scale) - 4;
        let icon = Rect {
            x: bounds.x + 5,
            y: bounds.y + (bounds.h - size) / 2,
            w: size,
            h: size,
        };
        if let Some(pixels) = &app.icon {
            super::super::app_center::draw_icon(canvas, pixels, icon)?;
        } else {
            text(
                canvas,
                &app.name.chars().next().unwrap_or('?').to_string(),
                icon,
                layout.text_scale,
                ACCENT,
            )?;
        }
        let x = icon.x + icon.w + 8;
        label(
            canvas,
            &app.name,
            Rect {
                x,
                w: bounds.w - (x - bounds.x) - 5,
                h: bounds.h / 2,
                ..bounds
            },
            layout.text_scale,
            INK,
        )?;
        label(
            canvas,
            &app.total.label(),
            Rect {
                x,
                y: bounds.y + bounds.h / 2,
                w: bounds.w - (x - bounds.x) - 5,
                h: bounds.h / 2,
            },
            layout.text_scale,
            if app.total.incomplete { AMBER } else { MUTED },
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::renderer) fn qa(
    canvas: &mut Screen,
    layout: &Layout,
    state: &mut crate::launcher::Launcher,
    textures: &[Option<sdl2::render::Texture<'_>>],
    output: &std::path::Path,
) -> Result<(), String> {
    use crate::{
        app_center::accounting::{AppUsage, aggregate},
        settings::Page,
        storage::{Disk, Report, scan::Size},
    };
    let original = state.settings.page;
    state.settings.page(Page::Storage);
    let save = |canvas: &mut Screen, state: &crate::launcher::Launcher, name: &str| {
        super::super::render(canvas, layout, state, textures)?;
        super::super::screenshot(
            canvas,
            &output.join(format!(
                "storage-{name}-{}x{}.bmp",
                layout.width, layout.height
            )),
        )
    };
    save(canvas, state, "loading")?;
    state.settings.storage.disk = Some(Ok(Disk {
        filesystem: "/dev/mmcblk0p1".into(),
        mount: "/".into(),
        total: 4_000_000_000,
        used: 3_950_000_000,
        available: 40_000_000,
    }));
    let apps: Vec<_> = (0..6)
        .map(|index| {
            let parts = std::array::from_fn(|part| Size {
                bytes: (6 - index) * 1_000_000 * (u64::try_from(part).unwrap() + 1),
                incomplete: index == 5,
                issue: (index == 5).then(|| "Permission denied".into()),
            });
            AppUsage {
                id: format!("io.test.app{index}"),
                name: [
                    "Python Reader",
                    "Rust Paint",
                    "Notes",
                    "Music",
                    "Weather",
                    "Unavailable app",
                ][usize::try_from(index).unwrap()]
                .into(),
                icon: None,
                total: aggregate(parts.iter()),
                parts,
            }
        })
        .collect();
    state.settings.storage.report = Some(Report {
        app_total: aggregate(apps.iter().map(|a| &a.total)),
        apps,
        categories: vec![(
            "Shell executable",
            Size {
                bytes: 3_200_000,
                ..Size::default()
            },
        )],
        root_bytes: 200_000_000,
        issue: None,
    });
    for (name, view) in [
        ("overview", StorageView::Overview),
        ("apps", StorageView::Apps),
        ("detail", StorageView::App(0)),
        ("categories", StorageView::Categories),
    ] {
        state.settings.storage_view = view;
        state.settings.selected = if matches!(view, StorageView::App(_) | StorageView::Categories) {
            6
        } else {
            0
        };
        save(canvas, state, name)?;
    }
    state.settings.storage_view = StorageView::Apps;
    state.settings.storage_start = 4;
    save(canvas, state, "partial")?;
    state.settings.storage_start = 0;
    state.settings.storage.report = Some(Report::default());
    save(canvas, state, "empty")?;
    state.settings.storage.report = None;
    state.settings.storage.disk = Some(Err("df: command timed out".into()));
    state.settings.storage.error = Some("App storage unavailable: permission denied".into());
    state.settings.storage_view = StorageView::Overview;
    save(canvas, state, "error")?;
    state.settings.page(original);
    Ok(())
}
