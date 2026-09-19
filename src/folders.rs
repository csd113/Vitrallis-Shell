//! Local user organization, independent of manifests, receipts and app data.
use crate::{
    app::{AppEntry, AppManifest, AppSource},
    app_center::storage::{self, FileData},
};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};

pub const PREFIX: &str = "vitrallis-folder-";
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folders {
    pub names: BTreeMap<String, String>,
    pub members: BTreeMap<String, String>,
}
#[derive(Debug)]
pub enum Change {
    Create(String),
    Rename(String, String),
    Delete(String),
    Move(String, Option<String>),
}
impl Folders {
    fn path() -> Result<PathBuf, String> {
        Ok(crate::app_center::storage::Locations::current()?
            .data
            .join("vitrallis/folders.json"))
    }
    pub fn load() -> Result<Self, String> {
        Self::read(&Self::path()?)
    }
    fn read(path: &std::path::Path) -> Result<Self, String> {
        let Some(file) = storage::read(path, 512 * 1024)? else {
            return Ok(Self::default());
        };
        let value = crate::app_center::metadata::json(&file.bytes)?;
        crate::app_center::metadata::fields(&value, "folders members")?;
        let decode = |key: &str| -> Result<BTreeMap<String, String>, String> {
            value[key]
                .as_object()
                .filter(|m| m.len() <= 1000)
                .ok_or("Invalid folder state")?
                .iter()
                .map(|(k, v)| {
                    Ok((
                        k.clone(),
                        v.as_str().ok_or("Invalid folder value")?.to_owned(),
                    ))
                })
                .collect()
        };
        let state = Self {
            names: decode("folders")?,
            members: decode("members")?,
        };
        state.validate()?;
        Ok(state)
    }
    fn validate(&self) -> Result<(), String> {
        if self.names.len() > 1000 || self.members.len() > 1000 {
            return Err("Folder limit reached".into());
        }
        for (id, name) in &self.names {
            crate::app_center::metadata::hex(
                id.strip_prefix(PREFIX).ok_or("Invalid folder ID")?,
                64,
            )?;
            Self::name(name)?;
        }
        for (app, folder) in &self.members {
            if app.is_empty()
                || app.len() > 256
                || !app
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                || app.starts_with(PREFIX)
                || !self.names.contains_key(folder)
            {
                return Err("Invalid folder membership".into());
            }
        }
        Ok(())
    }
    fn name(name: &str) -> Result<(), String> {
        if name.trim().is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control)
        {
            return Err("Folder name must contain 1–64 printable characters".into());
        }
        Ok(())
    }
    pub fn change(change: Change) -> Result<Self, String> {
        Self::write_change(&Self::path()?, change)
    }
    fn write_change(path: &std::path::Path, change: Change) -> Result<Self, String> {
        let _lock = storage::Lock::take(&path.with_extension("lock"))?;
        let mut state = Self::read(path)?;
        state.apply(change)?;
        state.validate()?;
        let bytes = serde_json::to_vec(&json!({"folders": state.names, "members": state.members}))
            .map_err(|e| e.to_string())?;
        storage::atomic(path, &FileData { bytes, mode: 0o600 })?;
        Ok(state)
    }
    fn apply(&mut self, change: Change) -> Result<(), String> {
        match change {
            Change::Create(name) => {
                Self::name(&name)?;
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?;
                let id = format!(
                    "{PREFIX}{}",
                    storage::sha(format!("{}:{stamp:?}", std::process::id()).as_bytes())
                );
                if self.names.contains_key(&id) {
                    return Err("Folder ID collision".into());
                }
                self.names.insert(id, name);
            }
            Change::Rename(id, name) => {
                Self::name(&name)?;
                *self.names.get_mut(&id).ok_or("Folder no longer exists")? = name;
            }
            Change::Delete(id) => {
                self.names.remove(&id).ok_or("Folder no longer exists")?;
                self.members.retain(|_, folder| folder != &id);
            }
            Change::Move(app, folder) => {
                if let Some(folder) = folder {
                    if !self.names.contains_key(&folder) {
                        return Err("Folder no longer exists".into());
                    }
                    self.members.insert(app, folder);
                } else {
                    self.members.remove(&app);
                }
            }
        }
        Ok(())
    }
    pub fn view(&self, apps: &[AppEntry], folder: Option<&str>) -> Vec<AppEntry> {
        let mut visible = Vec::new();
        if folder.is_none() {
            visible.extend(self.names.iter().map(|(id, name)| AppEntry {
                id: id.clone(),
                name: format!("[+] {name}"),
                source: AppSource::Folder,
                icon: None,
                manifest: AppManifest {
                    entry: "/vitrallis/builtin/folder".into(),
                    ..AppManifest::default()
                },
                unavailable: None,
            }));
        }
        visible.extend(
            apps.iter()
                .filter(|app| self.members.get(&app.id).map(String::as_str) == folder)
                .cloned(),
        );
        visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folders_persist_moves_and_delete_returns_apps_without_touching_installations()
    -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let path = scratch
            .0
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("folders.json");
        let mut state = Folders::write_change(&path, Change::Create("Tools".into()))?;
        let id = state.names.keys().next().ok_or("folder missing")?.clone();
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        state = Folders::write_change(&path, Change::Move(apps[0].id.clone(), Some(id.clone())))?;
        assert_eq!(state.view(&apps, Some(&id)), apps[..1]);
        assert!(
            !state
                .view(&apps, None)
                .iter()
                .any(|app| app.id == apps[0].id)
        );
        state = Folders::write_change(&path, Change::Rename(id.clone(), "Utilities".into()))?;
        assert_eq!(Folders::read(&path)?, state);
        state = Folders::write_change(&path, Change::Delete(id))?;
        assert_eq!(state.view(&apps, None), apps);
        let before = std::fs::read(&path).map_err(|e| e.to_string())?;
        assert!(Folders::write_change(&path, Change::Create("\n".into())).is_err());
        assert_eq!(before, std::fs::read(&path).map_err(|e| e.to_string())?);
        Ok(())
    }
}
