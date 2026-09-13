//! Per-user desktop entries. An owned icon lives in the same atomic record as
//! its shortcut, so edits/removal cannot leave a half-updated record/icon pair.
pub mod command;
pub mod screen;
use crate::{
    app::{AppEntry, AppManifest, AppSource},
    app_center::storage::{self, FileData},
    discovery::Catalog,
};
use command::Mode;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const PREFIX: &str = "vitrallis-shortcut-";
const RECORD_LIMIT: usize = 5 * 1024 * 1024;
const ICON_LIMIT: usize = 1024 * 1024;
const COUNT_LIMIT: usize = 1000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub mode: Mode,
    pub terminal: bool,
    pub icon: Option<Vec<u8>>,
}

impl Draft {
    fn validate(&self) -> Result<(), String> {
        for (label, value, limit, required) in [
            ("Name", self.name.as_str(), 100, true),
            ("Command", self.command.as_str(), 8192, true),
            ("Working directory", self.cwd.as_str(), 4096, false),
        ] {
            if value.len() > limit
                || value.chars().any(char::is_control)
                || (required && value.trim().is_empty())
            {
                return Err(format!(
                    "{label} is empty, too long or contains control characters"
                ));
            }
        }
        if !self.cwd.is_empty() && !Path::new(&self.cwd).is_absolute() {
            return Err("Working directory must be absolute".into());
        }
        if self.mode == Mode::Direct {
            command::parse(&self.command)?;
        }
        if self
            .icon
            .as_ref()
            .is_some_and(|bytes| bytes.len() > ICON_LIMIT)
        {
            return Err("Icon exceeds 1 MiB".into());
        }
        Ok(())
    }

    pub fn manifest(&self) -> Result<AppManifest, String> {
        self.validate()?;
        let cwd = if self.cwd.is_empty() {
            user_home()?
        } else {
            PathBuf::from(&self.cwd)
        };
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut manifest = command::manifest(&self.command, self.mode, &cwd, &path)?;
        if self.terminal {
            let executable = std::env::current_exe().map_err(|e| e.to_string())?;
            let terminal = executable
                .parent()
                .ok_or("Cannot locate native terminal")?
                .join("vitrallis-terminal");
            let mut args = vec!["--command".into(), manifest.entry.into_os_string()];
            args.append(&mut manifest.args);
            manifest.entry = terminal;
            manifest.args = args;
        }
        Ok(manifest)
    }

    pub fn choose_icon(&mut self, path: &Path) -> Result<(), String> {
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let file = storage::read(&path, ICON_LIMIT)?.ok_or("Icon file is missing")?;
        crate::renderer::decode_icon(&file.bytes)?;
        self.icon = Some(file.bytes);
        Ok(())
    }
}

