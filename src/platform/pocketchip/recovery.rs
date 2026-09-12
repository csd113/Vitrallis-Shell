//! Preserve the Marshmallow recovery tile; App Center is a native service.
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
            id: "vitrallis-return-marshmallow".into(),
            name: "Marshmallow".into(),
            icon: None,
            manifest: AppManifest {
                entry: "/usr/bin/systemctl".into(),
                args: ["--user", "stop", "vitrallis-session.service"]
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                ..AppManifest::default()
            },
            unavailable: None,
        });
    }
}
