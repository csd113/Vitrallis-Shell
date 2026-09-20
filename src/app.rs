use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

/// Runtime-independent launch definition. An absent runtime executes entry directly;
/// a runtime executes entry as its first argument (Python is only one possibility).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppManifest {
    pub tor: crate::tor::Requirement,
    pub runtime: Option<PathBuf>,
    pub entry: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<OsString, OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSource {
    Folder,
    Native,
    AppCenter,
    PocketHome,
    System,
    Demo,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    pub id: String,
    pub source: AppSource,
    pub name: String,
    pub icon: Option<PathBuf>,
    pub manifest: AppManifest,
    pub unavailable: Option<String>,
}

impl AppEntry {
    pub fn is_system_settings(&self) -> bool {
        // Preserve the generated Wi-Fi entry ID so catalogue identity is stable.
        self.id == "vitrallis-wifi-settings"
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        {
            return Err(
                "app id must contain only ASCII letters, digits, dash, dot or underscore".into(),
            );
        }
        if self.name.trim().is_empty() || self.name.chars().any(char::is_control) {
            return Err(format!("app {} has an empty or invalid label", self.id));
        }
        let m = &self.manifest;
        for path in [
            Some(&m.entry),
            m.runtime.as_ref(),
            m.cwd.as_ref(),
            self.icon.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if !path.is_absolute() || path.as_os_str().as_encoded_bytes().contains(&0) {
                return Err(format!(
                    "app {} requires absolute paths without NUL",
                    self.id
                ));
            }
        }
        if m.args.iter().any(|a| a.as_encoded_bytes().contains(&0)) {
            return Err(format!("app {} contains a NUL argument", self.id));
        }
        for (key, value) in &m.env {
            if key.is_empty()
                || key.as_encoded_bytes().iter().any(|b| matches!(b, 0 | b'='))
                || value.as_encoded_bytes().contains(&0)
            {
                return Err(format!(
                    "app {} contains an invalid environment entry",
                    self.id
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_data_is_rejected() {
        let mut app =
            crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis")).remove(0);
        assert!(app.validate().is_ok());
        app.name.clear();
        assert!(app.validate().is_err());
        app.name = "Demo".into();
        app.manifest.entry = "relative".into();
        assert!(app.validate().is_err());
        app.manifest.entry = "/bin/demo".into();
        app.manifest.env.insert("BAD=KEY".into(), "value".into());
        assert!(app.validate().is_err());
        app.manifest.env.clear();
        app.manifest.args.push("bad\0arg".into());
        assert!(app.validate().is_err());
    }
}
