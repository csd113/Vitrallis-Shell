use super::Platform;
use crate::app::{AppEntry, AppManifest};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct Generic;
impl Platform for Generic {
    fn fullscreen(&self) -> bool {
        false
    }
    fn resolution(&self) -> (u16, u16) {
        (800, 480)
    }
}
pub fn demo_apps(executable: &Path) -> Vec<AppEntry> {
    [
        ("demo", "Demo App", "ok"),
        ("brief", "Quick Return", "quick"),
        ("failure", "Exit Error", "fail"),
        ("missing", "Missing App", "ok"),
        ("notes", "Notes Demo", "ok"),
        ("tools", "Tools Demo", "ok"),
    ]
    .into_iter()
    .map(|(id, name, mode)| AppEntry {
        id: id.into(),
        name: name.into(),
        icon: None,
        unavailable: None,
        manifest: AppManifest {
            entry: if id == "missing" {
                executable.join("missing-app")
            } else {
                executable.to_path_buf()
            },
            args: vec!["--demo-child".into(), mode.into()],
            ..AppManifest::default()
        },
    })
    .collect()
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct Mock;
#[cfg(test)]
impl Platform for Mock {
    fn fullscreen(&self) -> bool {
        false
    }
    fn resolution(&self) -> (u16, u16) {
        (480, 272)
    }
}

impl super::system::System for Generic {
    fn refresh(&mut self) -> super::system::Status {
        super::system::Status {
            clock: super::command::clock(),
            ..super::system::Status::default()
        }
    }
    fn control(
        &mut self,
        _: super::system::Control,
        _: &mut super::system::Status,
    ) -> Result<(), String> {
        Err("hardware controls unavailable on generic platform".into())
    }
}
#[cfg(test)]
impl super::system::System for Mock {
    fn refresh(&mut self) -> super::system::Status {
        super::system::Status::default()
    }
    fn control(
        &mut self,
        _: super::system::Control,
        _: &mut super::system::Status,
    ) -> Result<(), String> {
        Err("unsupported".into())
    }
}
