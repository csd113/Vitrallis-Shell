//! Discovery backends return normalized entries without touching the session or
//! modifying source metadata. A native package directory can implement this trait.
mod catalog;
mod executable;
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
        return Ok(Catalog {
            apps: crate::platform::generic::demo_apps(
                &std::env::current_exe().map_err(|e| e.to_string())?,
            ),
            diagnostics: vec![],
            preferences: crate::preferences::Preferences::default(),
        });
    }
    let paths = Paths::from_config(config)?;
    eprintln!(
        "level=info event=discovery_paths config={:?} assets={:?} reserved_native_apps={:?}",
        paths.user_config, paths.asset_roots, paths.native_apps
    );
    let backend = catalog::CatalogFile {
        paths: &paths,
        explicit_config: config.catalog_path.is_some(),
    };
    // A broken catalog leaves a usable empty launcher, with a visible diagnostic.
    let mut catalog = match backend.discover() {
        Ok(catalog) => catalog,
        Err(error) if tolerate_invalid => Catalog {
            apps: vec![],
            diagnostics: vec![error],
            preferences: crate::preferences::Preferences::default(),
        },
        Err(error) => return Err(error),
    };
    if config.pocketchip
        && let Some(home) = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
    {
        crate::platform::pocketchip::store::integrate(&mut catalog, &home);
    }
    if config.pocketchip {
        for app in &mut catalog.apps {
            crate::platform::pocketchip::PocketChip.prepare_app(app);
        }
    }
    for diagnostic in &catalog.diagnostics {
        eprintln!("level=warn event=discovery message={diagnostic:?}");
    }
    Ok(catalog)
}

pub fn print(catalog: &Catalog) {
    let apps: Vec<_> = catalog.apps.iter().map(|app| serde_json::json!({
        "id": app.id, "name": app.name, "icon": app.icon,
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
