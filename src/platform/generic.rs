use super::Platform;
use crate::app::App;
use std::path::Path;

#[derive(Debug)]
pub struct Generic;
impl Platform for Generic {
    fn apps(&self) -> Result<Vec<App>, String> {
        std::env::current_exe()
            .map(|path| demo_apps(&path))
            .map_err(|e| format!("cannot locate demo executable: {e}"))
    }
    fn fullscreen(&self) -> bool {
        false
    }
    fn resolution(&self) -> (u16, u16) {
        (800, 480)
    }
}
pub fn demo_apps(executable: &Path) -> Vec<App> {
    [
        ("demo", "Demo App", "ok"),
        ("brief", "Quick Return", "quick"),
        ("failure", "Exit Error", "fail"),
        ("missing", "Missing App", "ok"),
        ("notes", "Notes Demo", "ok"),
        ("tools", "Tools Demo", "ok"),
    ]
    .into_iter()
    .map(|(id, name, mode)| App {
        id: id.into(),
        name: name.into(),
        icon: None,
        executable: if id == "missing" {
            executable.join("missing-app")
        } else {
            executable.to_path_buf()
        },
        args: vec!["--demo-child".into(), mode.into()],
        cwd: None,
    })
    .collect()
}

#[cfg(test)]
#[derive(Debug)]
pub struct Mock;
#[cfg(test)]
impl Platform for Mock {
    fn apps(&self) -> Result<Vec<App>, String> {
        Ok(demo_apps(std::path::Path::new("/mock/vitrallis")))
    }
    fn fullscreen(&self) -> bool {
        false
    }
    fn resolution(&self) -> (u16, u16) {
        (480, 272)
    }
}
