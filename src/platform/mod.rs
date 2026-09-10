pub mod generic;
pub mod pocketchip;

mod command;
pub mod system;

/// Session policy and an independently refreshed system backend.
pub trait Platform: system::System + Copy {
    fn fullscreen(&self) -> bool;
    fn resolution(&self) -> (u16, u16);
    fn raise_after_exit(&self) -> bool {
        true
    }
}
