use super::Platform;

#[derive(Debug)]
pub struct PocketChip;
impl Platform for PocketChip {
    fn fullscreen(&self) -> bool {
        true
    }
    fn resolution(&self) -> (u16, u16) {
        (480, 272)
    }
}
