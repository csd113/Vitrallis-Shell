use crate::layout::{Layout, Rect};

pub struct PanelLayout {
    pub controls: [Rect; 5],
    pub tracks: [Rect; 2],
    pub confirmation: [Rect; 2],
}
impl PanelLayout {
    pub fn footer(layout: &Layout) -> [Rect; 3] {
        [0_i32, 1_i32, 2_i32].map(|index| Rect {
            x: layout
                .footer
                .x
                .saturating_add(index.saturating_mul(layout.footer.w) / 3),
            w: layout.footer.w / 3,
            ..layout.footer
        })
    }

    /// Large home-menu options: two columns by four rows.
    pub fn home(layout: &Layout) -> [Rect; 8] {
        let gap = (i32::from(layout.height) / 40_i32).max(5_i32);
        let top = layout.title.h.saturating_add(gap);
        // One shared summary line replaces eight cramped two-line cards.
        let available = layout
            .footer
            .y
            .saturating_sub(top)
            .saturating_sub(gap)
            .saturating_sub(12_i32.saturating_mul(layout.text_scale));
        let height = available.saturating_sub(3_i32.saturating_mul(gap)) / 4_i32;
        let width = layout.title.w.saturating_sub(gap) / 2_i32;
        std::array::from_fn(|index| {
            let column = i32::try_from(index % 2).unwrap_or(0_i32);
            let row = i32::try_from(index / 2).unwrap_or(0_i32);
            Rect {
                x: layout
                    .title
                    .x
                    .saturating_add(column.saturating_mul(width.saturating_add(gap))),
                y: top.saturating_add(row.saturating_mul(height.saturating_add(gap))),
                w: width,
                h: height,
            }
        })
    }

    pub fn tor_controls(layout: &Layout) -> [Rect; 6] {
        let rows = Self::rows(layout, 4);
        let mut controls = [Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }; 6];
        for (group, row) in controls.chunks_mut(3).zip(rows.iter().skip(2)) {
            let width = row.w.saturating_sub(12) / 3_i32;
            for (column, control) in group.iter_mut().enumerate() {
                *control = Rect {
                    x: row.x.saturating_add(
                        i32::try_from(column)
                            .unwrap_or(0_i32)
                            .saturating_mul(width.saturating_add(6)),
                    ),
                    w: width,
                    ..*row
                };
            }
        }
        controls
    }
    pub fn storage_actions(layout: &Layout) -> [Rect; 2] {
        Self::new(layout).confirmation
    }

    pub fn rows(layout: &Layout, count: i32) -> Vec<Rect> {
        if count <= 0_i32 {
            return Vec::new();
        }
        let gap = (i32::from(layout.height) / 40_i32).max(5_i32);
        let top = layout.title.h.saturating_add(gap);
        let h = layout
            .footer
            .y
            .saturating_sub(top)
            .saturating_sub(gap.saturating_mul(count))
            .checked_div(count)
            .unwrap_or(0_i32);
        (0..count)
            .map(|index| Rect {
                x: layout.title.x,
                y: top.saturating_add(index.saturating_mul(h.saturating_add(gap))),
                w: layout.title.w,
                h,
            })
            .collect()
    }

    pub fn new(layout: &Layout) -> Self {
        let gap = (i32::from(layout.height) / 40_i32).max(5_i32);
        let x = layout.title.x;
        let y = layout.title.h.saturating_add(gap);
        let w = layout.title.w;
        let h = layout
            .footer
            .y
            .saturating_sub(y)
            .saturating_sub(gap.saturating_mul(3_i32))
            / 3_i32;
        let row = |index: i32| Rect {
            x,
            y: y.saturating_add(index.saturating_mul(h.saturating_add(gap))),
            w,
            h,
        };
        let first = row(0_i32);
        let second = row(1_i32);
        let bottom = row(2_i32);
        let half = w.saturating_sub(gap) / 2_i32;
        let small = w
            .saturating_sub(half)
            .saturating_sub(gap.saturating_mul(2_i32))
            / 2_i32;
        let controls = [
            first,
            second,
            Rect { w: half, ..bottom },
            Rect {
                x: x.saturating_add(half).saturating_add(gap),
                w: small,
                ..bottom
            },
            Rect {
                x: x.saturating_add(half)
                    .saturating_add(gap.saturating_mul(2))
                    .saturating_add(small),
                w: small,
                ..bottom
            },
        ];
        let tracks = [first, second].map(|r| Rect {
            x: r.x.saturating_add(h),
            y: r.y.saturating_add(h.saturating_mul(3) / 4),
            w: r.w
                .saturating_sub(h)
                .saturating_sub(64_i32.saturating_mul(layout.text_scale)),
            h: 4_i32.saturating_mul(layout.text_scale).max(4_i32),
        });
        Self {
            controls,
            tracks,
            confirmation: [
                Rect { w: half, ..bottom },
                Rect {
                    x: x.saturating_add(half).saturating_add(gap),
                    w: half,
                    ..bottom
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn home_options_fit_the_screen_without_overlapping_the_chrome() -> Result<(), String> {
        for (width, height) in [(320, 200), (480, 272), (800, 480), (1280, 720)] {
            let layout = Layout::home(width, height)?;
            let home = PanelLayout::home(&layout);
            for (index, bounds) in home.iter().enumerate() {
                assert!(bounds.w > 0_i32 && bounds.h > 0_i32, "option {index}");
                assert!(bounds.x >= 0_i32 && bounds.x.saturating_add(bounds.w) <= i32::from(width));
                assert!(bounds.y >= layout.title.h);
                assert!(bounds.y.saturating_add(bounds.h) <= layout.footer.y);
            }
            // Options never overlap each other.
            for (index, first) in home.iter().enumerate() {
                for second in home.iter().skip(index.saturating_add(1)) {
                    assert!(
                        first.x.saturating_add(first.w) <= second.x
                            || second.x.saturating_add(second.w) <= first.x
                            || first.y.saturating_add(first.h) <= second.y
                            || second.y.saturating_add(second.h) <= first.y,
                        "options overlap at {width}x{height}"
                    );
                }
            }
        }
        Ok(())
    }
}
