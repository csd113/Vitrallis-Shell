//! Bounded, shell-free subprocess execution; used only off the UI thread.
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(program)
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
            let result = stdout.take(8193).read_to_end(&mut bytes).map(|_| bytes);
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
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program}: timeout or wait failure ({result:?})"));
            }
        }
    };
    if !status.success() {
        return Err(format!("{program}: {status}"));
    }
    let bytes = receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| "command output timed out".to_owned())?
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8192 {
        return Err("command output exceeds 8 KiB".into());
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
}
