//! Non-reaping child status checks for safe process-group cleanup.
use std::{io, process::Child};

/// Check whether an owned child exited while retaining its PID until it is reaped.
///
/// The caller must serialize this check, group cleanup, and `Child::wait` with
/// any other waiter. In particular, no SIGCHLD handler may reap owned children.
///
/// # Errors
/// Returns the OS error if the child is no longer waitable or cannot be queried.
pub fn exited_unreaped(child: &mut Child) -> io::Result<bool> {
    let pid = libc::id_t::try_from(child.id()).map_err(io::Error::other)?;
    loop {
        // SAFETY: siginfo_t contains C scalar/union fields for which an all-zero
        // representation is valid. Zero si_pid also represents no pending exit.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: info is a live writable siginfo_t; P_PID selects only this
        // owned child. WNOWAIT preserves the waitable child and reserves its PID.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid,
                &raw mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            // SAFETY: successful waitid initialized the SIGCHLD payload, or left
            // the zero PID when WNOHANG found no exit. No other union is read.
            return Ok(unsafe { info.si_pid() } != 0);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Stop the process group led by an owned, unreaped child without spawning a helper.
///
/// The caller must serialize this operation with all child waiters. Commands
/// intended for group cleanup should be launched with a new process group or
/// session; descendants that create separate sessions are outside this scope.
///
/// # Errors
/// Returns an error if child ownership cannot be confirmed or signalling fails.
pub fn kill_child_group(child: &mut Child) -> io::Result<()> {
    // A successful non-reaping query reserves the leader PID until we finish
    // signaling its group. No other waiter owns this child.
    let _exited = exited_unreaped(child)?;
    let pid = i32::try_from(child.id()).map_err(io::Error::other)?;
    let group = pid
        .checked_neg()
        .filter(|group| *group < 0_i32)
        .ok_or_else(|| io::Error::other("invalid child process group"))?;
    // SAFETY: the negative ID names only the owned child's process group;
    // its unreaped leader still reserves the PID, preventing PID reuse.
    if unsafe { libc::kill(group, libc::SIGKILL) } < 0_i32 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}
/// An open Linux process reference that cannot signal a reused PID.
#[cfg(target_os = "linux")]
pub struct ProcessHandle(std::os::fd::OwnedFd);

#[cfg(target_os = "linux")]
impl ProcessHandle {
    /// Acquire the reference before checking process identity.
    ///
    /// # Errors
    /// Reports invalid PIDs, unavailable kernel support, and descriptor errors.
    /// A process that already disappeared returns `None`.
    pub fn open(pid: u32) -> io::Result<Option<Self>> {
        use std::os::fd::FromRawFd;
        let process_id = libc::pid_t::try_from(pid)
            .ok()
            .filter(|value| *value > 0_i32)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid process ID"))?;
        // SAFETY: pidfd_open takes only a positive scalar PID and unsigned flags;
        // it accesses no user memory. Zero flags request a close-on-exec handle.
        let result = unsafe { libc::syscall(libc::SYS_pidfd_open, process_id, 0_u32) };
        if result < 0 {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let descriptor = i32::try_from(result).map_err(io::Error::other)?;
        // SAFETY: the successful syscall returns a new, uniquely owned file
        // descriptor (kernel descriptors fit int). OwnedFd closes it exactly once.
        Ok(Some(Self(unsafe {
            std::os::fd::OwnedFd::from_raw_fd(descriptor)
        })))
    }

    /// Request graceful termination of this process, even if its PID was reused.
    ///
    /// # Errors
    /// Reports permission or kernel errors. An already exited process is success.
    pub fn terminate(&self) -> io::Result<()> {
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor remains owned throughout the call. SIGTERM is
        // valid, the null siginfo requests the standard signal payload, and zero
        // flags select only the process referenced by this handle.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                libc::SIGTERM,
                std::ptr::null::<libc::siginfo_t>(),
                0_u32,
            )
        };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command,
        time::{Duration, Instant},
    };

    #[test]
    fn group_cleanup_keeps_wait_ownership_and_rejects_reaped_children() -> io::Result<()> {
        use std::os::unix::process::{CommandExt, ExitStatusExt};
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exec sleep 30"])
            .process_group(0)
            .spawn()?;
        kill_child_group(&mut child)?;
        assert_eq!(child.wait()?.signal(), Some(libc::SIGKILL));
        assert!(
            kill_child_group(&mut child).is_err(),
            "group cleanup requires a still-owned, unreaped child"
        );
        Ok(())
    }

    #[test]
    fn exit_probe_retains_wait_status_until_explicit_reaping() -> io::Result<()> {
        let mut child = Command::new("/bin/sh").args(["-c", "exit 7"]).spawn()?;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(5))
            .ok_or_else(|| io::Error::other("test deadline overflow"))?;
        while !exited_unreaped(&mut child)? {
            if Instant::now() >= deadline {
                child.kill()?;
                let _status = child.wait()?;
                return Err(io::Error::other("child did not exit before deadline"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            exited_unreaped(&mut child)?,
            "the exit probe must not reap the child"
        );
        assert_eq!(child.wait()?.code(), Some(7_i32));
        assert!(
            exited_unreaped(&mut child).is_err(),
            "a reaped PID must never authorize group cleanup"
        );
        Ok(())
    }
}
