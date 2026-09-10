//! Hardware-independent snapshots and commands. Only the worker calls backends.
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Percent(u8);
impl Percent {
    pub fn new(value: u8) -> Result<Self, String> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err("percentage must be 0..100".into())
        }
    }
    pub const fn value(self) -> u8 {
        self.0
    }
    pub fn step(self, up: bool) -> Self {
        Self(if up {
            self.0.saturating_add(10).min(100)
        } else {
            self.0.saturating_sub(10)
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wifi {
    Off,
    Disconnected,
    Connecting,
    Connected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    Reboot,
    Shutdown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Brightness(Percent),
    Volume(Percent),
    Power(Power),
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub battery: Option<Percent>,
    pub charging: Option<bool>,
    pub external_power: Option<bool>,
    pub wifi: Option<Wifi>,
    // The reference has no live Bluetooth backend.
    pub bluetooth: Option<bool>,
    pub brightness: Option<Percent>,
    pub volume: Option<Percent>,
    pub muted: Option<bool>,
    pub clock: Option<String>,
    pub power_controls: bool,
}
pub trait System: Send + 'static {
    fn refresh(&mut self) -> Status;
    /// Apply a command and update only its affected fields after readback.
    fn control(&mut self, control: Control, status: &mut Status) -> Result<(), String>;
}

pub struct Update {
    pub status: Status,
    pub result: Option<Result<(), String>>,
}
pub struct Worker {
    commands: mpsc::SyncSender<Control>,
    updates: mpsc::Receiver<Update>,
    pub pending: bool,
    received: Instant,
}
impl Worker {
    pub fn start(mut backend: impl System) -> Result<Self, String> {
        let (commands, requests) = mpsc::sync_channel(1);
        let (results, updates) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("system-status".into())
            .spawn(move || {
                let mut status = backend.refresh();
                let mut next_refresh = Instant::now() + Duration::from_secs(10);
                if results
                    .send(Update {
                        status: status.clone(),
                        result: None,
                    })
                    .is_err()
                {
                    return;
                }
                loop {
                    let result = match requests
                        .recv_timeout(next_refresh.saturating_duration_since(Instant::now()))
                    {
                        Ok(command) => Some(backend.control(command, &mut status)),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    };
                    if Instant::now() >= next_refresh {
                        status = backend.refresh();
                        next_refresh = Instant::now() + Duration::from_secs(10);
                    }
                    // One outstanding snapshot provides backpressure while the UI is away.
                    if results
                        .send(Update {
                            status: status.clone(),
                            result,
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            })
            .map_err(|e| format!("system worker: {e}"))?;
        Ok(Self {
            commands,
            updates,
            pending: false,
            received: Instant::now(),
        })
    }
    pub fn submit(&mut self, command: Control) -> Result<(), String> {
        if self.pending {
            return Err("system control still pending".into());
        }
        self.commands
            .try_send(command)
            .map_err(|e| format!("system unavailable: {e}"))?;
        self.pending = true;
        Ok(())
    }
    pub fn update(&mut self) -> Option<Update> {
        match self.updates.try_recv() {
            Ok(update) => {
                if update.result.is_some() {
                    self.pending = false;
                }
                self.received = Instant::now();
                Some(update)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = false;
                None
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
    pub fn stale(&self) -> bool {
        self.received.elapsed() > Duration::from_secs(30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Mock {
        status: Status,
    }
    impl System for Mock {
        fn refresh(&mut self) -> Status {
            self.status.clone()
        }
        fn control(&mut self, control: Control, status: &mut Status) -> Result<(), String> {
            match control {
                Control::Volume(p) => {
                    self.status.volume = Some(p);
                    status.volume = Some(p);
                    Ok(())
                }
                _ => Err("unsupported".into()),
            }
        }
    }
    #[test]
    fn slow_refresh_never_blocks_the_client_and_controls_do_not_reprobe() -> Result<(), String> {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Slow {
            release: mpsc::Receiver<()>,
            reads: Arc<AtomicUsize>,
        }
        impl System for Slow {
            fn refresh(&mut self) -> Status {
                self.reads.fetch_add(1, Ordering::SeqCst);
                let _ = self.release.recv_timeout(Duration::from_secs(2));
                Status::default()
            }
            fn control(&mut self, _: Control, _: &mut Status) -> Result<(), String> {
                Ok(())
            }
        }
        let (release, wait) = mpsc::channel();
        let reads = Arc::new(AtomicUsize::new(0));
        let mut worker = Worker::start(Slow {
            release: wait,
            reads: Arc::clone(&reads),
        })?;
        // These calls complete while the backend is held behind a channel barrier.
        assert!(worker.update().is_none());
        worker.submit(Control::Volume(Percent::new(10)?))?;
        release.send(()).map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if worker
                .update()
                .is_some_and(|update| update.result.is_some())
            {
                break;
            }
            if Instant::now() >= deadline {
                return Err("worker timeout".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        worker.received = Instant::now()
            .checked_sub(Duration::from_secs(31))
            .ok_or("test clock")?;
        assert!(worker.stale());
        Ok(())
    }
    #[test]
    fn ranges_and_worker_mock() -> Result<(), String> {
        assert!(Percent::new(101).is_err());
        assert_eq!(Percent::new(0)?.step(false).value(), 0);
        assert_eq!(Percent::new(99)?.step(true).value(), 100);
        let mut worker = Worker::start(Mock {
            status: Status::default(),
        })?;
        worker.submit(Control::Volume(Percent::new(50)?))?;
        assert!(worker.submit(Control::Power(Power::Shutdown)).is_err());
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(update) = worker.update() {
                if let Some(result) = update.result {
                    result?;
                    assert_eq!(update.status.volume, Some(Percent::new(50)?));
                    break;
                }
            }
            if Instant::now() > deadline {
                return Err("worker timeout".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        worker.submit(Control::Power(Power::Shutdown))?;
        loop {
            if let Some(update) = worker.update() {
                if let Some(result) = update.result {
                    assert!(result.is_err());
                    break;
                }
            }
            if Instant::now() > deadline {
                return Err("worker timeout".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}
