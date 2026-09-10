pub mod generic;
pub mod pocketchip;

/// Session policy is deliberately local: no WM commands or hardware probes.
pub trait Platform {
    fn fullscreen(&self) -> bool;
    fn resolution(&self) -> (u16, u16);
    fn raise_after_exit(&self) -> bool {
        true
    }
}
