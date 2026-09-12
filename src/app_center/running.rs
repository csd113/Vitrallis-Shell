//! Match a user's exact Python script and process start identity before TERM.
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub start: String,
}
pub trait Processes {
    fn list(&self, entry: &Path) -> Result<Vec<Identity>, String>;
    fn terminate(&self, entry: &Path, identity: &Identity) -> Result<(), String>;
}
pub struct Native;
impl Processes for Native {
    fn list(&self, entry: &Path) -> Result<Vec<Identity>, String> {
        #[cfg(target_os = "linux")]
        {
            linux(entry)
        }
        #[cfg(not(target_os = "linux"))]
        {
            portable(entry)
        }
    }
    fn terminate(&self, entry: &Path, identity: &Identity) -> Result<(), String> {
        if self.list(entry)?.contains(identity) {
            crate::platform::command::run(
                "/bin/kill",
                &["-TERM", "--", &identity.pid.to_string()],
            )?;
        }
        Ok(())
    }
}
pub fn close(
    processes: &impl Processes,
    entry: &Path,
    confirmed: &[Identity],
    timeout: Duration,
) -> Result<(), String> {
    for identity in confirmed {
        processes.terminate(entry, identity)?;
    }
    let deadline = Instant::now() + timeout;
    while !processes.list(entry)?.is_empty() {
        if Instant::now() >= deadline {
            return Err("App did not close within eight seconds; skipped".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
pub(super) fn python(s: &str) -> bool {
    Path::new(s)
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| {
            (s == "python" || s == "Python")
                || s.strip_prefix("python").is_some_and(|v| {
                    !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit() || c == b'.')
                })
        })
}
#[cfg(target_os = "linux")]
fn linux(entry: &Path) -> Result<Vec<Identity>, String> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self")
        .map_err(|e| e.to_string())?
        .uid();
    let mut found = Vec::new();
    for proc in std::fs::read_dir("/proc").map_err(|e| e.to_string())? {
        let proc = proc.map_err(|e| e.to_string())?;
        let Ok(pid) = proc.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let path = proc.path();
        let result = (|| {
            if std::fs::metadata(&path)?.uid() != uid {
                return Ok(None);
            }
            let bytes = std::fs::read(path.join("cmdline"))?;
            let args: Vec<_> = bytes.split(|b| *b == 0).collect();
            let texts = args
                .iter()
                .filter_map(|a| std::str::from_utf8(a).ok())
                .collect::<Vec<_>>();
            if args.len() < 2
                || !std::str::from_utf8(args[0]).is_ok_and(python)
                || texts.len() != args.len()
                || script_argument(&texts[1..]).map(str::as_bytes)
                    != Some(entry.as_os_str().as_encoded_bytes())
            {
                return Ok(None);
            }
            let record = std::fs::read_to_string(path.join("stat"))?;
            let start = record
                .rsplit_once(')')
                .and_then(|(_, s)| s.split_whitespace().nth(19))
                .ok_or_else(|| std::io::Error::other("Invalid process identity"))?;
            Ok::<_, std::io::Error>(Some(Identity {
                pid,
                start: start.into(),
            }))
        })();
        match result {
            Ok(Some(p)) => found.push(p),
            Ok(None) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(found)
}
#[cfg(not(target_os = "linux"))]
fn portable(entry: &Path) -> Result<Vec<Identity>, String> {
    let uid = crate::platform::command::run("/usr/bin/id", &["-u"])?;
    let output = crate::platform::command::run_bounded(
        "/bin/ps",
        &["-u", uid.trim(), "-o", "uid=,pid=,lstart=,comm="],
        1024 * 1024,
    )?;
    let script = entry.to_str().ok_or("Process path must be UTF-8")?;
    let mut found = Vec::new();
    for line in output.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() < 8 || parts[0] != uid.trim() {
            continue;
        }
        let program = parts[7..].join(" ");
        if !python(&program) {
            continue;
        }
        // Unlike splitting command= blindly, comm= also recognizes framework
        // interpreter paths containing spaces. Ambiguous script paths fail closed.
        if script.chars().any(char::is_whitespace) {
            return Err(
                "Cannot verify Python script arguments containing whitespace on this host".into(),
            );
        }
        let pid = parts[1].parse::<u32>().map_err(|_| "Invalid PID")?;
        let Some(command) = crate::platform::command::process_arguments(pid)? else {
            continue;
        };
        let Some(args) = command.trim().strip_prefix(&program) else {
            if command.contains(script) {
                return Err(
                    "Target process changed during inspection; retry after closing it".into(),
                );
            }
            continue;
        };
        let args: Vec<_> = args.split_whitespace().collect();
        if script_argument(&args).is_none() && args.contains(&script) {
            return Err("Unrecognized Python options for this app; close it manually".into());
        }
        if script_argument(&args) == Some(script) {
            found.push(Identity {
                pid,
                start: parts[2..7].join(" "),
            });
        }
    }
    Ok(found)
}
fn script_argument<'a>(args: &[&'a str]) -> Option<&'a str> {
    for arg in args {
        if matches!(*arg, "-u" | "-B" | "-s" | "-E" | "-I" | "-O" | "-OO" | "--") {
            continue;
        }
        if arg.starts_with('-') {
            return None;
        }
        return Some(arg);
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_and_timeout_are_conservative() {
        struct Stuck;
        impl Processes for Stuck {
            fn list(&self, _: &Path) -> Result<Vec<Identity>, String> {
                Ok(vec![Identity {
                    pid: 42,
                    start: "later".into(),
                }])
            }
            fn terminate(&self, _: &Path, _: &Identity) -> Result<(), String> {
                Ok(())
            }
        }
        assert!(python("/usr/bin/python3.13"));
        assert!(!python("python-helper"));
        assert_eq!(script_argument(&["-u", "-B", "/app.py"]), Some("/app.py"));
        assert_eq!(script_argument(&["-c", "/app.py"]), None);
        assert!(close(&Stuck, Path::new("/app.py"), &[], Duration::ZERO).is_err());
    }
}
