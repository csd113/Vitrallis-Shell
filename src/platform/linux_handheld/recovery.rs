//! Exit the owned systemd session and restore the original desktop.
use crate::{
    app::{AppEntry, AppManifest},
    discovery::Catalog,
};
use std::path::Path;

pub fn integrate(catalog: &mut Catalog, home: &Path) {
    if std::env::var_os("VITRALLIS_SESSION").as_deref() == Some(std::ffi::OsStr::new("1")) {
        let launcher = home.join(".local/share/vitrallis/launch");
        catalog.apps.retain(|app| app.manifest.entry != launcher);
        catalog.apps.push(AppEntry {
            source: crate::app::AppSource::System,
            id: "vitrallis-exit-session".into(),
            name: "Exit Vitrallis".into(),
            icon: None,
            manifest: AppManifest {
                entry: "/usr/bin/python3".into(),
                // The helper verifies unit and process ownership before stopping.
                args: vec![
                    home.join(".local/share/vitrallis/vitrallis-session.py")
                        .into_os_string(),
                    "stop".into(),
                ],
                ..AppManifest::default()
            },
            unavailable: None,
        });
    }
}
