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
                lines.push(
                    if matches!(
                        app.id.as_str(),
                        "io.vitrallis.mediacarousel" | "io.vitrallis.carouselrust"
                    ) {
                        "Carousel library shared; counted once.".into()
                    } else {
                        "Known locations; shared files counted once.".into()
                    },
                );
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
                settings.storage_start.saturating_add(1_usize)
            },
            settings.storage_start.saturating_add(4_usize).min(count)
        );
    }
    "Arrows: select   Enter: open   Esc: back".into()
}

fn body(canvas: &mut Screen, layout: &Layout, lines: &[String]) -> Result<(), String> {
    let top = layout.title.h.saturating_add(6_i32);
    let height = layout.footer.y.saturating_sub(top).saturating_sub(6_i32) / 8_i32;
    for (index, line) in lines.iter().take(8).enumerate() {
        label(
            canvas,
            line,
            Rect {
                x: layout.title.x.saturating_add(4_i32),
                y: top.saturating_add(
                    i32::try_from(index)
                        .map_err(|error| format!("storage line: {error}"))?
                        .saturating_mul(height),
                ),
                w: layout.title.w.saturating_sub(8_i32),
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
    let top = layout.title.h.saturating_add(5_i32);
    let line_height = (actions
        .first()
        .ok_or("Missing storage action geometry")?
        .y
        .saturating_sub(top)
        .saturating_sub(15_i32)
        / 5_i32)
        .max(10_i32);
    let row = |index: i32| Rect {
        x: layout.title.x.saturating_add(4_i32),
        y: top.saturating_add(index.saturating_mul(line_height)),
        w: layout.title.w.saturating_sub(8_i32),
        h: line_height,
    };
    disk_capacity(canvas, layout, settings, row)?;
    let summary = settings.storage.report.as_ref().map_or_else(
        || storage_state(settings).to_owned(),
        |report| {
            format!(
                "Applications: {} in {} apps",
                report.app_total.label(),
                report.apps.len()
            )
        },
    );
    label(canvas, &summary, row(4_i32), layout.text_scale, INK)?;
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
fn disk_capacity(
    canvas: &mut Screen,
    layout: &Layout,
    settings: &Settings,
    row: impl Fn(i32) -> Rect,
) -> Result<(), String> {
    match &settings.storage.disk {
        Some(Ok(disk)) => {
            label(
                canvas,
                &format!("{} mounted at {}", disk.filesystem, disk.mount),
                row(0_i32),
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
                row(1_i32),
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
                row(2_i32),
                layout.text_scale,
                if critical { AMBER } else { ACCENT },
            )?;
            let track = Rect {
                y: row(3).y.saturating_add(3_i32),
                h: 5_i32.saturating_mul(layout.text_scale),
                ..row(3_i32)
            };
            progress(
                canvas,
                track,
                track.w.saturating_mul(i32::from(disk.percent())) / 100,
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
                row(0_i32),
                layout.text_scale,
                INK,
            )?;
            if let Some(Err(error)) = disk {
                label(canvas, error, row(1_i32), layout.text_scale, AMBER)?;
            }
        }
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
                    "No applications found".into()
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
        let size = bounds
            .h
            .min(32_i32.saturating_mul(layout.text_scale))
            .saturating_sub(4_i32);
        let icon = Rect {
            x: bounds.x.saturating_add(5_i32),
            y: bounds
                .y
                .saturating_add((bounds.h.saturating_sub(size)) / 2_i32),
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
        let x = icon.x.saturating_add(icon.w).saturating_add(8_i32);
        label(
            canvas,
            &app.name,
            Rect {
                x,
                w: bounds
                    .w
                    .saturating_sub(x.saturating_sub(bounds.x))
                    .saturating_sub(5_i32),
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
                y: bounds.y.saturating_add((bounds.h) / 2_i32),
                w: bounds
                    .w
                    .saturating_sub(x.saturating_sub(bounds.x))
                    .saturating_sub(5_i32),
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
    let save = |screen: &mut Screen, snapshot: &crate::launcher::Launcher, name: &str| {
        super::super::render(screen, layout, snapshot, textures)?;
        super::super::screenshot(
            screen,
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
    let apps: Vec<_> = (0..6_u64)
        .zip([
            "Python Reader",
            "Rust Paint",
            "Notes",
            "Music",
            "Weather",
            "Unavailable app",
        ])
        .map(|(index, name)| {
            let parts = [1_u64, 2, 3, 4, 5].map(|factor| Size {
                bytes: 6_u64
                    .saturating_sub(index)
                    .saturating_mul(1_000_000)
                    .saturating_mul(factor),
                incomplete: index == 5,
                issue: (index == 5).then(|| "Permission denied".into()),
            });
            AppUsage {
                id: format!("io.test.app{index}"),
                name: name.into(),
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
