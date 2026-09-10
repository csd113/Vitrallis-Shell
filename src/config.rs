#[derive(Debug, Default)]
pub struct Config {
    pub pocketchip: bool,
    pub size: Option<(u16, u16)>,
    pub screenshot: Option<std::path::PathBuf>,
    pub smoke: bool,
}
impl Config {
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();
        let mut args = args;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--pocketchip" => config.pocketchip = true,
                "--smoke-test" => config.smoke = true,
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
        if config.pocketchip && config.smoke {
            return Err("smoke test is desktop-only".into());
        }
        if config.smoke && config.screenshot.is_some() {
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
