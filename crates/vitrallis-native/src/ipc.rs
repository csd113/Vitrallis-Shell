//! Bounded local open requests. The launcher's private socket directory scopes a session.
use std::{
    fs, io,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, FileTypeExt, MetadataExt},
        net::UnixDatagram,
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
const LIMIT: usize = 8192;
pub const ENV: &str = "VITRALLIS_NATIVE_BROKER";
/// A single supervisor-side listener. Native apps can request Notepad through it.
pub struct Broker {
    pub path: PathBuf,
    socket: UnixDatagram,
}
impl Broker {
    /// # Errors
    /// Reports private socket directory creation/binding errors.
    pub fn new() -> io::Result<Self> {
        let root = std::env::temp_dir();
        for n in 0..32 {
            let directory = root.join(format!("vitrallis-native-{}-{n}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let path = directory.join("shell");
                    let result = UnixDatagram::bind(&path).and_then(|socket| {
                        socket.set_nonblocking(true)?;
                        Ok(socket)
                    });
                    return match result {
                        Ok(socket) => Ok(Self { path, socket }),
                        Err(error) => {
                            let _ = fs::remove_file(&path);
                            let _ = fs::remove_dir(&directory);
                            Err(error)
                        }
                    };
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other(
            "Unable to allocate native application sockets",
        ))
    }
    /// # Errors
    /// Rejects malformed, oversized, nonabsolute requests; never accepts a command string.
    pub fn receive(&self) -> io::Result<Option<PathBuf>> {
        let mut bytes = [0; LIMIT + 1];
        match self.socket.recv(&mut bytes) {
            Ok(count) if count > 0 && count <= LIMIT => decode(&bytes[..count]).map(Some),
            Ok(_) => Err(io::Error::other("Invalid native open request")),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
}
/// # Errors
/// Reports an absent or unresponsive existing Notepad inbox.
pub fn forward(broker: &Path, path: &Path) -> io::Result<()> {
    let parent = directory(broker)?;
    send(&parent.join("notepad"), path)
}
/// Remove a crashed instance's inbox only after its process has been reaped.
/// # Errors
/// Reports unexpected inbox cleanup errors.
pub fn clear_inbox(broker: &Path, name: &str) -> io::Result<()> {
    valid_name(name)?;
    let parent = directory(broker)?;
    match fs::remove_file(parent.join(name)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::remove_file(&self.path);
            for name in ["terminal", "notepad", "files"] {
                let _ = fs::remove_file(parent.join(name));
            }
            let _ = fs::remove_dir(parent);
        }
    }
}
fn decode(bytes: &[u8]) -> io::Result<PathBuf> {
    if bytes.contains(&0) {
        return Err(io::Error::other("Invalid path request"));
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()));
    if !path.is_absolute() {
        return Err(io::Error::other("Open request must use an absolute path"));
    }
    Ok(path)
}
fn send(socket: &Path, path: &Path) -> io::Result<()> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() > LIMIT || !path.is_absolute() {
        return Err(io::Error::other("Invalid open path"));
    }
    let sender = UnixDatagram::unbound()?;
    sender.set_nonblocking(true)?;
    sender.send_to(bytes, socket)?;
    Ok(())
}
/// # Errors
/// Reports an unavailable supervisor. Returns false only outside a shell session.
pub fn request_notepad(path: &Path) -> io::Result<bool> {
    if let Some(socket) = std::env::var_os(ENV) {
        directory(Path::new(&socket))?;
        send(Path::new(&socket), path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
/// # Errors
/// Sends a focus request to a known native process's private inbox.
pub fn focus(path: &Path) -> io::Result<()> {
    let sender = UnixDatagram::unbound()?;
    sender.set_nonblocking(true)?;
    sender.send_to(b"", path)?;
    Ok(())
}
/// Ask a native app to close only if it can do so without losing work.
/// Delivery is not an acknowledgement; process ownership is retained until exit.
/// # Errors
/// Reports an absent native inbox or failed delivery.
pub fn close_if_safe(path: &Path) -> io::Result<()> {
    let sender = UnixDatagram::unbound()?;
    sender.set_nonblocking(true)?;
    sender.send_to(b"close-if-safe", path)?;
    Ok(())
}
/// One blocking IPC thread only when launched by Vitrallis. Requests never repaint while idle.
pub struct Inbox {
    close_requested: Arc<AtomicBool>,
    receiver: mpsc::Receiver<PathBuf>,
    overflow: Arc<AtomicBool>,
    path: PathBuf,
    stopped: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Inbox {
    /// # Errors
    /// Reports socket/thread errors. Direct invocation needs no inbox/thread.
    pub fn new(
        name: &str,
        sender: sdl2::event::EventSender,
        focus: Arc<AtomicBool>,
    ) -> io::Result<Option<Self>> {
        let Some(broker) = std::env::var_os(ENV) else {
            return Ok(None);
        };
        Self::bind(Path::new(&broker), name, sender, focus).map(Some)
    }
    fn bind(
        broker: &Path,
        name: &str,
        sender: sdl2::event::EventSender,
        focus: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        valid_name(name)?;
        let path = directory(broker)?.join(name);
        let socket = UnixDatagram::bind(&path)?;
        // A datagram wakes this blocking recv immediately; the bounded wait only
        // guarantees that a failed wake (for example a stale inbox removed by the
        // launcher) cannot hang Inbox::drop's join forever.
        socket.set_read_timeout(Some(Duration::from_secs(1)))?;
        let (send, receiver) = mpsc::sync_channel(8);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopped);
        let overflow = Arc::new(AtomicBool::new(false));
        let dropped = Arc::clone(&overflow);
        let close_requested = Arc::new(AtomicBool::new(false));
        let close = Arc::clone(&close_requested);
        let worker = thread::Builder::new()
            .name("native-inbox".into())
            .stack_size(128 * 1024)
            .spawn(move || {
                let mut bytes = [0; LIMIT + 1];
                loop {
                    match socket.recv(&mut bytes) {
                        Ok(count) => {
                            if stop.load(Ordering::Acquire) {
                                break;
                            }
                            if &bytes[..count] == b"close-if-safe" {
                                close.store(true, Ordering::Release);
                                let _ = crate::ui::wake(&sender);
                                continue;
                            }
                            if count == 0 {
                                if !focus.swap(true, Ordering::AcqRel) {
                                    let _ = crate::ui::wake(&sender);
                                }
                                continue;
                            }
                            if count > LIMIT {
                                continue;
                            }
                            if let Ok(path) = decode(&bytes[..count]) {
                                let notify = send.try_send(path).is_ok()
                                    || !dropped.swap(true, Ordering::AcqRel);
                                if notify && let Err(e) = crate::ui::wake(&sender) {
                                    eprintln!("Notepad wake: {e}");
                                }
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                            ) =>
                        {
                            if stop.load(Ordering::Acquire) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                let _ = fs::remove_file(&path);
                return Err(error);
            }
        };
        Ok(Self {
            close_requested,
            receiver,
            overflow,
            path,
            stopped,
            worker: Some(worker),
        })
    }
    /// Consume an advisory automatic close. Apps must veto while busy, focused,
    /// or holding unsaved work; unsupported apps simply ignore this request.
    #[must_use]
    pub fn take_close_request(&self) -> bool {
        self.close_requested.swap(false, Ordering::AcqRel)
    }
    /// # Errors
    /// Reports request overflow instead of silently dropping an open request.
    pub fn receive(&self) -> Result<Option<PathBuf>, String> {
        if self.overflow.swap(false, Ordering::AcqRel) {
            return Err("Notepad open queue is full; an additional file was not opened".into());
        }
        Ok(self.receiver.try_recv().ok())
    }
}
fn valid_name(name: &str) -> io::Result<()> {
    if matches!(name, "terminal" | "notepad" | "files") {
        Ok(())
    } else {
        Err(io::Error::other("Unknown native inbox"))
    }
}
fn directory(broker: &Path) -> io::Result<&Path> {
    if !broker.is_absolute() || broker.file_name() != Some(std::ffi::OsStr::new("shell")) {
        return Err(io::Error::other("Invalid native broker path"));
    }
    let parent = broker
        .parent()
        .ok_or_else(|| io::Error::other("Missing broker directory"))?;
    let metadata = fs::symlink_metadata(parent)?;
    let socket = fs::symlink_metadata(broker)?;
    // SAFETY: geteuid has no arguments, allocation, or failure mode.
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir()
        || metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
        || !socket.file_type().is_socket()
        || socket.uid() != uid
    {
        return Err(io::Error::other(
            "Native broker must be a private socket owned by this user",
        ));
    }
    Ok(parent)
}
impl Drop for Inbox {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(sender) = UnixDatagram::unbound() {
            let _ = sender.send_to(b"", &self.path);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
pub(crate) fn test_private_inbox(sdl: &sdl2::Sdl) -> Result<(), String> {
    fn check(sdl: &sdl2::Sdl) -> Result<(), Box<dyn std::error::Error>> {
        use std::time::{Duration, Instant};
        let broker = Broker::new()?;
        let raised = Arc::new(AtomicBool::new(false));
        let inbox = Inbox::bind(
            &broker.path,
            "notepad",
            sdl.event()?.event_sender(),
            Arc::clone(&raised),
        )?;
        assert!(
            Inbox::bind(
                &broker.path,
                "../outside",
                sdl.event()?.event_sender(),
                Arc::clone(&raised)
            )
            .is_err()
        );
        let path = Path::new("/a path/with ' quotes and $dollars.txt");
        send(&broker.path, path)?;
        assert_eq!(broker.receive()?, Some(path.to_path_buf()));
        forward(&broker.path, path)?;
        focus(&inbox.path)?;
        close_if_safe(&inbox.path)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut received = None;
        while received.is_none()
            || !raised.load(Ordering::Acquire)
            || !inbox.close_requested.load(Ordering::Acquire)
        {
            if received.is_none() {
                received = inbox.receive()?;
            }
            if Instant::now() >= deadline {
                return Err("Inbox timed out".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(received, Some(path.to_path_buf()));
        assert!(send(&broker.path, Path::new("relative")).is_err());
        assert!(decode(b"/bad\0path").is_err());
        assert_eq!(inbox.receive()?, None);
        assert!(inbox.take_close_request());
        assert!(!inbox.take_close_request());
        let socket = inbox.path.clone();
        drop(inbox);
        assert!(!socket.exists());
        let directory = broker.path.parent().ok_or("No parent")?.to_path_buf();
        drop(broker);
        assert!(!directory.exists());
        Ok(())
    }
    // A dropped inbox must stop even when its wake datagram cannot be delivered
    // (the launcher can remove a stale inbox path before the old instance exits).
    fn stale_inbox_drop(sdl: &sdl2::Sdl) -> Result<(), Box<dyn std::error::Error>> {
        let broker = Broker::new()?;
        let inbox = Inbox::bind(
            &broker.path,
            "notepad",
            sdl.event()?.event_sender(),
            Arc::new(AtomicBool::new(false)),
        )?;
        fs::remove_file(&inbox.path)?;
        let (send, receive) = mpsc::channel();
        thread::Builder::new()
            .name("inbox-drop-test".into())
            .spawn(move || {
                drop(inbox);
                let _ = send.send(());
            })?;
        receive
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| io::Error::other("Inbox drop hung after the wake datagram failed"))?;
        Ok(())
    }
    check(sdl).map_err(|e| e.to_string())?;
    stale_inbox_drop(sdl).map_err(|e| e.to_string())
}
