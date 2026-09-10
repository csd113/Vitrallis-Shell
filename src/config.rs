#[derive(Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Launch,
    List,
    Smoke,
}
#[derive(Debug, Default)]
pub struct Config {
    pub pocketchip: bool,
    pub catalog_path: Option<std::path::PathBuf>,
    pub assets: Option<std::path::PathBuf>,
    pub mode: Mode,
    pub demo: bool,
    pub size: Option<(u16, u16)>,
    pub screenshot: Option<std::path::PathBuf>,
}
impl Config {
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();
        let mut args = args;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--app-config" => {
                    config.catalog_path =
                        Some(args.next().ok_or("--app-config requires a path")?.into());
                }
                "--assets" => {
                    config.assets =
                        Some(args.next().ok_or("--assets requires a directory")?.into());
                }
                "--list-apps" => config.mode = Mode::List,
                "--demo" => config.demo = true,
                "--pocketchip" => config.pocketchip = true,
                "--smoke-test" => config.mode = Mode::Smoke,
                "--size" => {
                    let value = args.next().ok_or("--size requires WIDTHxHEIGHT")?;
                    let (w, h) = value
                        .split_once('x')
                        .ok_or("--size requires WIDTHxHEIGHT")?;
                    config.size = Some((
                        w.parse().map_err(|_| "invalid width")?,
                        h.parse().map_err(|_| "invalid height")?,
                    ));
                }
                "--screenshot" => {
                    config.screenshot = Some(
                        args.next()
                            .ok_or("--screenshot requires a new BMP path")?
                            .into(),
                    );
                }
                _ => return Err(format!("unknown argument: {arg}")),
            }
        }
        if config.pocketchip && config.mode == crate::config::Mode::Smoke {
            return Err("smoke test is desktop-only".into());
        }
        if config.mode == crate::config::Mode::Smoke && config.screenshot.is_some() {
            return Err("smoke and screenshot modes are separate checks".into());
        }
        Ok(config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_arguments() {
        for args in [
            vec!["--size"],
            vec!["--size", "junk"],
            vec!["--size", "999999x200"],
            vec!["--wat"],
            vec!["--pocketchip", "--smoke-test"],
        ] {
            assert!(Config::parse(args.into_iter().map(str::to_owned)).is_err());
        }
    }
}

/// All filesystem conventions live here. Discovery only reads these locations.
#[derive(Debug, Clone)]
pub struct Paths {
    pub user_config: Option<std::path::PathBuf>,
    pub asset_roots: Vec<std::path::PathBuf>,
    pub native_apps: Option<std::path::PathBuf>,
    pub cwd: std::path::PathBuf,
    pub search_path: Vec<std::path::PathBuf>,
}
impl Paths {
    pub fn from_config(config: &Config) -> Result<Self, String> {
        let cwd = std::env::current_dir().map_err(|e| format!("current directory: {e}"))?;
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute());
        let absolute = |p: &std::path::PathBuf| {
            if p.is_absolute() {
                p.clone()
            } else {
                cwd.join(p)
            }
        };
        let user_config = config
            .catalog_path
            .as_ref()
            .map(absolute)
            .or_else(|| home.as_ref().map(|p| p.join(".pocket-home/config.json")));
        let asset_roots = config.assets.as_ref().map_or_else(
            || {
                vec![
                    "/usr/share/pocket-home".into(),
                    cwd.join("../../assets"),
                    cwd.clone(),
                ]
            },
            |p| vec![absolute(p)],
        );
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.map(|p| p.join(".local/share")));
        let native_apps = data.map(|p| p.join("vitrallis/apps"));
        // Preserve execvp's PATH order, including relative/empty entries, but resolve
        // against the inherited cwd before constructing a child command.
        let search_path = std::env::var_os("PATH").map_or_else(
            || vec!["/bin".into(), "/usr/bin".into()],
            |p| std::env::split_paths(&p).map(|p| absolute(&p)).collect(),
        );
        Ok(Self {
            user_config,
            asset_roots,
            native_apps,
            cwd,
            search_path,
        })
    }
    pub fn asset(&self, name: &str) -> std::path::PathBuf {
        let path = std::path::Path::new(name);
        if path.is_absolute() {
            return path.to_path_buf();
        }
        self.asset_roots
            .iter()
            .map(|root| root.join(path))
            .find(|p| p.exists())
            .unwrap_or_else(|| self.cwd.join(path))
    }
}

// Optional Unix process-group cleanup helper; no shell or PATH lookup.
#[cfg(unix)]
pub const KILL_HELPER: &str = "/bin/kill";
