//! An explicit argv boundary; no quoting or shell re-interpretation in terminal mode.
use std::{ffi::OsString, path::PathBuf};
use vitrallis_native::ui::Options;

pub fn parse(mut args: Vec<OsString>) -> Result<(Option<Options>, Vec<OsString>), String> {
    let command = if let Some(index) = args.iter().position(|arg| arg == "--command") {
        let command = args.split_off(index + 1);
        args.pop();
        if command.first().is_none_or(|arg| arg.is_empty()) {
            return Err("Use --command EXECUTABLE [ARG ...]".into());
        }
        command
    } else {
        Vec::new()
    };
    let options = Options::parse_args("vitrallis-terminal", args.into_iter())?;
    Ok((options, command))
}

pub fn launch(command: Vec<OsString>) -> Result<(PathBuf, Vec<OsString>), std::io::Error> {
    if command.is_empty() {
        let preferred = std::env::var_os("SHELL").map(PathBuf::from);
        return Ok((super::pty::shell(preferred.as_deref())?, vec!["-i".into()]));
    }
    let mut command = command.into_iter();
    let program = command
        .next()
        .ok_or_else(|| std::io::Error::other("Missing command"))?;
    Ok((program.into(), command.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_argv_is_preserved_without_a_shell() -> Result<(), Box<dyn std::error::Error>> {
        let (options, command) = parse(
            [
                "--size",
                "480x272",
                "--command",
                "/an executable",
                "two words",
                "",
                "$HOME",
                "--size",
            ]
            .map(OsString::from)
            .to_vec(),
        )?;
        assert_eq!(options.ok_or("options")?.size, Some((480, 272)));
        let (program, args) = launch(command)?;
        assert_eq!(program, PathBuf::from("/an executable"));
        assert_eq!(args, ["two words", "", "$HOME", "--size"]);
        assert!(parse(vec!["--command".into()]).is_err());
        Ok(())
    }
}
