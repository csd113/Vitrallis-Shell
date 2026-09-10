use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Clone)]
pub struct App {
    pub id: String,
    pub name: String,
    pub icon: Option<PathBuf>,
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
}

impl App {
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
        // Absolute paths make execution independent of an untrusted PATH or cwd.
        if !self.executable.is_absolute() {
            return Err(format!(
                "app {} requires an absolute executable path",
                self.id
            ));
        }
        if self.cwd.as_ref().is_some_and(|p| !p.is_absolute())
            || self.icon.as_ref().is_some_and(|p| !p.is_absolute())
        {
            return Err(format!(
                "app {} requires absolute asset and working paths",
                self.id
            ));
        }
        if self.executable.as_os_str().as_encoded_bytes().contains(&0)
            || self.args.iter().any(|a| a.as_encoded_bytes().contains(&0))
            || self
                .cwd
                .as_ref()
                .is_some_and(|p| p.as_os_str().as_encoded_bytes().contains(&0))
        {
            return Err(format!("app {} contains a NUL in its command", self.id));
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
        app.executable = "relative".into();
        assert!(app.validate().is_err());
        app.executable = "/bin/demo".into();
        app.args.push("bad\0arg".into());
        assert!(app.validate().is_err());
    }
}
