use crate::layout::{Layout, Rect};

pub struct PanelLayout {
    pub controls: [Rect; 5],
    pub tracks: [Rect; 2],
    pub confirmation: [Rect; 2],
}
impl PanelLayout {
    pub fn footer(layout: &Layout) -> [Rect; 3] {
        [0, 1, 2].map(|index| Rect {
            x: layout.footer.x + index * layout.footer.w / 3,
            w: layout.footer.w / 3,
            ..layout.footer
        })
    }

    pub fn rows(layout: &Layout, count: i32) -> Vec<Rect> {
        let gap = (i32::from(layout.height) / 40).max(5);
        let top = layout.title.h + gap;
        let h = (layout.footer.y - top - gap * count) / count;
        (0..count)
            .map(|index| Rect {
                x: layout.title.x,
                y: top + index * (h + gap),
                w: layout.title.w,
                h,
            })
            .collect()
    }

    pub fn new(layout: &Layout) -> Self {
        let gap = (i32::from(layout.height) / 40).max(5);
        let x = layout.title.x;
        let y = layout.title.h + gap;
        let w = layout.title.w;
        let h = (layout.footer.y - y - gap * 3) / 3;
        let row = |index| Rect {
            x,
            y: y + index * (h + gap),
            w,
            h,
        };
        let first = row(0);
        let second = row(1);
        let bottom = row(2);
        let half = (w - gap) / 2;
        let small = (w - half - gap * 2) / 2;
        let controls = [
            first,
            second,
            Rect { w: half, ..bottom },
            Rect {
                x: x + half + gap,
                w: small,
                ..bottom
            },
            Rect {
                x: x + half + gap * 2 + small,
                w: small,
                ..bottom
            },
        ];
        let tracks = [first, second].map(|r| Rect {
            x: r.x + h,
            y: r.y + h * 3 / 4,
            w: r.w - h - 64 * layout.text_scale,
            h: (4 * layout.text_scale).max(4),
        });
        Self {
            controls,
            tracks,
            confirmation: [
                Rect { w: half, ..bottom },
                Rect {
                    x: x + half + gap,
                    w: half,
                    ..bottom
                },
            ],
        }
    }
}
