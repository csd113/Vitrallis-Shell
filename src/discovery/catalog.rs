//! Read a bounded catalog file using the configured source precedence.
use super::{Catalog, Discovery, pockethome};
use crate::config::Paths;
use std::io::Read;

pub struct CatalogFile<'a> {
    pub paths: &'a Paths,
}
impl Discovery for CatalogFile<'_> {
    fn discover(&self) -> Result<Catalog, String> {
        let path = self
            .paths
            .explicit_catalog
            .clone()
            .unwrap_or_else(|| self.paths.asset("config.json"));
        eprintln!(
            "level=info event=discovery_source path={:?}",
            path.to_string_lossy()
        );
        if !std::fs::metadata(&path)
            .map_err(|e| format!("config {}: {e}", path.display()))?
            .is_file()
        {
            return Err("app config must be a regular file".into());
        }
        let file =
            std::fs::File::open(&path).map_err(|e| format!("config {}: {e}", path.display()))?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("app config must be a regular file".into());
        }
        let mut text = String::new();
        file.take(1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(|e| format!("config {}: {e}", path.display()))?;
        if text.len() > 1024 * 1024 {
            return Err("app config exceeds 1 MiB".into());
        }
        pockethome::parse_catalog(&text, self.paths)
            .map_err(|e| format!("config {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_catalog_overrides_system_without_merging_or_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let root = &scratch.0;
        let mut paths = Paths {
            explicit_catalog: None,
            asset_roots: vec![],
            cwd: "/".into(),
            search_path: vec!["/bin".into(), "/usr/bin".into()],
        };
        paths.asset_roots = vec![root.clone()];
        let default =
            br#"{"pages":[{"name":"Apps","items":[{"name":"Default","shell":"sh","icon":""}]}]}"#;
        std::fs::write(root.join("config.json"), default)?;
        let backend = CatalogFile { paths: &paths };
        assert_eq!(backend.discover()?.apps[0].name, "Default");
        assert!(!root.join("user.json").exists());
        paths.explicit_catalog = Some(root.join("user.json"));
        let backend = CatalogFile { paths: &paths };
        let user = br#"{"pages":[{"name":"Apps","items":[]}]}"#;
        std::fs::write(root.join("user.json"), user)?;
        assert!(backend.discover()?.apps.is_empty());
        assert_eq!(std::fs::read(root.join("user.json"))?, user);
        assert_eq!(std::fs::read(root.join("config.json"))?, default);
        std::fs::write(root.join("user.json"), "broken")?;
        assert!(backend.discover().is_err()); // Never silently replace broken user config.
        std::fs::remove_file(root.join("user.json"))?;
        assert!(CatalogFile { paths: &paths }.discover().is_err());
        Ok(())
    }
}
