//! Discovery backends return normalized entries without touching the session or
//! modifying source metadata. A native package directory can implement this trait.
mod marshmallow;
use crate::{
    app::AppEntry,
    config::{Config, Paths},
};

#[derive(Debug, Default)]
pub struct Catalog {
    pub apps: Vec<AppEntry>,
    pub diagnostics: Vec<String>,
}
pub trait Discovery {
    fn discover(&self) -> Result<Catalog, String>;
}

pub fn load(config: &Config) -> Result<Catalog, String> {
    if config.demo || config.mode == crate::config::Mode::Smoke {
        return Ok(Catalog {
            apps: crate::platform::generic::demo_apps(
                &std::env::current_exe().map_err(|e| e.to_string())?,
            ),
            diagnostics: vec![],
        });
    }
    let paths = Paths::from_config(config)?;
    eprintln!(
        "level=info event=discovery_paths config={:?} assets={:?} reserved_native_apps={:?}",
        paths.user_config, paths.asset_roots, paths.native_apps
    );
    let backend = marshmallow::Marshmallow {
        paths: &paths,
        explicit_config: config.catalog_path.is_some(),
    };
    // A broken catalog leaves a usable empty launcher, with a visible diagnostic.
    let catalog = backend.discover().unwrap_or_else(|error| Catalog {
        apps: vec![],
        diagnostics: vec![error],
    });
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