fn user_home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| "HOME must be absolute".into())
}

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    #[cfg(test)]
    pub const fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn current() -> Result<Self, String> {
        let data = std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map_or_else(
                || user_home().map(|p| p.join(".local/share")),
                |p| Ok(PathBuf::from(p)),
            )?;
        let root = data.join("vitrallis/shortcuts");
        storage::safe(&root)?;
        Ok(Self { root })
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        let suffix = id.strip_prefix(PREFIX).ok_or("Invalid shortcut identity")?;
        if suffix.len() != 64
            || !suffix
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err("Invalid shortcut identity".into());
        }
        Ok(self.root.join(format!("{id}.json")))
    }

    pub fn load(&self, id: &str) -> Result<Draft, String> {
        let record =
            storage::read(&self.path(id)?, RECORD_LIMIT)?.ok_or("Shortcut no longer exists")?;
        let value: Value = serde_json::from_slice(&record.bytes).map_err(|e| e.to_string())?;
        if value["kind"] != "custom-shortcut" || value["id"] != id {
            return Err("Invalid shortcut provenance or identity".into());
        }
        let text = |key: &str| {
            value[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("Invalid {key}"))
        };
        let flag = |key: &str| value[key].as_bool().ok_or_else(|| format!("Invalid {key}"));
        let draft = Draft {
            name: text("name")?,
            command: text("command")?,
            cwd: text("cwd")?,
            mode: match value["mode"].as_str() {
                Some("direct") => Mode::Direct,
                Some("shell") => Mode::Shell,
                _ => return Err("Invalid command mode".into()),
            },
            terminal: flag("terminal")?,
            icon: if value["icon"].is_null() {
                None
            } else {
                let bytes = value["icon"]
                    .as_array()
                    .filter(|v| v.len() <= ICON_LIMIT)
                    .ok_or("Invalid icon")?;
                Some(
                    bytes
                        .iter()
                        .map(|v| {
                            v.as_u64()
                                .and_then(|v| u8::try_from(v).ok())
                                .ok_or("Invalid icon byte")
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            },
        };
        draft.validate()?;
        Ok(draft)
    }

    pub fn save(&self, id: Option<&str>, draft: &Draft) -> Result<String, String> {
        draft.manifest()?; // Validation only; never creates a process.
        if let Some(id) = id {
            self.path(id)?;
        }
        if let Some(bytes) = &draft.icon {
            crate::renderer::decode_icon(bytes)?;
        }
        let _lock = storage::Lock::take(&self.root)?;
        let id = if let Some(id) = id {
            self.load(id)?;
            id.to_owned()
        } else {
            if self.ids()?.len() >= COUNT_LIMIT {
                return Err("Shortcut limit reached (1000)".into());
            }
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?;
            let id = format!(
                "{PREFIX}{}",
                storage::sha(format!("{}:{}", std::process::id(), stamp.as_nanos()).as_bytes())
            );
            if self.path(&id)?.try_exists().map_err(|e| e.to_string())? {
                return Err("Shortcut ID collision; try again".into());
            }
            id
        };
        let bytes = serde_json::to_vec(&json!({
            "kind": "custom-shortcut", "id": id, "name": draft.name,
            "command": draft.command, "cwd": draft.cwd,
            "mode": match draft.mode { Mode::Direct => "direct", Mode::Shell => "shell" },
            "terminal": draft.terminal, "icon": draft.icon,
        }))
        .map_err(|e| e.to_string())?;
        if bytes.len() > RECORD_LIMIT {
            return Err("Shortcut record is too large".into());
        }
        storage::atomic(&self.path(&id)?, &FileData { bytes, mode: 0o600 })?;
        Ok(id)
    }

    /// The only deleted file is derived from a validated custom ID, never a
    /// command, icon source path, installed package ID or field in the record.
    pub fn remove(&self, app: &AppEntry) -> Result<(), String> {
        if app.source != AppSource::Custom {
            return Err("Only custom shortcuts can be removed from shortcut storage".into());
        }
        let path = self.path(&app.id)?;
        let _lock = storage::Lock::take(&self.root)?;
        storage::read(&path, RECORD_LIMIT)?.ok_or("Shortcut no longer exists")?;
        fs::remove_file(path).map_err(|e| e.to_string())?;
        storage::sync(&self.root)
    }

    fn ids(&self) -> Result<Vec<String>, String> {
        storage::safe(&self.root)?;
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        let mut ids = Vec::new();
        for item in entries.take(COUNT_LIMIT * 2) {
            let item = item.map_err(|e| e.to_string())?;
            if let Some(id) = item.path().file_stem().and_then(|s| s.to_str())
                && item.path().extension().is_some_and(|ext| ext == "json")
                && self.path(id).is_ok()
            {
                ids.push(id.to_owned());
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn integrate(&self, catalog: &mut Catalog) -> Result<(), String> {
        // Custom IDs are reserved; imported entries cannot acquire this identity.
        catalog.apps.retain(|app| !app.id.starts_with(PREFIX));
        for id in self.ids()?.into_iter().take(COUNT_LIMIT) {
            match self.load(&id) {
                Ok(draft) => {
                    let (manifest, unavailable) = match draft.manifest() {
                        Ok(manifest) => (manifest, None),
                        Err(error) => (
                            AppManifest {
                                entry: "/vitrallis/unavailable-shortcut".into(),
                                ..AppManifest::default()
                            },
                            Some(error),
                        ),
                    };
                    catalog.apps.push(AppEntry {
                        id,
                        source: AppSource::Custom,
                        name: draft.name,
                        icon: None,
                        manifest,
                        unavailable,
                    });
                }
                Err(error) => catalog.diagnostics.push(format!("Shortcut {id}: {error}")),
            }
        }
        let hidden = self.hidden()?;
        catalog
            .apps
            .retain(|app| hidden_key(app).is_none_or(|key| !hidden.contains(&key)));
        Ok(())
    }

    fn hidden(&self) -> Result<Vec<String>, String> {
        let Some(file) = storage::read(&self.root.join("hidden.json"), 128 * 1024)? else {
            return Ok(Vec::new());
        };
        let hidden: Vec<String> = serde_json::from_slice(&file.bytes)
            .map_err(|e| format!("Invalid hidden desktop entries: {e}"))?;
        if hidden.len() > COUNT_LIMIT
            || hidden
                .iter()
                .any(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("Invalid hidden desktop entries".into());
        }
        Ok(hidden)
    }

    pub fn hide(&self, app: &AppEntry) -> Result<(), String> {
        let key =
            hidden_key(app).ok_or("This entry cannot be hidden; managed apps require uninstall")?;
        let _lock = storage::Lock::take(&self.root)?;
        let mut hidden = self.hidden()?;
        if !hidden.contains(&key) {
            if hidden.len() >= COUNT_LIMIT {
                return Err("Hidden entry limit reached".into());
            }
            hidden.push(key);
        }
        let bytes = serde_json::to_vec(&hidden).map_err(|e| e.to_string())?;
        storage::atomic(
            &self.root.join("hidden.json"),
            &FileData { bytes, mode: 0o600 },
        )
    }
}

pub fn hidden_key(app: &AppEntry) -> Option<String> {
    if matches!(app.source, AppSource::Custom | AppSource::AppCenter)
        || app.id == crate::app_center::TILE_ID
        || app.is_system_settings()
    {
        return None;
    }
    Some(storage::sha(
        format!("{:?}:{}", app.source, app.id).as_bytes(),
    ))
}

pub fn integrate(catalog: &mut Catalog) {
    if let Err(error) = Store::current().and_then(|store| store.integrate(catalog)) {
        catalog.diagnostics.push(error);
    }
}

pub fn icon(app: &AppEntry) -> Option<Vec<u8>> {
    if app.source != AppSource::Custom {
        return None;
    }
    Store::current()
        .and_then(|store| store.load(&app.id))
        .ok()?
        .icon
}

#[cfg(test)]
mod tests;
