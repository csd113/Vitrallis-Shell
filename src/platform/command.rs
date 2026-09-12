//! Bounded, shell-free subprocess execution; used only off the UI thread.
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    run_bounded(program, args, 8192)
}
pub fn run_bounded(program: &str, args: &[&str], limit: u64) -> Result<String, String> {
    execute(program, args, limit, false)
}
#[cfg(not(target_os = "linux"))]
pub fn process_arguments(pid: u32) -> Result<Option<String>, String> {
    // BSD ps exits 1 with no output when a valid PID query has no match.
    let output = execute(
        "/bin/ps",
        &["-p", &pid.to_string(), "-o", "command="],
        8192,
        true,
    )?;
    Ok((!output.trim().is_empty()).then_some(output))
}
fn execute(
    program: &str,
    args: &[&str],
    limit: u64,
    no_matches_ok: bool,
) -> Result<String, String> {
    let mut command = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("missing command output".into());
    };
    let (send, receive) = mpsc::sync_channel(1);
    let reader = thread::Builder::new()
        .name("system-output".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .take(limit.saturating_add(1))
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = send.send(result);
        });
    if let Err(error) = reader {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.to_string());
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            result => {
                crate::process::cleanup_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program}: timeout or wait failure ({result:?})"));
            }
        }
    };
    if !(status.success() || no_matches_ok && status.code() == Some(1)) {
        crate::process::cleanup_group(child.id());
        return Err(format!("{program}: {status}"));
    }
    let bytes = receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| {
            crate::process::cleanup_group(child.id());
            "command output timed out".to_owned()
        })?
        .map_err(|e| e.to_string())?;
    if u64::try_from(bytes.len()).map_err(|e| e.to_string())? > limit {
        return Err(format!("command output exceeds {limit} bytes"));
    }
    String::from_utf8(bytes).map_err(|_| "command output is not UTF-8".into())
}
pub fn clock() -> Option<String> {
    let value = run("/bin/date", &["+%H:%M"]).ok()?;
    let value = value.trim();
    let (hours, minutes) = value.split_once(':')?;
    if value.len() == 5 && hours.parse::<u8>().ok()? < 24 && minutes.parse::<u8>().ok()? < 60 {
        Some(value.into())
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_missing_and_excessive_commands_are_errors() {
        assert!(run("/no/such/vitrallis-command", &[]).is_err());
        assert!(run("/bin/sh", &["-c", "exit 7"]).is_err());
        assert!(run("/bin/sh", &["-c", "printf '%09000d' 0"]).is_err());
        assert!(run("/bin/sh", &["-c", "exec sleep 5"]).is_err());
    }
    #[test]
    fn descendants_holding_output_are_cleaned_on_timeout() -> Result<(), String> {
        let started = Instant::now();
        // The shell exits immediately, but its descendant retains the output
        // pipe. The command deadline must cover both process and pipe lifetime.
        let result = run("/bin/sh", &["-c", "sleep 30 &"]);
        assert!(result.is_err_and(|error| error.contains("output timed out")));
        if started.elapsed() > Duration::from_secs(5) {
            return Err("inherited output pipe blocked command cleanup".into());
        }
        Ok(())
    }
}
