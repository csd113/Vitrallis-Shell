//! Reap background service children without blocking the launcher heartbeat.
use crate::{platform::system::Worker, tor::Service};
use std::thread::{self, JoinHandle};

#[derive(Default)]
pub(super) struct Services {
    stopping: Option<JoinHandle<Result<(), String>>>,
    started: bool,
}
impl Services {
    pub(super) fn prepare(
        &mut self,
        system: &mut Option<Worker>,
        tor: &mut Service,
    ) -> Result<bool, String> {
        if !self.started {
            self.started = true;
            let system = system.take();
            let tor = std::mem::take(tor);
            self.stopping = Some(
                thread::Builder::new()
                    .name("shell-relaunch".into())
                    .spawn(move || {
                        let system_result = system.map_or(Ok(()), Worker::shutdown);
                        let tor_result = tor.shutdown();
                        system_result.and(tor_result)
                    })
                    .map_err(|error| format!("Cannot prepare shell relaunch: {error}"))?,
            );
        }
        let handle = self
            .stopping
            .as_ref()
            .ok_or("Relaunch preparation unavailable")?;
        if !handle.is_finished() {
            return Ok(false);
        }
        self.stopping
            .take()
            .ok_or("Relaunch preparation unavailable")?
            .join()
            .map_err(|_| "Relaunch preparation stopped unexpectedly".to_owned())??;
        Ok(true)
    }
    pub(super) const fn active(&self) -> bool {
        self.started
    }
    pub(super) const fn completed(&self) -> bool {
        self.started && self.stopping.is_none()
    }
    pub(super) const fn reset(&mut self) {
        self.started = false;
    }
}
impl Drop for Services {
    fn drop(&mut self) {
        if let Some(handle) = self.stopping.take() {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    eprintln!("level=error event=relaunch_shutdown_failed message={error:?}");
                }
                Err(_) => {
                    eprintln!("level=error event=relaunch_shutdown_failed message=worker_panicked");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::system::{Control, Status, System};
    use std::{
        process::{Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };

    #[derive(Clone)]
    struct Probe {
        release: Arc<Mutex<mpsc::Receiver<()>>>,
        started: mpsc::SyncSender<Result<u32, String>>,
        reaped: Arc<AtomicBool>,
    }
    impl System for Probe {
        fn refresh(&mut self) -> Status {
            let child = Command::new("/bin/sleep")
                .arg("30")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            let mut child = match child {
                Ok(child) => child,
                Err(error) => {
                    let _ = self.started.send(Err(error.to_string()));
                    return Status::default();
                }
            };
            let _ = self.started.send(Ok(child.id()));
            if let Ok(release) = self.release.lock() {
                // Also releases and reaps the fixture if the test exits early.
                let _ = release.recv_timeout(Duration::from_secs(5));
            }
            let killed = child.kill();
            let waited = child.wait();
            self.reaped
                .store(killed.is_ok() && waited.is_ok(), Ordering::SeqCst);
            Status::default()
        }
        fn control(&mut self, _: Control, _: &mut Status) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn preparation_waits_for_the_status_child_without_blocking_its_caller() -> Result<(), String> {
        let (release, wait) = mpsc::channel();
        let (started, started_child) = mpsc::sync_channel(1);
        let reaped = Arc::new(AtomicBool::new(false));
        let mut system = Some(Worker::start(Probe {
            release: Arc::new(Mutex::new(wait)),
            started,
            reaped: Arc::clone(&reaped),
        })?);
        let pid = started_child
            .recv_timeout(Duration::from_secs(3))
            .map_err(|error| error.to_string())??;
        // Status probes can own children even when no control is pending.
        assert!(!system.as_ref().ok_or("missing worker")?.pending);
        let mut services = Services::default();
        let mut tor = Service::default();
        assert!(!services.prepare(&mut system, &mut tor)?);
        assert!(services.active());
        assert!(!services.completed());
        assert!(system.is_none());
        assert!(!services.prepare(&mut system, &mut tor)?);
        assert!(!reaped.load(Ordering::SeqCst));
        release.send(()).map_err(|error| error.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while !services.prepare(&mut system, &mut tor)? {
            if Instant::now() >= deadline {
                return Err("relaunch preparation did not complete".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(reaped.load(Ordering::SeqCst));
        #[cfg(target_os = "linux")]
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        #[cfg(not(target_os = "linux"))]
        assert!(crate::platform::command::process_arguments(pid)?.is_none());
        assert!(services.completed());
        services.reset();
        assert!(!services.active());
        Ok(())
    }
}
