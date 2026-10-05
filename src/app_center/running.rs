//! Match an owned Python script or native executable and process start identity before TERM.
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
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
        #[cfg(target_os = "linux")]
        {
            // Acquire a kernel reference before rechecking the requested app's
            // start identity; a later exit/reuse cannot redirect the signal.
            let Some(handle) = vitrallis_native::process::ProcessHandle::open(identity.pid)
                .map_err(|error| {
                    format!("Cannot safely close this process: {error}; close the app manually")
                })?
            else {
                return Ok(());
            };
            let uid = std::fs::metadata("/proc/self")
                .map_err(|error| error.to_string())?
                .uid();
            if process_identity_result(linux_identity(entry, identity.pid, uid))?.as_ref()
                == Some(identity)
            {
                handle
                    .terminate()
                    .map_err(|error| format!("App close failed: {error}"))?;
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            if self.list(entry)?.contains(identity) {
                return Err("This host cannot safely signal an external process by identity; close the app manually".into());
            }
            Ok(())
        }
    }
}
pub fn close(
    processes: &impl Processes,
    entry: &Path,
    confirmed: &[Identity],
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or("App close deadline overflow")?;
    for identity in confirmed {
        processes.terminate(entry, identity)?;
    }
    while !processes.list(entry)?.is_empty() {
        if Instant::now() >= deadline {
            return Err("App did not close within eight seconds; skipped".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
pub(super) fn python(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            (name == "python" || name == "Python")
                || name.strip_prefix("python").is_some_and(|v| {
                    !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit() || c == b'.')
                })
        })
}
#[cfg(target_os = "linux")]
fn linux(entry: &Path) -> Result<Vec<Identity>, String> {
    let uid = std::fs::metadata("/proc/self")
        .map_err(|error| error.to_string())?
        .uid();
    let mut found = Vec::new();
    for directory in std::fs::read_dir("/proc").map_err(|error| error.to_string())? {
        let proc_entry = directory.map_err(|error| error.to_string())?;
        let Ok(pid) = proc_entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if let Some(identity) = process_identity_result(linux_identity(entry, pid, uid))? {
            found.push(identity);
        }
    }
    Ok(found)
}
#[cfg(target_os = "linux")]
fn linux_identity(entry: &Path, pid: u32, uid: u32) -> std::io::Result<Option<Identity>> {
    let path = std::path::PathBuf::from("/proc").join(pid.to_string());
    if std::fs::metadata(&path)?.uid() != uid {
        return Ok(None);
    }
    let initial_start = linux_start(&path)?;
    let bytes = proc_read(&path.join("cmdline"), 64 * 1024)?;
    let args = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let executable = std::fs::read_link(path.join("exe"))?;
    let native = executable == entry;
    if !native {
        let Some((program, remaining)) = args.split_first() else {
            return Ok(None);
        };
        if !std::str::from_utf8(program).is_ok_and(python) {
            return Ok(None);
        }
        let texts = remaining
            .iter()
            .map(|argument| std::str::from_utf8(argument))
            .collect::<Result<Vec<_>, _>>();
        let script = texts
            .as_ref()
            .ok()
            .and_then(|arguments| script_argument(arguments))
            .map(str::as_bytes);
        let target = entry.as_os_str().as_encoded_bytes();
        if script.is_none() && remaining.contains(&target) {
            return Err(std::io::Error::other(
                "Unrecognized Python options for this app; close it manually",
            ));
        }
        if script != Some(target) {
            return Ok(None);
        }
    }
    if initial_start != linux_start(&path)? {
        return Err(std::io::Error::other(
            "Process changed during identity inspection; retry",
        ));
    }
    Ok(Some(Identity {
        pid,
        start: initial_start,
    }))
}
#[cfg(target_os = "linux")]
fn linux_start(path: &Path) -> std::io::Result<String> {
    let bytes = proc_read(&path.join("stat"), 16 * 1024)?;
    let record = std::str::from_utf8(&bytes).map_err(std::io::Error::other)?;
    record
        .rsplit_once(')')
        .and_then(|(_, fields)| fields.split_whitespace().nth(19))
        .filter(|start| start.parse::<u64>().is_ok())
        .map(str::to_owned)
        .ok_or_else(|| std::io::Error::other("Invalid process identity"))
}
#[cfg(target_os = "linux")]
fn proc_read(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let sentinel = limit
        .checked_add(1)
        .ok_or_else(|| std::io::Error::other("proc limit overflow"))?;
    let mut bytes = Vec::new();
    let _bytes_read = std::fs::File::open(path)?
        .take(sentinel)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_err(std::io::Error::other)? > limit {
        return Err(std::io::Error::other(
            "Process metadata exceeds inspection bounds",
        ));
    }
    Ok(bytes)
}
#[cfg(target_os = "linux")]
fn process_identity_result(
    result: std::io::Result<Option<Identity>>,
) -> Result<Option<Identity>, String> {
    match result {
        // procfs can report ESRCH instead of ENOENT while a process exits.
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || error.raw_os_error() == Some(libc::ESRCH) =>
        {
            Ok(None)
        }
        outcome => outcome.map_err(|error| error.to_string()),
    }
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
        let [
            owner,
            pid_field,
            day,
            month,
            date,
            clock,
            year,
            executable @ ..,
        ] = parts.as_slice()
        else {
            continue;
        };
        if executable.is_empty() || *owner != uid.trim() {
            continue;
        }
        let program = executable.join(" ");
        if program == script {
            found.push(Identity {
                pid: pid_field
                    .parse::<u32>()
                    .map_err(|error| format!("Invalid PID: {error}"))?,
                start: [*day, *month, *date, *clock, *year].join(" "),
            });
            continue;
        }
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
        let pid = pid_field
            .parse::<u32>()
            .map_err(|error| format!("Invalid PID: {error}"))?;
        let Some(command) = crate::platform::command::process_arguments(pid)? else {
            continue;
        };
        let Some(arguments) = command.trim().strip_prefix(&program) else {
            if command.contains(script) {
                return Err(
                    "Target process changed during inspection; retry after closing it".into(),
                );
            }
            continue;
        };
        let args: Vec<_> = arguments.split_whitespace().collect();
        if script_argument(&args).is_none() && args.contains(&script) {
            return Err("Unrecognized Python options for this app; close it manually".into());
        }
        if script_argument(&args) == Some(script) {
            found.push(Identity {
                pid,
                start: [*day, *month, *date, *clock, *year].join(" "),
            });
        }
    }
    Ok(found)
}
fn script_argument<'a>(args: &[&'a str]) -> Option<&'a str> {
    let mut arguments = args.iter().copied();
    while let Some(arg) = arguments.next() {
        match arg {
            "-u" | "-B" | "-s" | "-E" | "-I" | "-O" | "-OO" => {}
            "--" => return arguments.next(),
            "--check-hash-based-pycs" => {
                if !matches!(arguments.next(), Some("default" | "always" | "never")) {
                    return None;
                }
            }
            option if option.starts_with('-') => return None,
            script => return Some(script),
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn disappeared_processes_do_not_hide_other_scan_errors() -> Result<(), String> {
        let identity = Identity {
            pid: 42,
            start: "verified".into(),
        };
        assert_eq!(
            process_identity_result(Ok(Some(identity.clone())))?,
            Some(identity)
        );
        for code in [libc::ENOENT, libc::ESRCH] {
            assert_eq!(
                process_identity_result(Err(std::io::Error::from_raw_os_error(code)))?,
                None
            );
        }
        for code in [libc::EACCES, libc::EPERM, libc::EIO] {
            let error = std::io::Error::from_raw_os_error(code);
            let message = error.to_string();
            assert_eq!(process_identity_result(Err(error)), Err(message));
        }
        Ok(())
    }
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
        for value in ["default", "always", "never"] {
            assert_eq!(
                script_argument(&["--check-hash-based-pycs", value, "/app.py"]),
                Some("/app.py")
            );
        }
        assert_eq!(script_argument(&["--check-hash-based-pycs"]), None);
        assert_eq!(
            script_argument(&["--check-hash-based-pycs", "/app.py"]),
            None
        );
        assert_eq!(
            script_argument(&["--check-hash-based-pycs", "invalid", "/app.py"]),
            None
        );
        assert_eq!(script_argument(&["--", "-app.py"]), Some("-app.py"));
        assert!(close(&Stuck, Path::new("/app.py"), &[], Duration::ZERO).is_err());
    }
}
