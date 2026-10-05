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
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(2))
        .ok_or("command deadline overflow")?;
    let sentinel = limit
        .checked_add(1)
        .ok_or("command output limit overflow")?;
    let mut command = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let _group_command = command.process_group(0);
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
        crate::process::cleanup_child(&mut child);
        return Err("missing command output".into());
    };
    let (send, receive) = mpsc::sync_channel(1);
    let spawned_reader = thread::Builder::new()
        .name("system-output".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.take(sentinel).read_to_end(&mut bytes).map(|_| bytes);
            if send.send(result).is_err() {
                eprintln!("level=debug event=command_output_cancelled");
            }
        });
    // On an error the reader is detached after group termination, rather than
    // risking an unbounded join on an inherited pipe. Success below joins it.
    let reader = match spawned_reader {
        Ok(reader) => reader,
        Err(error) => {
            crate::process::cleanup_child(&mut child);
            return Err(error.to_string());
        }
    };
    let output = (|| {
        loop {
            #[cfg(unix)]
            let exited = vitrallis_native::process::exited_unreaped(&mut child)
                .map_err(|error| format!("{program}: exit probe failed: {error}"))?;
            #[cfg(not(unix))]
            let exited = child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some();
            if exited {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!("{program}: command timed out"));
            }
            thread::sleep(Duration::from_millis(10));
        }
        receive
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| format!("command output timed out: {error}"))?
            .map_err(|error| error.to_string())
    })();
    // System helpers are bounded tasks, not app launchers. Retain the child's
    // PID until any remaining descendants are stopped, then reap it. In
    // particular, an inherited stdout cannot authorize killing a reused PID.
    crate::process::cleanup_group(&mut child);
    if output.is_err() {
        crate::process::cleanup_child(&mut child);
        return output.map(|_| String::new());
    }
    let status = child
        .wait()
        .map_err(|error| format!("{program}: wait failed: {error}"))?;
    if !(status.success() || no_matches_ok && status.code() == Some(1_i32)) {
        return Err(format!("{program}: {status}"));
    }
    let bytes = output?;
    reader
        .join()
        .map_err(|payload| format!("command output worker panicked: {payload:?}"))?;
    if u64::try_from(bytes.len()).map_err(|e| e.to_string())? > limit {
        return Err(format!("command output exceeds {limit} bytes"));
    }
    String::from_utf8(bytes)
        .map_err(|error| format!("command output is not UTF-8: {}", error.utf8_error()))
}
pub fn clock() -> Option<String> {
    let value = run("/bin/date", &["+%H:%M"]).ok()?;
    let clock = value.trim();
    let (hours, minutes) = clock.split_once(':')?;
    if clock.len() == 5 && hours.parse::<u8>().ok()? < 24 && minutes.parse::<u8>().ok()? < 60 {
        Some(clock.into())
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
