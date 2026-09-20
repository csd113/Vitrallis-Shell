//! Hardware-independent snapshots and commands. Only the worker calls backends.
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub const SCREEN_TIMEOUTS: [u16; 7] = [0, 30, 60, 120, 300, 600, 1800];

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
    pub const fn snapped(self) -> Self {
        Self(((self.0 + 5) / 10) * 10)
    }
    pub fn step(self, up: bool) -> Self {
        Self(if up {
            ((self.0 / 10 + 1) * 10).min(100)
        } else {
            (self.0.saturating_sub(1) / 10) * 10
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
pub enum Radio {
    Wifi,
    Bluetooth,
}
impl Radio {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Radio(Radio, bool),
    Brightness(Percent),
    Volume(Percent),
    Power(Power),
    ScreenTimeout(u16),
    Timezone(usize),
    ReadTimezone,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub ip: Option<std::net::Ipv4Addr>,
    pub screen_timeout: Option<u16>,
    pub timezone: Option<String>,
    pub timezones: Vec<String>,
    pub calibration: bool,
    pub battery: Option<Percent>,
    pub charging: Option<bool>,
    pub external_power: Option<bool>,
    pub wifi: Option<Wifi>,
    pub wifi_enabled: Option<bool>,
    pub bluetooth: Option<bool>,
    pub brightness: Option<Percent>,
    pub brightness_minimum: Option<Percent>,
    pub volume: Option<Percent>,
    pub muted: Option<bool>,
    pub clock: Option<String>,
    pub power_controls: bool,
}
pub trait System: Send + 'static {
    fn initialize(&mut self) {}
    fn refresh(&mut self) -> Status;
    /// Apply a command and update only its affected fields after readback.
    fn control(&mut self, control: Control, status: &mut Status) -> Result<(), String>;
}

pub struct Update {
    pub status: Status,
    pub result: Option<Result<(), String>>,
}
struct Sample {
    status: Status,
    result: Option<(Control, Result<(), String>)>,
    started: Instant,
}
pub struct Worker {
    commands: mpsc::SyncSender<(Control, Status)>,
    updates: mpsc::Receiver<Sample>,
    _stop: mpsc::Sender<()>,
    status: Status,
    controlled: [Option<Instant>; 6],
    pub pending: bool,
    received: Instant,
}
impl Worker {
    pub fn start(mut backend: impl System + Clone) -> Result<Self, String> {
        let (commands, requests) = mpsc::sync_channel::<(Control, Status)>(1);
        let (results, updates) = mpsc::sync_channel(2);
        let (stop, stopped) = mpsc::channel();
        let mut controls = backend.clone();
        let control_results = results.clone();
        thread::Builder::new()
            .name("system-control".into())
            .spawn(move || {
                while let Ok((command, mut status)) = requests.recv() {
                    let result = controls.control(command, &mut status);
                    if control_results
                        .send(Sample {
                            status,
                            result: Some((command, result)),
                            started: Instant::now(),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            })
            .map_err(|error| format!("control worker: {error}"))?;
        thread::Builder::new()
            .name("system-status".into())
            .spawn(move || {
                backend.initialize();
                loop {
                    let started = Instant::now();
                    let status = backend.refresh();
                    if results
                        .send(Sample {
                            status,
                            result: None,
                            started,
                        })
                        .is_err()
                    {
                        return;
                    }
                    if stopped.recv_timeout(Duration::from_secs(10))
                        != Err(mpsc::RecvTimeoutError::Timeout)
                    {
                        return;
                    }
                }
            })
            .map_err(|error| format!("status worker: {error}"))?;
        Ok(Self {
            commands,
            updates,
            _stop: stop,
            status: Status::default(),
            controlled: [None; 6],
            pending: false,
            received: Instant::now(),
        })
    }
    pub fn submit(&mut self, command: Control) -> Result<(), String> {
        if self.pending {
            return Err("system control still pending".into());
        }
        self.commands
            .try_send((command, self.status.clone()))
            .map_err(|error| format!("system unavailable: {error}"))?;
        self.pending = true;
        Ok(())
    }
    fn accept(&mut self, sample: Sample) -> Update {
        let result = if let Some((command, result)) = sample.result {
            self.pending = false;
            match command {
                Control::Brightness(_) => {
                    self.status.brightness = sample.status.brightness;
                    self.controlled[0] = Some(sample.started);
                }
                Control::Volume(_) => {
                    self.status.volume = sample.status.volume;
                    self.status.muted = sample.status.muted;
                    self.controlled[1] = Some(sample.started);
                }
                Control::Radio(Radio::Wifi, _) => {
                    self.status.wifi_enabled = sample.status.wifi_enabled;
                    self.status.wifi = sample.status.wifi;
                    self.status.ip = sample.status.ip;
                    self.controlled[4] = Some(sample.started);
                }
                Control::Radio(Radio::Bluetooth, _) => {
                    self.status.bluetooth = sample.status.bluetooth;
                    self.controlled[5] = Some(sample.started);
                }
                Control::Power(_) => {}
                Control::ScreenTimeout(_) => {
                    self.status.screen_timeout = sample.status.screen_timeout;
                    self.controlled[2] = Some(sample.started);
                }
                Control::Timezone(_) | Control::ReadTimezone => {
                    self.status.timezone = sample.status.timezone;
                    self.status.clock = sample.status.clock;
                    self.controlled[3] = Some(sample.started);
                }
            }
            Some(result)
        } else {
            let mut status = sample.status;
            // A slow snapshot may have read a control before a newer write.
            // Keep that write's readback until a snapshot started after it.
            if self.controlled[0].is_some_and(|time| time >= sample.started) {
                status.brightness = self.status.brightness;
            }
            if self.controlled[1].is_some_and(|time| time >= sample.started) {
                status.volume = self.status.volume;
                status.muted = self.status.muted;
            }
            if self.controlled[2].is_some_and(|time| time >= sample.started) {
                status.screen_timeout = self.status.screen_timeout;
            }
            if self.controlled[3].is_some_and(|time| time >= sample.started) {
                status.timezone.clone_from(&self.status.timezone);
                status.clock.clone_from(&self.status.clock);
            }
            if self.controlled[4].is_some_and(|time| time >= sample.started) {
                status.wifi_enabled = self.status.wifi_enabled;
                status.wifi = self.status.wifi;
                status.ip = self.status.ip;
            }
            if self.controlled[5].is_some_and(|time| time >= sample.started) {
                status.bluetooth = self.status.bluetooth;
            }
            self.status = status;
            self.received = Instant::now();
            None
        };
        Update {
            status: self.status.clone(),
            result,
        }
    }
    pub fn update(&mut self) -> Option<Update> {
        match self.updates.try_recv() {
            Ok(sample) => Some(self.accept(sample)),
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
    #[derive(Clone)]
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
        #[derive(Clone)]
        struct Slow {
            release: Arc<std::sync::Mutex<mpsc::Receiver<()>>>,
            reads: Arc<AtomicUsize>,
        }
        impl System for Slow {
            fn refresh(&mut self) -> Status {
                self.reads.fetch_add(1, Ordering::SeqCst);
                if let Ok(release) = self.release.lock() {
                    let _ = release.recv_timeout(Duration::from_secs(30));
                }
                Status::default()
            }
            fn control(&mut self, _: Control, _: &mut Status) -> Result<(), String> {
                Ok(())
            }
        }
        let (release, wait) = mpsc::channel();
        let reads = Arc::new(AtomicUsize::new(0));
        let mut worker = Worker::start(Slow {
            release: Arc::new(std::sync::Mutex::new(wait)),
            reads: Arc::clone(&reads),
        })?;
        // These calls complete while the backend is held behind a channel barrier.
        assert!(worker.update().is_none());
        worker.submit(Control::Volume(Percent::new(10)?))?;
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
        // The command completed while refresh was still held behind the barrier.
        release.send(()).map_err(|error| error.to_string())?;
        while reads.load(Ordering::SeqCst) == 0 {
            if Instant::now() >= deadline {
                return Err("refresh did not start".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        worker.received = Instant::now()
            .checked_sub(Duration::from_secs(31))
            .ok_or("test clock")?;
        assert!(worker.stale());
        Ok(())
    }
    #[test]
    fn older_snapshots_cannot_revert_control_readback_or_refresh_its_age() -> Result<(), String> {
        let mut worker = Worker::start(Mock {
            status: Status::default(),
        })?;
        let earlier = Instant::now();
        let later = earlier + Duration::from_millis(1);
        worker.received = earlier
            .checked_sub(Duration::from_secs(31))
            .ok_or("test clock")?;
        worker.accept(Sample {
            status: Status {
                volume: Some(Percent::new(80)?),
                muted: Some(false),
                ..Status::default()
            },
            result: Some((Control::Volume(Percent::new(80)?), Ok(()))),
            started: later,
        });
        assert!(worker.stale());
        let stale = Status {
            volume: Some(Percent::new(30)?),
            muted: Some(true),
            battery: Some(Percent::new(60)?),
            ..Status::default()
        };
        let updated = worker.accept(Sample {
            status: stale.clone(),
            result: None,
            started: earlier,
        });
        assert_eq!(updated.status.volume, Some(Percent::new(80)?));
        assert_eq!(updated.status.muted, Some(false));
        assert_eq!(updated.status.battery, Some(Percent::new(60)?));
        let updated = worker.accept(Sample {
            status: stale,
            result: None,
            started: later + Duration::from_millis(1),
        });
        assert_eq!(updated.status.volume, Some(Percent::new(30)?));
        assert_eq!(updated.status.muted, Some(true));
        Ok(())
    }
    #[test]
    fn delayed_snapshots_cannot_revert_either_radio_switch() -> Result<(), String> {
        let mut worker = Worker::start(Mock {
            status: Status::default(),
        })?;
        let earlier = Instant::now();
        let later = earlier + Duration::from_millis(1);
        for radio in [Radio::Wifi, Radio::Bluetooth] {
            worker.accept(Sample {
                status: Status {
                    wifi_enabled: Some(false),
                    wifi: Some(Wifi::Off),
                    bluetooth: Some(false),
                    ..Status::default()
                },
                result: Some((Control::Radio(radio, false), Ok(()))),
                started: later,
            });
        }
        let result = worker.accept(Sample {
            status: Status {
                wifi_enabled: Some(true),
                wifi: Some(Wifi::Connected),
                bluetooth: Some(true),
                ..Status::default()
            },
            result: None,
            started: earlier,
        });
        assert_eq!(result.status.wifi_enabled, Some(false));
        assert_eq!(result.status.wifi, Some(Wifi::Off));
        assert_eq!(result.status.bluetooth, Some(false));
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
            if let Some(update) = worker.update()
                && let Some(result) = update.result
            {
                result?;
                assert_eq!(update.status.volume, Some(Percent::new(50)?));
                break;
            }
            if Instant::now() > deadline {
                return Err("worker timeout".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        worker.submit(Control::Power(Power::Shutdown))?;
        loop {
            if let Some(update) = worker.update()
                && let Some(result) = update.result
            {
                assert!(result.is_err());
                break;
            }
            if Instant::now() > deadline {
                return Err("worker timeout".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}
