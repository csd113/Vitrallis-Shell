//! Blocking work stays on this worker; idle On-demand waits without a timer.
use super::{Control, Mode, Paths, Snapshot, State};
use crate::app_center::storage;
use std::{
    io::{BufRead, BufReader},
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
        command
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
        let mut child = command
            .spawn()
            .map_err(|e| format!("Tor supervisor: {e}"))?;
        let stdout = child.stdout.take().ok_or("Missing Tor status pipe")?;
        self.generation += 1;
        let generation = self.generation;
        let tx = tx.clone();
        self.reader = Some(std::thread::spawn(move || {
            // The embedded helper emits bounded JSON, never raw Arti logs.
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                let state = match v["state"].as_str() {
                    Some("connected") => State::Connected,
                    Some("bootstrapping") => State::Bootstrapping,
                    Some("error") => State::Error,
                    _ => continue,
                };
                let progress = v["progress"]
                    .as_u64()
                    .and_then(|p| u8::try_from(p.min(100)).ok());
                let diagnostic = v["diagnostic"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(160)
                    .collect();
                let _ = tx.try_send(Event::Status(generation, state, progress, diagnostic));
            }
        }));
        self.child = Some(child);
        Ok(())
    }
    fn stop(&mut self) {
        self.snapshot.state = State::Stopping;
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take()); // EOF makes the guardian stop and reap Arti.
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
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
                self.snapshot.diagnostic = "Stopped by user".into();
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
                                + Duration::from_secs(2_u64.pow(u32::from(self.failures))),
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
            let idle = self
                .idle
                .get_or_insert_with(|| Instant::now() + Duration::from_secs(30));
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
        Ok(mode) => {
            worker.snapshot.mode = mode;
            if mode == Mode::Disabled {
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
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
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
                if worker.child.is_some() && generation == worker.generation =>
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
        command
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
        for _ in 0..3 {
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
            child.wait().map_err(|e| e.to_string())?;
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
}
