//! Adapt the existing `PocketCHIP` updater; its explicit catalogue remains the
//! authority. No repository-provided commands are interpreted by this adapter.
use super::Catalog;
use crate::app::{AppEntry, AppManifest};
use std::path::Path;

pub fn integrate(catalog: &mut Catalog, home: &Path) {
    if std::env::var_os("VITRALLIS_SESSION").as_deref() == Some(std::ffi::OsStr::new("1")) {
        let launcher = home.join(".local/share/vitrallis/launch");
        catalog.apps.retain(|app| app.manifest.entry != launcher);
        catalog.apps.push(AppEntry {
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
    let root = home.join(".local/share/pocket-update-apps");
    let entry = root.join("launch");
    if let Some(app) = catalog.apps.iter_mut().find(|app| {
        app.manifest.entry == entry
            && app.manifest.runtime.is_none()
            && app.manifest.args.is_empty()
    }) {
        app.name = "App Center".into();
        return;
    }
    let available = entry.is_file() && root.join("update_apps.py").is_file();
    catalog.apps.push(AppEntry {
        id: "vitrallis-pocketchip-store".into(),
        name: "App Center".into(),
        icon: Some(root.join("update-apps.png")),
        manifest: AppManifest {
            entry,
            ..AppManifest::default()
        },
        unavailable: (!available)
            .then(|| "PocketCHIP Update Apps is not installed; see the store setup guide".into()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn store_is_visible_when_missing_and_existing_entry_is_not_duplicated() {
        let mut catalog = Catalog::default();
        integrate(&mut catalog, Path::new("/nonexistent/vitrallis-test"));
        assert_eq!(catalog.apps.len(), 1);
        assert!(catalog.apps[0].unavailable.is_some());
        assert_eq!(catalog.apps[0].name, "App Center");
        integrate(&mut catalog, Path::new("/nonexistent/vitrallis-test"));
        assert_eq!(catalog.apps.len(), 1);
        assert!(catalog.apps[0].manifest.args.is_empty());
    }
}
