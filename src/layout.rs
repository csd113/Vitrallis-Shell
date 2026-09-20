//! Reusable display dimensions and proportional layout, independent of device
//! support and the keyboard, mouse, or touch events delivered by SDL.

pub const DISPLAY_480X272: (u16, u16) = (480, 272);
pub const DISPLAY_800X480: (u16, u16) = (800, 480);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && y >= f64::from(self.y)
            && x < f64::from(self.x + self.w)
            && y < f64::from(self.y + self.h)
    }
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub width: u16,
    pub height: u16,
    pub columns: usize,
    pub tiles: Vec<Rect>,
    pub icon_size: i32,
    pub text_scale: i32,
    pub title: Rect,
    pub footer: Rect,
    pub previous: Rect,
    pub next: Rect,
    pub folder_back: Rect,
    pub desktop_menu: Rect,
}
impl Layout {
    pub fn home(width: u16, height: u16) -> Result<Self, String> {
        Self::new(width, height, 3, 2)
    }
    pub fn new(width: u16, height: u16, columns: u16, rows: u16) -> Result<Self, String> {
        if !(320..=4096).contains(&width)
            || !(200..=4096).contains(&height)
            || !(1..=6).contains(&columns)
            || !(1..=4).contains(&rows)
        {
            return Err("layout requires 320..4096 x 200..4096 and a 1..6 x 1..4 grid".into());
        }
        let w = i32::from(width);
        let h = i32::from(height);
        let margin = w / 40;
        let top = h / 7;
        let bottom = h / 8;
        let gap = h / 30;
        let cell_w = (w - 2 * margin - (i32::from(columns) - 1) * gap) / i32::from(columns);
        let cell_h = (h - top - bottom - (i32::from(rows) - 1) * gap) / i32::from(rows);
        if cell_w < 48 || cell_h < 48 {
            return Err("grid cells are too small".into());
        }
        let tiles = (0..rows)
            .flat_map(|row| {
                (0..columns).map(move |col| Rect {
                    x: margin + i32::from(col) * (cell_w + gap),
                    y: top + i32::from(row) * (cell_h + gap),
                    w: cell_w,
                    h: cell_h,
                })
            })
            .collect();
        Ok(Self {
            width,
            height,
            columns: usize::from(columns),
            tiles,
            icon_size: (cell_h * 3 / 5).min(cell_w / 2),
            text_scale: vitrallis_native::theme::text_scale(h),
            title: Rect {
                x: margin,
                y: 0,
                w: w - 2 * margin,
                h: top,
            },
            previous: Rect {
                x: margin,
                y: 0,
                w: top,
                h: top,
            },
            next: Rect {
                x: w - margin - top,
                y: 0,
                w: top,
                h: top,
            },
            footer: Rect {
                x: margin,
                y: h - bottom,
                w: w - 2 * margin,
                h: bottom,
            },
            folder_back: Rect {
                x: margin + (w - 2 * margin) / 3,
                y: h - bottom,
                w: (w - 2 * margin) / 3,
                h: bottom,
            },
            desktop_menu: Rect {
                x: margin + (i32::from(columns) - 1) * (cell_w + gap),
                y: h - bottom,
                w: cell_w,
                h: bottom,
            },
        })
    }
    pub fn hit(&self, x: f64, y: f64, count: usize) -> Option<usize> {
        self.tiles.iter().take(count).position(|r| r.contains(x, y))
    }
    #[cfg(test)]
    pub fn touch(&self, x: f32, y: f32, count: usize) -> Option<usize> {
        self.hit(
            f64::from(x) * f64::from(self.width),
            f64::from(y) * f64::from(self.height),
            count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_grids_have_nonoverlapping_hitboxes() -> Result<(), String> {
        for (w, h) in [(480, 272), (800, 480), (1024, 600), (1280, 720)] {
            let grid = Layout::new(w, h, 3, 2)?;
            for (i, tile) in grid.tiles.iter().enumerate() {
                assert_eq!(
                    grid.hit(
                        f64::from(tile.x + tile.w / 2),
                        f64::from(tile.y + tile.h / 2),
                        6
                    ),
                    Some(i)
                );
                assert!(tile.x >= 0 && tile.x + tile.w <= i32::from(w));
                assert!(tile.y >= 0 && tile.y + tile.h <= i32::from(h));
                assert!(!tile.contains(f64::from(tile.x + tile.w), f64::from(tile.y)));
            }
            assert_eq!(grid.hit(-1., 40., 6), None);
            assert_eq!(grid.hit(f64::NAN, 40., 6), None);
            assert_eq!(grid.touch(1., 1., 6), None);
            assert_eq!(grid.touch(0.5, 0.3, 6), Some(1));
            assert_eq!(grid.touch(0.5, 0.3, 1), None);
        }
        assert!(Layout::new(0, 272, 3, 2).is_err());
        assert!(Layout::new(480, 272, 0, 2).is_err());
        Ok(())
    }
}
