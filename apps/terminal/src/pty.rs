//! Small POSIX PTY boundary. All unsafe code is limited to documented OS calls.
use std::{
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            fs::{MetadataExt, PermissionsExt},
            net::UnixStream,
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};
const LIMIT: usize = 64 * 1024;
const INPUT_LIMIT: usize = 16 * 1024;
#[derive(Default)]
struct State {
    output: Vec<u8>,
    input: Vec<u8>,
    size: Option<(u16, u16)>,
    stop: bool,
    wake_pending: bool,
    end: Option<String>,
}
pub struct Pty {
    state: Arc<Mutex<State>>,
    signal: UnixStream,
    worker: Option<thread::JoinHandle<()>>,
}
impl Pty {
    pub fn spawn(
        shell: &Path,
        args: &[std::ffi::OsString],
        rows: u16,
        cols: u16,
        sender: sdl2::event::EventSender,
    ) -> io::Result<Self> {
        // SAFETY: geteuid takes no pointers and has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return Err(io::Error::other(
                "Terminal must run as your normal user, not root",
            ));
        }
        Self::start(spawn_child(shell, args, rows, cols)?, sender)
    }
    fn start(
        (master, spawned_child): (File, Child),
        sender: sdl2::event::EventSender,
    ) -> io::Result<Self> {
        let mut child = OwnedChild {
            child: spawned_child,
            reaped: false,
        };
        let state = Arc::new(Mutex::new(State {
            output: Vec::with_capacity(LIMIT),
            input: Vec::with_capacity(INPUT_LIMIT),
            ..State::default()
        }));
        let output = Arc::clone(&state);
        let (signal, receiver) = UnixStream::pair()?;
        signal.set_nonblocking(true)?;
        receiver.set_nonblocking(true)?;
        let worker = thread::Builder::new()
            .name("terminal-pty".into())
            .stack_size(256 * 1024)
            .spawn(move || {
                let result = pump(&master, &mut child, &receiver, &output, &sender);
                drop(child);
                if let Ok(mut final_state) = output.lock() {
                    if let Err(error) = result {
                        final_state.end = Some(format!("Terminal I/O: {error}"));
                    } else if final_state.end.is_none() {
                        final_state.end = Some("Shell closed".into());
                    }
                    notify(&mut final_state, &sender);
                }
            })?;
        Ok(Self {
            state,
            signal,
            worker: Some(worker),
        })
    }
    pub fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| io::Error::other(format!("PTY state unavailable: {error}")))?;
        if state.end.is_some() {
            return Err(io::Error::other("Shell has exited"));
        }
        if state
            .input
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > INPUT_LIMIT)
        {
            return Err(io::Error::other(
                "Terminal input buffer is full; wait for the shell",
            ));
        }
        state.input.extend_from_slice(bytes);
        drop(state);
        self.signal()
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> io::Result<()> {
        self.state
            .lock()
            .map_err(|error| io::Error::other(format!("PTY state unavailable: {error}")))?
            .size = Some((rows, cols));
        self.signal()
    }
    pub fn take(&mut self, output: &mut Vec<u8>) -> io::Result<Option<String>> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| io::Error::other(format!("PTY state unavailable: {error}")))?;
        output.clear();
        std::mem::swap(output, &mut state.output);
        state.wake_pending = false;
        let end = state.end.take();
        drop(state);
        self.signal()?;
        Ok(end)
    }
    fn signal(&mut self) -> io::Result<()> {
        match self.signal.write(&[1]) {
            Ok(_) => Ok(()),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::BrokenPipe
                        | io::ErrorKind::ConnectionReset
                ) =>
            {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.stop = true;
        }
        if let Err(error) = self.signal() {
            eprintln!("PTY shutdown wake failed: {error}");
        }
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            eprintln!("PTY worker stopped unexpectedly");
        }
    }
}
struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // spawn_child creates a new session/process group. Closing the terminal
        // must also stop ordinary descendants left behind by its command.
        if self.reaped {
            return;
        }
        if let Err(error) = vitrallis_native::process::kill_child_group(&mut self.child) {
            eprintln!("PTY group cleanup: {error}");
        }
        if let Err(error) = self.child.kill()
            && error.kind() != io::ErrorKind::InvalidInput
        {
            eprintln!("PTY cleanup: {error}");
        }
        if let Err(error) = self.child.wait() {
            eprintln!("PTY reap: {error}");
        }
    }
}
fn notify(state: &mut State, sender: &sdl2::event::EventSender) {
    if !state.wake_pending {
        match vitrallis_native::ui::wake(sender) {
            Ok(()) => state.wake_pending = true,
            Err(error) => eprintln!("PTY event wake failed: {error}"),
        }
    }
}
fn pump(
    mut master: &File,
    child: &mut OwnedChild,
    mut signal: &UnixStream,
    state: &Mutex<State>,
    sender: &sdl2::event::EventSender,
) -> io::Result<()> {
    let mut buffer = [0; 8192];
    let mut exited = None;
    loop {
        let mut pending = state
            .lock()
            .map_err(|error| io::Error::other(format!("PTY state unavailable: {error}")))?;
        if pending.stop {
            return Ok(());
        }
        if let Some((rows, cols)) = pending.size.take() {
            resize(master, rows, cols)?;
        }
        let reading = pending.output.len() < LIMIT;
        let writing = !pending.input.is_empty();
        drop(pending);
        let mut fds = [
            libc::pollfd {
                fd: if reading || writing {
                    master.as_raw_fd()
                } else {
                    -1
                },
                events: if reading { libc::POLLIN } else { 0 }
                    | if writing { libc::POLLOUT } else { 0 },
                revents: 0,
            },
            libc::pollfd {
                fd: signal.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: fds is a live array of two initialized pollfd structures.
        // One-second timeout is solely to reap a shell whose descendants retain
        // the slave after its exit; no UI repaint or filesystem polling occurs.
        let result = unsafe { libc::poll(fds.as_mut_ptr(), 2, 1000) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        let [master_poll, signal_poll] = &fds;
        if signal_poll.revents != 0 {
            while signal.read(&mut buffer).is_ok_and(|n| n > 0) {}
        }
        let mut shared = state
            .lock()
            .map_err(|error| io::Error::other(format!("PTY state unavailable: {error}")))?;
        if master_poll.revents & libc::POLLOUT != 0 && !shared.input.is_empty() {
            match master.write(&shared.input) {
                Ok(n) => {
                    drop(shared.input.drain(..n));
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
        let eof = if reading
            && master_poll.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
        {
            read_output(master, &mut buffer, &mut shared, sender)?
        } else {
            false
        };
        if exited.is_none() && vitrallis_native::process::exited_unreaped(&mut child.child)? {
            exited = Some(std::time::Instant::now());
        }
        if let Some(time) = &exited {
            // Drain successive bounded batches through EOF. An exited shell's
            // surviving background job cannot hold the terminal open forever.
            if eof
                || (time.elapsed() >= std::time::Duration::from_secs(1) && shared.output.is_empty())
            {
                vitrallis_native::process::kill_child_group(&mut child.child)?;
                let status = child
                    .child
                    .try_wait()?
                    .ok_or_else(|| io::Error::other("PTY exit status disappeared"))?;
                child.reaped = true;
                shared.end = Some(format!("Shell exited: {status}"));
                notify(&mut shared, sender);
                return Ok(());
            }
        }

        if eof {
            shared.end = Some("PTY closed".into());
            notify(&mut shared, sender);
            return Ok(());
        }
        drop(shared);
    }
}
fn read_output(
    mut master: &File,
    buffer: &mut [u8],
    shared: &mut State,
    sender: &sdl2::event::EventSender,
) -> io::Result<bool> {
    let limit = buffer.len().min(LIMIT.saturating_sub(shared.output.len()));
    match master.read(
        buffer
            .get_mut(..limit)
            .ok_or_else(|| io::Error::other("invalid PTY read limit"))?,
    ) {
        Ok(0) => return Ok(true),
        Ok(n) => {
            shared.output.extend_from_slice(
                buffer
                    .get(..n)
                    .ok_or_else(|| io::Error::other("invalid PTY read length"))?,
            );
            notify(shared, sender);
        }
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
        Err(e) if e.raw_os_error() == Some(libc::EIO) => return Ok(true),
        Err(e) => return Err(e),
    }
    Ok(false)
}

fn spawn_child(
    shell: &Path,
    args: &[std::ffi::OsString],
    rows: u16,
    cols: u16,
) -> io::Result<(File, Child)> {
    let mut master_fd = -1_i32;
    let mut slave_fd = -1_i32;
    let mut size = libc::winsize {
        ws_row: rows.max(1),
        ws_col: cols.max(1),
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: valid writable descriptor outputs and a live winsize; null optional
    // name and termios arguments request the OS defaults.
    if unsafe {
        libc::openpty(
            &raw mut master_fd,
            &raw mut slave_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    } < 0_i32
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful openpty returned two newly owned descriptors.
    let master = unsafe { File::from_raw_fd(master_fd) };
    // SAFETY: slave is distinct from master and is adopted exactly once.
    let slave = unsafe { File::from_raw_fd(slave_fd) };
    for file in [&master, &slave] {
        // SAFETY: descriptors are valid and fcntl receives an integer flag.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0_i32 {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: master remains owned throughout; setting nonblocking affects only
    // this PTY master and prevents stalled children from blocking the SDL thread.
    if unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } < 0_i32 {
        return Err(io::Error::last_os_error());
    }
    let mut command = Command::new(shell);
    let _child_options = command
        .args(args)
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave));
    // SAFETY: the pre-exec closure invokes only async-signal-safe OS operations,
    // allocates nothing, and acquires no locks after fork. std has installed fd 0.
    let pre_exec = || -> io::Result<()> {
        // SAFETY: setsid is async-signal-safe and only detaches the child.
        if unsafe { libc::setsid() } < 0_i32 {
            return Err(io::Error::last_os_error());
        }
        #[cfg(target_os = "macos")]
        let request = u64::from(libc::TIOCSCTTY);
        #[cfg(not(target_os = "macos"))]
        let request = libc::TIOCSCTTY;
        // SAFETY: the child owns the slave PTY on stdin and is session leader here.
        if unsafe { libc::ioctl(libc::STDIN_FILENO, request, 0) } < 0_i32 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    };
    // SAFETY: the closure above runs post-fork and touches only the OS calls it
    // documents; it captures nothing and never allocates.
    let _session_options = unsafe { command.pre_exec(pre_exec) };
    Ok((master, command.spawn()?))
}
fn resize(master: &File, rows: u16, cols: u16) -> io::Result<()> {
    let size = libc::winsize {
        ws_row: rows.max(1),
        ws_col: cols.max(1),
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: valid owned PTY descriptor and live immutable winsize.
    if unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &raw const size) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn shell(preferred: Option<&Path>) -> io::Result<PathBuf> {
    preferred
        .into_iter()
        .chain([Path::new("/bin/bash"), Path::new("/bin/sh")])
        .find(|path| {
            path.is_absolute()
                && std::fs::metadata(path).is_ok_and(|m| {
                    m.is_file() && m.permissions().mode() & 0o111 != 0 && m.mode() & 0o6000 == 0
                })
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::other("No executable shell found in SHELL, /bin/bash, or /bin/sh")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_resolution_rejects_relative_directories_and_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(shell(Some(Path::new("/bin/sh")))?, Path::new("/bin/sh"));
        for path in ["relative", "/", "/vitrallis-missing-shell"] {
            assert!(
                shell(Some(Path::new(path)))
                    .is_ok_and(|p| p == Path::new("/bin/bash") || p == Path::new("/bin/sh"))
            );
        }
        Ok(())
    }
    #[test]
    fn pty_command_resize_output_eof_and_reaping() -> io::Result<()> {
        let (mut master, mut child) = spawn_child(
            Path::new("/bin/sh"),
            &[
                "-c".into(),
                "printf 'vitrallis-pty-ok\\n'; stty size".into(),
            ],
            11,
            37,
        )?;
        resize(&master, 11, 37)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut output = Vec::new();
        let mut bytes = [0; 1024];
        loop {
            match master.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(
                    bytes
                        .get(..n)
                        .ok_or_else(|| io::Error::other("invalid PTY read length"))?,
                ),
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        child.kill()?;
                        let _exit_status = child.wait()?;
                        return Err(io::Error::other("PTY smoke timed out"));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(e) => return Err(e),
            }
        }
        assert!(child.wait()?.success());
        let output_text = String::from_utf8_lossy(&output);
        assert!(output_text.contains("vitrallis-pty-ok"), "{output_text}");
        assert!(output_text.contains("11 37"), "{output_text}");
        assert!(child.try_wait()?.is_some());
        Ok(())
    }
    #[test]
    fn nonexistent_shell_reports_spawn_error() {
        assert!(spawn_child(Path::new("/vitrallis-no-shell"), &[], 10, 40).is_err());
    }
    #[test]
    fn worker_drains_flood_through_eof_with_bounded_batches()
    -> Result<(), Box<dyn std::error::Error>> {
        let sdl = sdl2::init()?;
        let events = sdl.event()?;
        let child = spawn_child(
            Path::new("/bin/sh"),
            &[
                "-c".into(),
                "dd if=/dev/zero bs=8192 count=64 2>/dev/null; printf finished".into(),
            ],
            28,
            60,
        )?;
        let mut pty = Pty::start(child, events.event_sender())?;
        let mut output = Vec::with_capacity(LIMIT);
        let mut total = 0;
        let mut tail: Vec<u8> = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let end = pty.take(&mut output)?;
            assert!(output.len() <= LIMIT);
            total += output.len();
            tail.extend(output.iter().filter(|&&byte| byte != 0));
            if end.is_some() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err("PTY flood timed out".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(total, 512 * 1024 + b"finished".len());
        assert_eq!(tail, b"finished");
        drop(pty);
        Ok(())
    }
}
