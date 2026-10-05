//! Blocking work stays on this worker; idle On-demand waits without a timer.
use super::{Control, Mode, Paths, Snapshot, State};
use crate::app_center::storage;
use std::{
    io::{BufRead, BufReader, Read},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(super) enum Event {
    Control(Control),
    Status(u64, State, Option<u8>, String),
}
struct Worker {
    paths: Paths,
    snapshot: Snapshot,
    child: Option<Child>,
    reader: Option<std::thread::JoinHandle<()>>,
    failures: u8,
    generation: u64,
    retry: Option<Instant>,
    idle: Option<Instant>,
    manual: bool,
    inhibited: bool,
}
impl Worker {
    fn start(&mut self, tx: &mpsc::SyncSender<Event>) -> Result<(), String> {
        if self.child.is_some() {
            return Ok(());
        }
        if self.snapshot.mode == Mode::Disabled {
            return Err("Tor support is disabled".into());
        }
        self.snapshot.state = State::Starting;
        self.snapshot.progress = None;
        self.snapshot.diagnostic.clear();
        let mut command = Command::new("/usr/bin/python3");
        let _configured_builder = command
            .args(["-I", "-u", "-c", super::HELPER])
            .arg(&self.paths.root)
            .arg(
                std::env::current_exe()
                    .map_err(|e| e.to_string())?
                    .with_file_name("arti"),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        self.spawn(&mut command, tx)
    }
    fn spawn(&mut self, command: &mut Command, tx: &mpsc::SyncSender<Event>) -> Result<(), String> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or("Tor worker generation exhausted")?;
        let mut child = command
            .spawn()
            .map_err(|e| format!("Tor supervisor: {e}"))?;
        let Some(stdout) = child.stdout.take() else {
            crate::process::cleanup_child(&mut child);
            return Err("Missing Tor status pipe".into());
        };
        let events = tx.clone();
        let reader = std::thread::Builder::new()
            .name("tor-status".into())
            .spawn(move || read_status(stdout, generation, &events));
        match reader {
            Ok(handle) => self.reader = Some(handle),
            Err(error) => {
                crate::process::cleanup_child(&mut child);
                return Err(format!("Tor status reader: {error}"));
            }
        }
        self.generation = generation;
        self.child = Some(child);
        Ok(())
    }
    fn stop(&mut self) {
        self.snapshot.state = State::Stopping;
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take()); // EOF makes the guardian stop and reap Arti.
            let now = Instant::now();
            let deadline = now.checked_add(Duration::from_secs(5)).unwrap_or(now);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => (),
                    Err(error) => {
                        eprintln!("level=error event=tor_child_wait message={error:?}");
                        break;
                    }
                }
                if Instant::now() >= deadline {
                    if let Err(error) = child.kill() {
                        eprintln!("level=error event=tor_child_kill message={error:?}");
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            // A failed kill must not turn into an unbounded `Child::wait`.
            if !reap(&mut child) {
                eprintln!("level=error event=tor_child_stuck message=reaping timed out");
                // Retain ownership and refuse to advertise Stopped or launch a
                // replacement while this supervisor is still unreaped.
                self.child = Some(child);
                self.inhibited = true;
                self.error("Tor supervisor could not stop; cleanup is still pending".into());
                return;
            }
        }
        if let Some(reader) = self.reader.take() {
            if reader.is_finished() {
                if let Err(payload) = reader.join() {
                    eprintln!("level=error event=tor_reader_panicked payload={payload:?}");
                }
            } else {
                // Never join a reader waiting to send into this worker's full
                // queue. The terminated guardian closes stdout; queued old
                // generations are discarded, and receiver shutdown releases a
                // blocked send. No detached reader owns a child process.
                eprintln!("level=debug event=tor_reader_finishing");
            }
        }
        self.snapshot.state = if self.snapshot.mode == Mode::Disabled {
            State::Disabled
        } else {
            State::Stopped
        };
        self.snapshot.progress = None;
        self.retry = None;
        self.idle = None;
    }
    fn error(&mut self, error: String) {
        self.snapshot.state = State::Error;
        self.snapshot.progress = None;
        self.snapshot.diagnostic = error;
    }
    fn wanted(&self) -> bool {
        !self.inhibited
            && self.snapshot.mode != Mode::Disabled
            && (self.manual || self.snapshot.mode == Mode::AlwaysOn || self.snapshot.apps > 0)
    }
    fn control(&mut self, control: Control, tx: &mpsc::SyncSender<Event>) -> Result<(), String> {
        match control {
            Control::Mode(mode) => {
                self.paths.save(mode)?;
                self.manual = false;
                self.snapshot.mode = mode;
                if self.child.is_none() {
                    self.snapshot.state = State::Stopped;
                    self.snapshot.diagnostic.clear();
                }
                self.inhibited = false;
                self.failures = 0;
                if mode == Mode::Disabled {
                    self.manual = false;
                    self.stop();
                }
            }
            Control::Demand(count) => {
                if count > self.snapshot.apps {
                    self.inhibited = false;
                }
                self.snapshot.apps = count;
                if count > 0 {
                    self.idle = None;
                }
            }
            Control::Start | Control::Restart => {
                if matches!(control, Control::Restart) {
                    self.stop();
                    if self.child.is_some() {
                        return Err("Tor cleanup is still pending; restart refused".into());
                    }
                }
                self.inhibited = false;
                self.failures = 0;
                self.retry = None;
                self.manual = true;
                self.start(tx)?;
            }
            Control::Stop => {
                self.manual = false;
                self.inhibited = true;
                self.stop();
                if self.child.is_none() {
                    self.snapshot.diagnostic = "Stopped by user".into();
                }
            }
            Control::Shutdown => (),
        }
        Ok(())
    }
    fn tick(&mut self, tx: &mpsc::SyncSender<Event>) {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(None) => (),
                result => {
                    let diagnostic = if self.snapshot.diagnostic.is_empty() {
                        format!("Arti stopped unexpectedly: {result:?}")
                    } else {
                        self.snapshot.diagnostic.clone()
                    };
                    self.stop();
                    self.error(diagnostic);
                    self.failures = self.failures.saturating_add(1);
                    if self.failures <= 3 {
                        self.retry = Some(
                            Instant::now()
                                .checked_add(Duration::from_secs(
                                    2_u64.pow(u32::from(self.failures)),
                                ))
                                .unwrap_or_else(Instant::now),
                        );
                    }
                }
            }
        }
        if self.wanted()
            && self.child.is_none()
            && self.failures <= 3
            && self.retry.is_none_or(|time| Instant::now() >= time)
        {
            self.retry = None;
            if let Err(error) = self.start(tx) {
                self.failures = 4;
                self.error(error);
            }
        }
        if !self.wanted() {
            self.retry = None;
        }
        if self.child.is_some() && !self.wanted() {
            let idle = self.idle.get_or_insert_with(|| {
                let now = Instant::now();
                now.checked_add(Duration::from_secs(30)).unwrap_or(now)
            });
            if Instant::now() >= *idle {
                self.stop();
            }
        }
    }
    fn publish(&self, output: &Arc<Mutex<Snapshot>>) {
        if let Ok(mut value) = output.lock() {
            value.clone_from(&self.snapshot);
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // This destructor runs on the service worker, never the event loop.
            // Retain the owned child through its final wait if bounded shutdown
            // failed; a relaunch cannot succeed before this worker finishes.
            crate::process::cleanup_child(&mut child);
        }
    }
}
fn read_status(stdout: std::process::ChildStdout, generation: u64, tx: &mpsc::SyncSender<Event>) {
    const LINE_LIMIT: u64 = 4096;
    let mut input = BufReader::new(stdout);
    let mut line = Vec::new();
    loop {
        line.clear();
        match input
            .by_ref()
            .take(LINE_LIMIT.saturating_add(1))
            .read_until(b'\n', &mut line)
        {
            Ok(0) => break,
            Ok(_) if u64::try_from(line.len()).is_ok_and(|size| size <= LINE_LIMIT) => (),
            Ok(_) => {
                eprintln!("level=error event=tor_status_oversized");
                break;
            }
            Err(error) => {
                eprintln!("level=error event=tor_status_read message={error:?}");
                break;
            }
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line) else {
            continue;
        };
        let state = match value.get("state").and_then(serde_json::Value::as_str) {
            Some("connected") => State::Connected,
            Some("bootstrapping") => State::Bootstrapping,
            Some("error") => State::Error,
            _ => continue,
        };
        let progress = value
            .get("progress")
            .and_then(serde_json::Value::as_u64)
            .and_then(|p| u8::try_from(p.min(100)).ok());
        let diagnostic = value
            .get("diagnostic")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .chars()
            .take(160)
            .collect();
        // Preserve terminal transitions even when the bounded queue is full.
        // stop() never joins an unfinished sender, so shutdown cannot deadlock.
        if let Err(error) = tx.send(Event::Status(generation, state, progress, diagnostic)) {
            eprintln!("level=debug event=tor_status_disconnected message={error}");
            break;
        }
    }
}
/// Reap an already stopped or killed child without risking an unbounded
/// `Child::wait`. Returns false when the child is still alive after the grace period.
fn reap(child: &mut Child) -> bool {
    let now = Instant::now();
    let deadline = now.checked_add(Duration::from_secs(1)).unwrap_or(now);
    loop {
        if child.try_wait().is_ok_and(|status| status.is_some()) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
pub(super) fn run(
    paths: Paths,
    rx: &mpsc::Receiver<Event>,
    tx: &mpsc::SyncSender<Event>,
    output: &Arc<Mutex<Snapshot>>,
) {
    let mode = paths.load();
    let mut worker = Worker {
        paths,
        snapshot: Snapshot::default(),
        child: None,
        reader: None,
        failures: 0,
        generation: 0,
        retry: None,
        idle: None,
        manual: false,
        inhibited: false,
    };
    match mode {
        Ok(loaded_mode) => {
            worker.snapshot.mode = loaded_mode;
            if loaded_mode == Mode::Disabled {
                worker.snapshot.state = State::Disabled;
            }
        }
        Err(error) => {
            worker.snapshot.mode = Mode::Disabled;
            worker.inhibited = true;
            worker.error(error);
        }
    }
    // Create defaults only when absent; malformed input is never overwritten.
    if worker.paths.load().is_ok()
        && storage::read(&worker.paths.config, 4096).is_ok_and(|f| f.is_none())
        && let Err(error) = worker.paths.save(worker.snapshot.mode)
    {
        worker.error(error);
        worker.inhibited = true;
    }
    worker.tick(tx);
    worker.publish(output);
    loop {
        let event = if worker.child.is_some() || worker.retry.is_some() {
            rx.recv_timeout(Duration::from_secs(1))
        } else {
            match rx.recv() {
                Ok(event) => Ok(event),
                Err(mpsc::RecvError) => Err(mpsc::RecvTimeoutError::Disconnected),
            }
        };
        match event {
            Ok(Event::Control(Control::Shutdown)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
            Ok(Event::Control(control)) => {
                if matches!(
                    control,
                    Control::Stop | Control::Restart | Control::Mode(Mode::Disabled)
                ) && worker.child.is_some()
                {
                    worker.snapshot.state = State::Stopping;
                    worker.publish(output);
                }
                if let Err(error) = worker.control(control, tx) {
                    worker.error(error);
                }
            }
            Ok(Event::Status(generation, state, progress, diagnostic))
                if worker.child.is_some()
                    && generation == worker.generation
                    && !worker.inhibited =>
            {
                worker.snapshot.state = state;
                worker.snapshot.progress = progress;
                worker.snapshot.diagnostic = diagnostic;
            }
            _ => (),
        }
        worker.tick(tx);
        worker.publish(output);
    }
    worker.stop();
    worker.publish(output);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Result<(crate::test_support::Scratch, Worker), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let worker = Worker {
            paths: Paths {
                config: root.join("tor.json"),
                root,
            },
            snapshot: Snapshot::default(),
            child: None,
            reader: None,
            failures: 0,
            generation: 0,
            retry: None,
            idle: None,
            manual: false,
            inhibited: false,
        };
        Ok((scratch, worker))
    }
    fn fake(worker: &mut Worker, tx: &mpsc::SyncSender<Event>) -> Result<(), String> {
        let mut command = Command::new("/usr/bin/python3");
        let _configured_builder = command
            .args([
                "-I",
                "-u",
                "-c",
                "import sys; print('{\"state\":\"connected\",\"progress\":100}'); sys.stdin.read()",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        worker.spawn(&mut command, tx)
    }
    #[test]
    fn duplicate_start_stop_restart_and_on_demand_grace() -> Result<(), String> {
        let (_scratch, mut worker) = fixture()?;
        let (tx, rx) = mpsc::sync_channel(32);
        fake(&mut worker, &tx)?;
        let pid = worker.child.as_ref().ok_or("no child")?.id();
        worker.start(&tx)?;
        assert_eq!(worker.child.as_ref().ok_or("no child")?.id(), pid);
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(3)),
            Ok(Event::Status(_, State::Connected, Some(100), _))
        ));
        worker.control(Control::Demand(1), &tx)?;
        worker.tick(&tx);
        assert!(worker.idle.is_none());
        worker.control(Control::Demand(0), &tx)?;
        worker.tick(&tx);
        assert!(worker.idle.is_some());
        worker.idle = Some(Instant::now());
        worker.tick(&tx);
        assert!(worker.child.is_none());
        assert_eq!(worker.snapshot.state, State::Stopped);
        for _ in 0_i32..3_i32 {
            fake(&mut worker, &tx)?;
            worker.stop();
            assert!(worker.child.is_none());
        }
        worker.control(Control::Mode(Mode::Disabled), &tx)?;
        assert_eq!(worker.snapshot.state, State::Disabled);
        assert!(worker.start(&tx).is_err());
        assert_eq!(worker.paths.load()?, Mode::Disabled);
        Ok(())
    }
    #[test]
    fn crashes_use_bounded_backoff_and_idle_cancels_recovery() -> Result<(), String> {
        let (_scratch, mut worker) = fixture()?;
        let (tx, _rx) = mpsc::sync_channel(32);
        worker.snapshot.apps = 1;
        for failures in 1..=4 {
            fake(&mut worker, &tx)?;
            let child = worker.child.as_mut().ok_or("no child")?;
            child.kill().map_err(|e| e.to_string())?;
            let _reaped_status = child.wait().map_err(|e| e.to_string())?;
            worker.tick(&tx);
            assert_eq!(worker.failures, failures);
            assert_eq!(worker.snapshot.state, State::Error);
            assert_eq!(worker.retry.is_some(), failures <= 3);
        }
        worker.snapshot.apps = 0;
        worker.tick(&tx);
        assert!(worker.retry.is_none());
        assert!(worker.child.is_none());
        Ok(())
    }
    #[test]
    fn full_status_queue_preserves_terminal_events_and_does_not_block_stop() -> Result<(), String> {
        let (_scratch, mut worker) = fixture()?;
        let (tx, rx) = mpsc::sync_channel(1);
        let mut command = Command::new("/usr/bin/python3");
        let _pipe_options = command.args(["-I", "-u", "-c", "import sys\nprint('{\"state\":\"bootstrapping\"}')\nprint('{\"state\":\"connected\",\"progress\":100}')\nprint('{\"state\":\"connected\",\"progress\":100}')\nsys.stdin.read()"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        worker.spawn(&mut command, &tx)?;
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(3)),
            Ok(Event::Status(_, State::Bootstrapping, None, _))
        ));
        let stopping = Instant::now();
        worker.stop();
        assert!(
            stopping.elapsed() < Duration::from_secs(3),
            "shutdown must not join a sender blocked on a full status queue"
        );
        assert!(worker.child.is_none(), "shutdown must reap the supervisor");
        for _ in 0_i32..2_i32 {
            assert!(
                matches!(
                    rx.recv_timeout(Duration::from_secs(3)),
                    Ok(Event::Status(_, State::Connected, Some(100_u8), _))
                ),
                "connected transitions must survive a full queue"
            );
        }
        Ok(())
    }
    #[test]
    fn reaping_is_bounded_when_a_kill_does_not_take_effect() -> Result<(), String> {
        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        // A live child cannot be reaped; the helper must time out instead of blocking.
        assert!(!reap(&mut child));
        child.kill().map_err(|e| e.to_string())?;
        assert!(reap(&mut child));
        Ok(())
    }
}
