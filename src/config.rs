pub use vitrallis_native::renderer::RendererMode;

#[derive(Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Launch,
    List,
    Smoke,
    GraphicsInfo,
    GraphicsTest,
}
#[derive(Debug, Default)]
pub struct Config {
    pub renderer: RendererMode,
    pub linux_handheld: bool,
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
                "--renderer" => {
                    config.renderer = args
                        .next()
                        .ok_or("--renderer requires auto, hardware or software")?
                        .parse()?;
                }
                "--app-config" => {
                    config.catalog_path =
                        Some(args.next().ok_or("--app-config requires a path")?.into());
                }
                "--assets" => {
                    config.assets =
                        Some(args.next().ok_or("--assets requires a directory")?.into());
                }
                "--graphics-info" | "--graphics-test" => {
                    if config.mode != Mode::Launch {
                        return Err("graphics commands cannot be combined with other modes".into());
                    }
                    config.mode = if arg == "--graphics-info" {
                        Mode::GraphicsInfo
                    } else {
                        Mode::GraphicsTest
                    };
                }
                "--list-apps" | "--smoke-test" => {
                    if config.mode != Mode::Launch {
                        return Err("command modes cannot be combined".into());
                    }
                    config.mode = if arg == "--list-apps" {
                        Mode::List
                    } else {
                        Mode::Smoke
                    };
                }
                "--demo" => config.demo = true,
                "--linux-handheld" => config.linux_handheld = true,
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
        if matches!(config.mode, Mode::GraphicsInfo | Mode::GraphicsTest)
            && (config.screenshot.is_some()
                || config.catalog_path.is_some()
                || config.assets.is_some()
                || config.demo
                || config.linux_handheld
                || config.size.is_some())
        {
            return Err("graphics commands accept only --renderer; they use a small diagnostic window and do not load or change user data".into());
        }
        if config.linux_handheld && config.mode == crate::config::Mode::Smoke {
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
            vec!["--graphics-test", "--smoke-test"],
            vec!["--smoke-test", "--graphics-test"],
            vec!["--graphics-info", "--graphics-test"],
            vec!["--graphics-info", "--screenshot", "new.bmp"],
            vec!["--graphics-test", "--app-config", "ignored.json"],
            vec!["--renderer"],
            vec!["--renderer", "invalid"],
            vec!["--renderer", ""],
            vec!["--linux-handheld", "--smoke-test"],
        ] {
            assert!(Config::parse(args.into_iter().map(str::to_owned)).is_err());
        }
    }

    #[test]
    fn renderer_options_use_existing_config_parser() -> Result<(), String> {
        assert_eq!(
            Config::parse(std::iter::empty())?.renderer,
            RendererMode::Auto
        );
        for (value, mode) in [
            ("auto", RendererMode::Auto),
            ("hardware", RendererMode::Hardware),
            ("software", RendererMode::Software),
        ] {
            let config = Config::parse(
                ["--renderer", value, "--demo"]
                    .into_iter()
                    .map(str::to_owned),
            )?;
            assert_eq!(config.renderer, mode);
            assert!(config.demo);
            assert_eq!(mode.as_str(), value);
        }
        Ok(())
    }

    #[test]
    fn display_size_is_independent_of_device_selection() -> Result<(), String> {
        use crate::platform::{Platform, generic::Generic, linux_handheld::LinuxHandheld};

        let desktop = Config::parse(["--size", "480x272"].into_iter().map(str::to_owned))?;
        assert!(!desktop.linux_handheld);
        assert_eq!(desktop.size, Some((480, 272)));
        assert_eq!(Generic.resolution(), (800, 480));
        assert!(!Generic.fullscreen());

        let device = Config::parse(
            ["--linux-handheld", "--size", "800x480"]
                .into_iter()
                .map(str::to_owned),
        )?;
        assert!(device.linux_handheld);
        assert_eq!(device.size, Some((800, 480)));
        assert_eq!(LinuxHandheld.resolution(), (480, 272));
        assert!(LinuxHandheld.fullscreen());

        let smoke = Config::parse(
            ["--size", "480x272", "--smoke-test"]
                .into_iter()
                .map(str::to_owned),
        )?;
        assert!(!smoke.linux_handheld);
        assert_eq!(smoke.mode, Mode::Smoke);
        Ok(())
    }
}

/// All filesystem conventions live here. Discovery only reads these locations.
#[derive(Debug, Clone)]
pub struct Paths {
    pub explicit_catalog: Option<std::path::PathBuf>,
    pub asset_roots: Vec<std::path::PathBuf>,
    pub cwd: std::path::PathBuf,
    pub search_path: Vec<std::path::PathBuf>,
}
impl Paths {
    pub fn from_config(config: &Config) -> Result<Self, String> {
        let cwd = std::env::current_dir().map_err(|e| format!("current directory: {e}"))?;
        let absolute = |p: &std::path::PathBuf| {
            if p.is_absolute() {
                p.clone()
            } else {
                cwd.join(p)
            }
        };
        let explicit_catalog = config.catalog_path.as_ref().map(absolute);
        let asset_roots = config.assets.as_ref().map_or_else(
            || vec!["/usr/share/pocket-home".into(), cwd.clone()],
            |p| vec![absolute(p)],
        );
        // Preserve execvp's PATH order, including relative/empty entries, but resolve
        // against the inherited cwd before constructing a child command.
        let search_path = std::env::var_os("PATH").map_or_else(
            || vec!["/bin".into(), "/usr/bin".into()],
            |p| std::env::split_paths(&p).map(|p| absolute(&p)).collect(),
        );
        Ok(Self {
            explicit_catalog,
            asset_roots,
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
