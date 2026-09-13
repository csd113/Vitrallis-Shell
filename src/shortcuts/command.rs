//! Command-line quoting without shell expansion or execution.
use crate::{app::AppManifest, discovery::executable};
use std::{ffi::OsString, path::Path};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Direct,
    Shell,
}

/// Single and double quotes group arguments; backslashes escape outside single
/// quotes. Double-quoted backslashes follow POSIX quoting. Expansion is never
/// performed. Unquoted shell operators require the explicit shell option.
pub fn parse(command: &str) -> Result<Vec<OsString>, String> {
    let mut words: Vec<OsString> = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        if c == '\0' || (c.is_control() && c != '\t') {
            return Err("Command must be one line without control characters".into());
        }
        match (quote, c) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), _) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (_, '\\') => {
                let next = chars.next().ok_or("Command ends with a backslash")?;
                if next.is_control() {
                    return Err("Invalid escaped control character".into());
                }
                if quote == Some('"') && !matches!(next, '$' | '`' | '"' | '\\') {
                    word.push('\\');
                }
                word.push(next);
                started = true;
            }
            (None, ' ' | '\t') => {
                if started {
                    words.push(std::mem::take(&mut word).into());
                    started = false;
                }
            }
            (None, '|' | '&' | ';' | '<' | '>' | '(' | ')' | '`') => {
                return Err("Pipelines and redirects require Shell mode".into());
            }
            _ => {
                word.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err("Command has an unclosed quote".into());
    }
    if started {
        words.push(word.into());
    }
    if words.first().is_none_or(|word| word.is_empty()) {
        return Err("Enter an executable and optional arguments".into());
    }
    Ok(words)
}

pub fn manifest(
    command: &str,
    mode: Mode,
    cwd: &Path,
    path: &std::ffi::OsStr,
) -> Result<AppManifest, String> {
    if !cwd.is_absolute() || !cwd.is_dir() {
        return Err("Working directory must be an existing absolute directory".into());
    }
    let words = match mode {
        Mode::Direct => parse(command)?,
        Mode::Shell => vec!["/bin/sh".into(), "-c".into(), command.into()],
    };
    let program = words
        .first()
        .and_then(|word| word.to_str())
        .ok_or("Invalid executable encoding")?;
    let search = std::env::split_paths(path).collect::<Vec<_>>();
    let entry = executable::resolve(program, cwd, &search)
        .ok_or_else(|| format!("Executable not found or not executable: {program}"))?;
    Ok(AppManifest {
        entry,
        args: words.into_iter().skip(1).collect(),
        cwd: Some(cwd.into()),
        ..AppManifest::default()
    })
}

pub fn quote(path: &Path) -> Result<String, String> {
    let path = path.to_str().ok_or("Path must be valid UTF-8")?;
    if path.chars().any(char::is_control) {
        return Err("Path contains control characters".into());
    }
    Ok(format!("'{}'", path.replace('\'', "'\\''")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_preserve_empty_arguments_spaces_and_literal_expansion() -> Result<(), String> {
        let result = parse(r#"'/opt/my app' "two words" '' a\ b '$HOME' "a\q" "a\"b""#)?;
        assert_eq!(
            result,
            [
                "/opt/my app",
                "two words",
                "",
                "a b",
                "$HOME",
                "a\\q",
                "a\"b"
            ]
        );
        for bad in [
            "",
            "''",
            "echo '",
            "echo \\",
            "echo a | cat",
            "echo > out",
            "echo\ntrue",
            "echo\0",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        let path = Path::new("/a path/it's executable");
        assert_eq!(parse(&quote(path)?)?, [path.as_os_str()]);
        Ok(())
    }
}
