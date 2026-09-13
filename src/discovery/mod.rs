//! Discovery backends return normalized entries without touching the session or
//! modifying source metadata. App Center supplies installed manifest packages.
mod catalog;
pub mod executable;
mod pockethome;
use crate::{
    app::AppEntry,
    config::{Config, Paths},
    platform::Platform,
};

#[derive(Debug, Default)]
pub struct Catalog {
    pub preferences: crate::preferences::Preferences,
    pub apps: Vec<AppEntry>,
    pub diagnostics: Vec<String>,
}
pub trait Discovery {
    fn discover(&self) -> Result<Catalog, String>;
}

pub fn load(config: &Config) -> Result<Catalog, String> {
    load_with_policy(config, true)
}

pub fn refresh(config: &Config) -> Result<Catalog, String> {
    load_with_policy(config, false)
}

fn load_with_policy(config: &Config, tolerate_invalid: bool) -> Result<Catalog, String> {
    if config.demo || config.mode == crate::config::Mode::Smoke {
        let mut catalog = Catalog {
            apps: crate::platform::generic::demo_apps(
                &std::env::current_exe().map_err(|e| e.to_string())?,
            ),
            diagnostics: vec![],
            preferences: crate::preferences::Preferences::default(),
        };
        crate::shortcuts::integrate(&mut catalog);
        return Ok(catalog);
    }
    let paths = Paths::from_config(config)?;
    eprintln!(
        "level=info event=discovery_paths config={:?} assets={:?}",
        paths.explicit_catalog, paths.asset_roots
    );
    let backend = catalog::CatalogFile { paths: &paths };
    // A broken catalog leaves a usable empty launcher, with a visible diagnostic.
    let mut catalog = match if config.linux_handheld || config.catalog_path.is_some() {
        backend.discover()
    } else {
        Ok(Catalog::default())
    } {
        Ok(catalog) => catalog,
        Err(error) if tolerate_invalid => Catalog {
            apps: vec![],
            diagnostics: vec![error],
            preferences: crate::preferences::Preferences::default(),
        },
        Err(error) => return Err(error),
    };
    if config.linux_handheld
        && let Some(home) = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
    {
        crate::platform::linux_handheld::recovery::integrate(&mut catalog, &home);
    }
    crate::app_center::integrate(&mut catalog);
    crate::native::integrate(&mut catalog)?;
    crate::shortcuts::integrate(&mut catalog);
    if config.linux_handheld {
        for app in &mut catalog.apps {
            crate::platform::linux_handheld::LinuxHandheld.prepare_app(app);
        }
    }
    for diagnostic in &catalog.diagnostics {
        eprintln!("level=warn event=discovery message={diagnostic:?}");
    }
    Ok(catalog)
}

pub fn print(catalog: &Catalog) {
    let apps: Vec<_> = catalog.apps.iter().map(|app| serde_json::json!({
        "id": app.id, "name": app.name, "icon": app.icon, "source": format!("{:?}",app.source),
        "runtime": app.manifest.runtime, "entry": app.manifest.entry,
        "args": app.manifest.args.iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>(),
        "cwd": app.manifest.cwd, "environment_keys": app.manifest.env.keys().map(|s| s.to_string_lossy()).collect::<Vec<_>>(),
        "unavailable": app.unavailable,
    })).collect();
    println!(
        "{}",
        serde_json::json!({"apps": apps, "diagnostics": catalog.diagnostics})
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppSource;

    #[test]
    fn stock_catalog_refreshes_keep_natives_once_and_preserve_custom_apps()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let config_path = scratch.0.join("config.json");
        let config = Config {
            catalog_path: Some(config_path.clone()),
            ..Config::default()
        };
        for fixture in [
            include_str!("../../tests/fixtures/pockethome/stock.json"),
            include_str!("../../tests/fixtures/pockethome/current.json"),
        ] {
            let mut root: serde_json::Value = serde_json::from_str(fixture)?;
            let items = root["pages"][0]["items"].as_array_mut().ok_or("items")?;
            // Labels/icons may be localized or replaced without restoring stock utilities.
            for (index, item) in items.iter_mut().enumerate() {
                item["name"] = format!("Traduit 日本語 {index}").into();
                item["icon"] = "custom.png".into();
            }
            for name in ["Terminal", "Files", "Notepad"] {
                items.push(serde_json::json!({"name":name,"shell":"/bin/sh -c true","icon":""}));
            }
            items.push(serde_json::json!({"name":"Editor with document","shell":"/usr/bin/leafpad note.txt","icon":""}));
            std::fs::write(&config_path, serde_json::to_vec(&root)?)?;
            let original = std::fs::read(&config_path)?;
            let first = load(&config)?;
            let ids: Vec<_> = first.apps.iter().map(|app| app.id.clone()).collect();
            for _ in 0..4 {
                let mut catalog = refresh(&config)?;
                // Idempotence of native integration also protects repeated reload callers.
                crate::native::integrate(&mut catalog)?;
                assert_eq!(
                    catalog
                        .apps
                        .iter()
                        .map(|a| a.id.clone())
                        .collect::<Vec<_>>(),
                    ids
                );
                for native in vitrallis_native::APPLICATIONS {
                    assert_eq!(catalog.apps.iter().filter(|a| a.id == native.id).count(), 1);
                    assert_eq!(
                        catalog
                            .apps
                            .iter()
                            .filter(|a| a.source == AppSource::PocketHome && a.name == native.name)
                            .count(),
                        1
                    );
                }
                let imported: Vec<_> = catalog
                    .apps
                    .iter()
                    .filter(|a| a.source == AppSource::PocketHome)
                    .collect();
                assert!(imported.iter().any(|a| a.name == "Traduit 日本語 1"));
                assert!(imported.iter().any(|a| a.name == "Traduit 日本語 2"));
                assert!(imported.iter().any(|a| a.name == "Traduit 日本語 3"));
                for index in [0, 4, 5] {
                    assert!(
                        !imported
                            .iter()
                            .any(|a| a.name == format!("Traduit 日本語 {index}"))
                    );
                }
                assert!(imported.iter().any(|a| a.name == "Editor with document"));
                assert!(catalog.apps.iter().any(|a| a.id == "vitrallis-app-center"));
            }
            assert_eq!(std::fs::read(&config_path)?, original);
        }
        Ok(())
    }
}
