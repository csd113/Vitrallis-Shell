use crate::{app::AppEntry, platform::FocusResult};
use std::{
    io,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub fn command(app: &AppEntry) -> Result<Command, String> {
    app.validate()?;
    let m = &app.manifest;
    if let Some(parent) = m.entry.parent()
        && parent
            .join(".installation-pending")
            .symlink_metadata()
            .map(|_| true)
            .or_else(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(error)
                }
            })
            .map_err(|error| format!("cannot check installation state: {error}"))?
    {
        return Err("installation incomplete; use App Center to repair this app".into());
    }
    let mut command = Command::new(m.runtime.as_ref().unwrap_or(&m.entry));
    if m.runtime.is_some() {
        command.arg(&m.entry);
    }
    command.envs(&m.env);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .args(&app.manifest.args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(cwd) = &app.manifest.cwd {
        command.current_dir(cwd);
    }
    Ok(command)
}

pub trait Processes {
    fn start(&mut self, app: &AppEntry) -> Result<(), String>;
    fn deliver(&mut self, _app: &AppEntry) -> Result<(), String> {
        Ok(())
    }
    fn waits_for_window(&self) -> bool {
        false
    }
    fn poll(&mut self) -> Result<Option<ExitStatus>, String>;
    fn focus(&mut self) -> Result<(), String> {
        Err("application focus is unavailable".into())
    }
    fn poll_focus(&mut self) -> Result<Option<FocusResult>, String> {
        Ok(None)
    }
}

/// Each application keeps its own existing process owner. Window focus never
/// transfers process ownership, and dropping the set cleans every owned child.
#[derive(Debug, Default)]
pub struct ProcessSet<P = NativeProcess> {
    members: Vec<(String, P)>,
    active: Option<String>,
    pub exited_active: bool,
    resume: Option<Resume>,
}
#[derive(Debug)]
struct Resume {
    app: AppEntry,
    deadline: Instant,
    next_attempt: Instant,
    relaunch_on_exit: bool,
    last_error: Option<String>,
}
impl<P> ProcessSet<P> {
    pub const fn has_children(&self) -> bool {
        !self.members.is_empty()
    }
    pub fn running_ids(&self) -> Vec<String> {
        self.members.iter().map(|(id, _)| id.clone()).collect()
    }
}
impl ProcessSet<NativeProcess> {
    pub(crate) fn terminate(&mut self, id: &str) -> bool {
        let Some(index) = self.members.iter().position(|(member, _)| member == id) else {
            return false;
        };
        if self
            .resume
            .as_ref()
            .is_some_and(|resume| resume.app.id == id)
        {
            self.resume = None;
        }
        if self.active.as_deref() == Some(id) {
            self.active = None;
        }
        // Dropping the owner kills its process group and reaps the direct child.
        self.members.remove(index);
        true
    }
}
impl<P: Processes + Default> Processes for ProcessSet<P> {
    fn start(&mut self, app: &AppEntry) -> Result<(), String> {
        if let Some(index) = self.members.iter().position(|(id, _)| id == &app.id) {
            // A window can close before the next scheduled child poll. Reap it
            // now so activation does not try to resume an already exited app.
            if self.members[index].1.poll()?.is_none() {
                self.members[index].1.deliver(app)?;
                self.members[index].1.focus()?;
                self.active = Some(app.id.clone());
                self.resume = Some(Resume {
                    app: app.clone(),
                    deadline: Instant::now() + Duration::from_secs(30),
                    next_attempt: Instant::now() + Duration::from_millis(250),
                    relaunch_on_exit: true,
                    last_error: None,
                });
                return Ok(());
            }
            self.members.remove(index);
        }
        self.resume = None;
        let mut process = P::default();
        process.start(app)?;
        if process.waits_for_window() {
            process.focus()?;
            self.resume = Some(Resume {
                app: app.clone(),
                deadline: Instant::now() + Duration::from_secs(30),
                next_attempt: Instant::now() + Duration::from_millis(250),
                relaunch_on_exit: false,
                last_error: None,
            });
        }
        self.members.push((app.id.clone(), process));
        self.active = Some(app.id.clone());
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        let mut failure = None;
        for index in 0..self.members.len() {
            // A pending activation owns its exit/restart transition. Do not reap
            // it here and lose the user's request between window close and exit.
            if self.resume.as_ref().is_some_and(|resume| {
                resume.relaunch_on_exit && resume.app.id == self.members[index].0
            }) {
                continue;
            }
            let result = self.members[index].1.poll();
            if let Err(error) = result {
                failure = Some(error);
            } else if let Ok(Some(status)) = result {
                let (id, _) = self.members.remove(index);
                if self
                    .resume
                    .as_ref()
                    .is_some_and(|resume| resume.app.id == id)
                {
                    self.resume = None;
                }
                self.exited_active = self.active.as_ref() == Some(&id);
                if self.exited_active {
                    self.active = None;
                }
                return Ok(Some(status));
            }
        }
        failure.map_or(Ok(None), Err)
    }
    fn focus(&mut self) -> Result<(), String> {
        self.members
            .iter_mut()
            .find(|(id, _)| Some(id) == self.active.as_ref())
            .ok_or("no active app")?
            .1
            .focus()
    }
    fn poll_focus(&mut self) -> Result<Option<FocusResult>, String> {
        let Some(resume) = &self.resume else {
            return Ok(None);
        };
        let Some(index) = self.members.iter().position(|(id, _)| id == &resume.app.id) else {
            self.resume = None;
            return Ok(None);
        };
        if resume.relaunch_on_exit && self.members[index].1.poll()?.is_some() {
            self.members.remove(index);
            let Some(resume) = self.resume.take() else {
                return Ok(None);
            };
            // Only spawn after the former child has been reaped and its group
            // cleaned. A disappearing window alone never authorizes a duplicate.
            self.start(&resume.app)?;
            return Ok(Some(FocusResult::Focused));
        }
        match self.members[index].1.poll_focus() {
            Ok(Some(FocusResult::Focused)) => {
                self.resume = None;
                Ok(Some(FocusResult::Focused))
            }
            result => {
                let Some(resume) = &mut self.resume else {
                    return Ok(None);
                };
                match result {
                    Err(error) => resume.last_error = Some(error),
                    Ok(Some(_)) => resume.last_error = None,
                    Ok(None) => {}
                }
                if Instant::now() >= resume.deadline {
                    let error = resume.last_error.take();
                    self.resume = None;
                    return error.map_or(Ok(Some(FocusResult::Missing)), Err);
                }
                if Instant::now() >= resume.next_attempt {
                    self.members[index].1.focus()?;
                    if let Some(resume) = &mut self.resume {
                        resume.next_attempt = Instant::now() + Duration::from_millis(250);
                    }
                }
                Ok(None)
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct NativeProcess {
    child: Option<Child>,
    window_hint: Option<crate::platform::AppWindow>,
    focus_result: Option<mpsc::Receiver<Result<FocusResult, String>>>,
    native_socket: Option<std::path::PathBuf>,
}
impl Processes for NativeProcess {
    fn deliver(&mut self, app: &AppEntry) -> Result<(), String> {
        if app.source == crate::app::AppSource::Native
            && app.id == "io.vitrallis.notepad"
            && !app.manifest.args.is_empty()
        {
            let path = app.manifest.args.get(1).ok_or("Missing native open path")?;
            let broker = app
                .manifest
                .env
                .get(std::ffi::OsStr::new(vitrallis_native::ipc::ENV))
                .ok_or("Native request service is unavailable")?;
            vitrallis_native::ipc::forward(
                std::path::Path::new(broker),
                std::path::Path::new(path),
            )
            .map_err(|e| format!("Notepad cannot accept the file yet: {e}"))?;
        }
        Ok(())
    }

    fn waits_for_window(&self) -> bool {
        self.native_socket.is_some()
            || std::env::var_os("VITRALLIS_SESSION").as_deref() == Some(std::ffi::OsStr::new("1"))
                && !matches!(
                    self.window_hint,
                    Some(crate::platform::AppWindow::Calibration)
                )
    }
    fn focus(&mut self) -> Result<(), String> {
        let child = self.child.as_ref().ok_or("no running app")?;
        if self.focus_result.is_some() {
            return Ok(());
        }
        let pid = child.id();
        let hint = self.window_hint;
        let native = self.native_socket.clone();
        self.focus_result = Some(request_focus(move || {
            let result = native.map_or_else(
                || crate::platform::focus_application(pid, hint),
                |path| match vitrallis_native::ipc::focus(&path) {
                    Ok(()) => Ok(FocusResult::Focused),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                        ) =>
                    {
                        Ok(FocusResult::Missing)
                    }
                    Err(e) => Err(e.to_string()),
                },
            );
            eprintln!("level=info event=app_resume pid={pid} result={result:?}");
            result
        })?);
        Ok(())
    }
    fn poll_focus(&mut self) -> Result<Option<FocusResult>, String> {
        let Some(result) = &self.focus_result else {
            return Ok(None);
        };
        let result = match result.try_recv() {
            Ok(result) => result.map(Some),
            Err(mpsc::TryRecvError::Empty) => return Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err("focus worker stopped".into()),
        };
        self.focus_result = None;
        result
    }
    fn start(&mut self, app: &AppEntry) -> Result<(), String> {
        if self.child.is_some() {
            return Err("a child is already running".into());
        }
        self.native_socket = None;
        self.focus_result = None;
        if let Some(reason) = &app.unavailable {
            return Err(format!("{}: {reason}", app.name));
        }
        eprintln!(
            "level=info event=launch_request app={} name={:?} runtime={:?} entry={:?} args={:?} cwd={:?} environment_keys={:?}",
            app.id,
            app.name,
            app.manifest.runtime,
            app.manifest.entry.to_string_lossy(),
            app.manifest.args,
            app.manifest.cwd,
            app.manifest.env.keys().collect::<Vec<_>>()
        );
        if app.source == crate::app::AppSource::Native
            && let Some(native) = vitrallis_native::APPLICATIONS
                .iter()
                .find(|native| native.id == app.id)
            && let Some(broker) = app
                .manifest
                .env
                .get(std::ffi::OsStr::new(vitrallis_native::ipc::ENV))
        {
            let name = native
                .executable
                .strip_prefix("vitrallis-")
                .ok_or("Invalid native executable identity")?;
            let broker = std::path::Path::new(broker);
            vitrallis_native::ipc::clear_inbox(broker, name).map_err(|e| e.to_string())?;
            self.native_socket = Some(
                broker
                    .parent()
                    .ok_or("Invalid native broker directory")?
                    .join(name),
            );
        }
        let child = command(app)?
            .spawn()
            .map_err(|e| format!("{}: {e}", app.name))?;
        eprintln!(
            "level=info event=app_started app={} pid={}",
            app.id,
            child.id()
        );
        self.child = Some(child);
        self.window_hint = crate::platform::AppWindow::for_entry(&app.manifest.entry);
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        let Some(child) = &mut self.child else {
            return Ok(None);
        };
        let status = child
            .try_wait()
            .map_err(|e| format!("child wait failed: {e}"))?;
        if let Some(status) = status {
            eprintln!(
                "level=info event=child_reaped pid={} status={status:?}",
                child.id()
            );
            cleanup_group(child.id());
            self.child = None;
        }
        Ok(status)
    }
}

fn request_focus(
    operation: impl FnOnce() -> Result<FocusResult, String> + Send + 'static,
) -> Result<mpsc::Receiver<Result<FocusResult, String>>, String> {
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("app-focus".into())
        .spawn(move || {
            let _ = send.send(operation());
        })
        .map_err(|error| format!("focus worker: {error}"))?;
    Ok(receive)
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Signal only the process group created for our child.
            match child.try_wait() {
                Ok(Some(_)) => {
                    cleanup_group(child.id());
                    return;
                }
                Ok(None) => {}
                Err(error) => eprintln!("level=error event=child_wait message={error:?}"),
            }
            cleanup_group(child.id());
            if let Err(error) = child.kill()
                && error.kind() != io::ErrorKind::InvalidInput
            {
                eprintln!("level=error event=child_kill message={error:?}");
            }
            if let Err(error) = child.wait() {
                eprintln!("level=error event=child_reap message={error:?}");
            }
        }
    }
}

// Apps must remain in their launcher-created group. Daemons that call setsid
// escape this scope and require a later session supervisor. The OS reaps orphaned
// descendants; this launcher always waits for its own direct child.
pub fn cleanup_group(pid: u32) {
    #[cfg(unix)]
    {
        match Command::new(crate::config::KILL_HELPER)
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => eprintln!(
                "level=info event=group_cleanup pgid={pid} helper_status={status} (nonzero_can_mean_group_already_gone)"
            ),
            Err(error) => {
                eprintln!(
                    "level=warn event=group_cleanup_unavailable pgid={pid} message={error:?}"
                );
            }
        }
    }
    #[cfg(not(unix))]
    let _ = pid;
}

/// One place coordinates the process boundary with renderer-independent state.
pub fn activate(
    state: &mut crate::launcher::Launcher,
    processes: &mut impl Processes,
    index: usize,
) {
    if state.phase != crate::launcher::Phase::Launching {
        return;
    }
    let Some(app) = state.apps.get(index) else {
        state.failed("Invalid app index".into());
        return;
    };
    match processes.start(app) {
        Ok(()) => state.started(),
        Err(error) => {
            eprintln!("level=error event=launch_failed message={error:?}");
            state.failed(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    fn app() -> AppEntry {
        AppEntry {
            source: crate::app::AppSource::Demo,
            id: "test".into(),
            name: "Test".into(),
            icon: None,
            unavailable: None,
            manifest: crate::app::AppManifest {
                entry: PathBuf::from("/bin/sh"),
                args: vec![],
                cwd: Some(std::env::temp_dir()),
                ..crate::app::AppManifest::default()
            },
        }
    }
    #[test]
    fn terminate_kills_only_the_named_app_and_cancels_its_resume()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut first = app();
        first.manifest.args = vec!["-c".into(), "exec sleep 30".into()];
        let mut second = first.clone();
        second.id = "second".into();
        let mut processes = ProcessSet::<NativeProcess>::default();
        processes.start(&first)?;
        processes.start(&second)?;
        let pid = processes.members[0]
            .1
            .child
            .as_ref()
            .ok_or("missing child")?
            .id();
        processes.resume = Some(Resume {
            app: first.clone(),
            deadline: Instant::now(),
            next_attempt: Instant::now(),
            relaunch_on_exit: true,
            last_error: None,
        });

        assert!(!processes.terminate("not-running"));
        assert!(processes.terminate(&first.id));
        assert!(processes.resume.is_none());
        assert_eq!(processes.active.as_deref(), Some("second"));
        assert_eq!(processes.running_ids(), ["second"]);
        assert!(processes.members[0].1.poll()?.is_none());
        let result = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()?;
        assert!(result.stdout.is_empty(), "terminated child still exists");
        assert!(processes.terminate(&second.id));
        assert!(processes.active.is_none());
        assert!(!processes.has_children());
        Ok(())
    }
    #[test]
    fn command_preserves_arguments_and_working_directory() -> Result<(), String> {
        let mut app = app();
        app.manifest.args = vec!["literal ; $(echo unsafe)".into(), "two words".into()];
        let cmd = command(&app)?;
        assert_eq!(cmd.get_program(), "/bin/sh");
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), app.manifest.args);
        assert_eq!(cmd.get_current_dir(), app.manifest.cwd.as_deref());
        Ok(())
    }
    #[test]
    fn incomplete_installation_never_spawns() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let mut entry = app();
        entry.manifest.entry = scratch.0.join("launch");
        std::fs::write(scratch.0.join(".installation-pending"), "repair required")?;
        assert!(command(&entry).is_err_and(|error| error.contains("incomplete")));
        #[cfg(unix)]
        {
            std::fs::remove_file(scratch.0.join(".installation-pending"))?;
            std::os::unix::fs::symlink(
                scratch.0.join("missing"),
                scratch.0.join(".installation-pending"),
            )?;
            assert!(command(&entry).is_err_and(|error| error.contains("incomplete")));
        }
        Ok(())
    }
    #[test]
    fn slow_focus_is_nonblocking_and_reports_failure() -> Result<(), String> {
        let (release, wait) = mpsc::channel();
        let receive = request_focus(move || {
            wait.recv_timeout(Duration::from_secs(5))
                .map_err(|error| error.to_string())?;
            Err("window manager unavailable".into())
        })?;
        let mut process = NativeProcess {
            focus_result: Some(receive),
            child: None,
            window_hint: None,
            native_socket: None,
        };
        assert_eq!(process.poll_focus()?, None);
        release.send(()).map_err(|error| error.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Err(error) = process.poll_focus() {
                assert_eq!(error, "window manager unavailable");
                assert!(process.focus_result.is_none());
                break;
            }
            if Instant::now() >= deadline {
                return Err("focus worker timed out".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    #[test]
    fn home_allows_another_app_and_resume_does_not_spawn_a_duplicate() -> Result<(), String> {
        #[derive(Default)]
        struct Fake {
            starts: usize,
            focuses: usize,
            exited: bool,
        }
        impl Processes for Fake {
            fn start(&mut self, _: &AppEntry) -> Result<(), String> {
                self.starts += 1;
                Ok(())
            }
            fn focus(&mut self) -> Result<(), String> {
                self.focuses += 1;
                Ok(())
            }
            fn poll_focus(&mut self) -> Result<Option<FocusResult>, String> {
                Ok(Some(FocusResult::Focused))
            }
            fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
                use std::os::unix::process::ExitStatusExt;
                Ok(self.exited.then(|| ExitStatus::from_raw(0)))
            }
        }
        let first = app();
        let mut second = first.clone();
        second.id = "second".into();
        let mut state = crate::launcher::Launcher::new(vec![first, second], 2, 2)?;
        let mut processes = ProcessSet::<Fake>::default();
        state.input(crate::input::Action::Activate);
        activate(&mut state, &mut processes, 0);
        state.returned_home();
        state.input(crate::input::Action::SelectAndActivate(1));
        activate(&mut state, &mut processes, 1);
        assert_eq!(processes.running_ids(), ["test", "second"]);
        processes.start(&state.apps[0])?;
        assert_eq!(processes.members[0].1.starts, 1);
        assert_eq!(processes.members[0].1.focuses, 1);
        processes.poll_focus()?;
        processes.members[0].1.exited = true;
        // Activation arriving before the scheduled exit poll starts a fresh
        // process rather than dispatching focus to a dead window.
        processes.start(&state.apps[0])?;
        assert!(!processes.members[1].1.exited);
        assert_eq!(processes.members[1].1.starts, 1);
        assert_eq!(processes.members[1].1.focuses, 0);
        processes.members.swap(0, 1);
        processes.members[1].1.exited = true;
        assert!(processes.poll()?.is_some());
        assert!(!processes.exited_active);
        assert_eq!(processes.running_ids(), ["test"]);
        processes.members[0].1.exited = true;
        assert!(processes.poll()?.is_some());
        assert!(processes.exited_active);
        assert!(!processes.has_children());
        Ok(())
    }
    #[derive(Default)]
    struct ClosingWindow {
        focus_error: Option<String>,
        exited: bool,
        window_ready: bool,
        starts: usize,
        focuses: usize,
    }
    impl Processes for ClosingWindow {
        fn start(&mut self, _: &AppEntry) -> Result<(), String> {
            self.starts += 1;
            Ok(())
        }
        fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
            use std::os::unix::process::ExitStatusExt;
            Ok(self.exited.then(|| ExitStatus::from_raw(0)))
        }
        fn focus(&mut self) -> Result<(), String> {
            self.focuses += 1;
            Ok(())
        }
        fn poll_focus(&mut self) -> Result<Option<FocusResult>, String> {
            if let Some(error) = self.focus_error.take() {
                return Err(error);
            }
            Ok(Some(if self.window_ready {
                FocusResult::Focused
            } else {
                FocusResult::Missing
            }))
        }
    }
    #[test]
    fn reopen_waits_for_closing_process_then_spawns_exactly_once() -> Result<(), String> {
        let mut processes = ProcessSet::<ClosingWindow>::default();
        processes.start(&app())?;
        processes.start(&app())?;
        assert_eq!(processes.poll_focus()?, None); // Window gone; process still exits asynchronously.
        assert_eq!(processes.members.len(), 1);
        assert_eq!(processes.members[0].1.starts, 1);
        processes.members[0].1.exited = true;
        assert_eq!(processes.poll()?, None); // Preserve the pending activation.
        assert_eq!(processes.poll_focus()?, Some(FocusResult::Focused));
        assert_eq!(processes.members.len(), 1);
        assert!(!processes.members[0].1.exited);
        assert_eq!(processes.members[0].1.starts, 1);
        assert_eq!(processes.members[0].1.focuses, 0);
        assert_eq!(processes.poll_focus()?, None);
        Ok(())
    }
    #[test]
    fn delayed_window_resumes_without_restarting_and_absent_window_is_bounded() -> Result<(), String>
    {
        let mut processes = ProcessSet::<ClosingWindow>::default();
        processes.start(&app())?;
        processes.start(&app())?;
        assert_eq!(processes.poll_focus()?, None);
        processes.members[0].1.window_ready = true;
        assert_eq!(processes.poll_focus()?, Some(FocusResult::Focused));
        assert_eq!(processes.members[0].1.starts, 1);
        processes.members[0].1.window_ready = false;
        processes.start(&app())?;
        processes.resume.as_mut().ok_or("missing resume")?.deadline = Instant::now();
        assert_eq!(processes.poll_focus()?, Some(FocusResult::Missing));
        assert_eq!(processes.members.len(), 1);
        assert_eq!(processes.members[0].1.starts, 1);
        assert!(processes.resume.is_none());
        Ok(())
    }
    #[test]
    fn recovered_focus_helper_does_not_turn_a_missing_window_into_an_error() -> Result<(), String> {
        let mut processes = ProcessSet::<ClosingWindow>::default();
        processes.start(&app())?;
        processes.start(&app())?;
        processes.members[0].1.focus_error = Some("temporary transport failure".into());
        assert_eq!(processes.poll_focus()?, None);
        processes
            .resume
            .as_mut()
            .ok_or("missing activation")?
            .deadline = Instant::now();
        assert_eq!(processes.poll_focus()?, Some(FocusResult::Missing));
        assert_eq!(processes.members.len(), 1);
        Ok(())
    }
    #[test]
    fn startup_exit_is_reaped_without_automatic_restart() -> Result<(), String> {
        let mut processes = ProcessSet::<ClosingWindow>::default();
        processes.start(&app())?;
        processes.start(&app())?;
        processes
            .resume
            .as_mut()
            .ok_or("missing activation")?
            .relaunch_on_exit = false;
        processes.members[0].1.exited = true;
        assert!(processes.poll()?.is_some());
        assert!(processes.resume.is_none());
        assert!(!processes.has_children());
        assert_eq!(processes.poll_focus()?, None);
        Ok(())
    }
    #[test]
    fn real_child_is_reaped_and_launcher_recovers() -> Result<(), String> {
        let mut app = app();
        // Fixed test-only script; no external input is interpolated.
        app.manifest.args = vec!["-c".into(), "exit 7".into()];
        let mut state = crate::launcher::Launcher::new(vec![app], 1, 1)?;
        let mut process = NativeProcess::default();
        let index = state
            .input(crate::input::Action::Activate)
            .ok_or("no activation")?;
        activate(&mut state, &mut process, index);
        assert_eq!(state.phase, crate::launcher::Phase::Running);
        assert!(process.start(&state.apps[0]).is_err());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = process.poll()? {
                assert_eq!(status.code(), Some(7));
                state.finished(format!("{status}"));
                break;
            }
            if Instant::now() >= deadline {
                return Err("child did not exit".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(state.phase, crate::launcher::Phase::Ready);
        assert!(process.child.is_none());
        assert!(process.poll()?.is_none());
        Ok(())
    }
    #[test]
    fn failed_spawn_allows_retry() -> Result<(), String> {
        let mut app = app();
        app.manifest.entry = "/vitrallis-missing-test/application".into();
        let mut state = crate::launcher::Launcher::new(vec![app], 1, 1)?;
        let mut process = NativeProcess::default();
        state.input(crate::input::Action::Activate);
        activate(&mut state, &mut process, 0);
        assert_eq!(state.phase, crate::launcher::Phase::Ready);
        assert!(state.status.contains("LAUNCH FAILED"));
        assert!(process.child.is_none());
        assert_eq!(state.input(crate::input::Action::Activate), None); // Dismiss the error.
        assert_eq!(state.input(crate::input::Action::Activate), Some(0));
        Ok(())
    }
    #[test]
    fn wait_error_does_not_prevent_reaping_other_children() -> Result<(), String> {
        #[derive(Default)]
        struct Fake(bool);
        impl Processes for Fake {
            fn start(&mut self, _: &AppEntry) -> Result<(), String> {
                Ok(())
            }
            fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
                use std::os::unix::process::ExitStatusExt;
                if self.0 {
                    Err("injected wait failure".into())
                } else {
                    Ok(Some(ExitStatus::from_raw(0)))
                }
            }
        }
        let mut processes = ProcessSet {
            members: vec![
                ("failed".into(), Fake(true)),
                ("exited".into(), Fake(false)),
            ],
            active: Some("exited".into()),
            exited_active: false,
            resume: None,
        };
        assert!(processes.poll()?.is_some());
        assert!(processes.exited_active);
        assert_eq!(processes.running_ids(), ["failed"]);
        assert!(processes.poll().is_err());
        Ok(())
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::{
        app::AppManifest,
        input::Action,
        launcher::{Launcher, Phase},
    };
    use std::time::{Duration, Instant};
    fn fixture() -> AppEntry {
        AppEntry {
            source: crate::app::AppSource::Demo,
            id: "cycles".into(),
            name: "Cycles".into(),
            icon: None,
            unavailable: None,
            manifest: AppManifest {
                entry: "/bin/sh".into(),
                args: vec!["-c".into(), "exit 0".into()],
                ..AppManifest::default()
            },
        }
    }
    fn wait(process: &mut NativeProcess) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = process.poll()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err("process did not exit".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn forty_launch_exit_cycles_reap_and_recover_without_accumulating_children()
    -> Result<(), String> {
        let mut state = Launcher::new(vec![fixture()], 1, 1)?;
        let mut process = NativeProcess::default();
        for _ in 0..40 {
            assert_eq!(state.input(Action::Activate), Some(0));
            activate(&mut state, &mut process, 0);
            assert_eq!(state.phase, Phase::Running);
            assert!(state.input(Action::Activate).is_none());
            assert!(process.start(&state.apps[0]).is_err());
            assert!(wait(&mut process)?.success());
            assert!(process.child.is_none());
            assert!(process.poll()?.is_none());
            state.finished("closed".into());
            assert_eq!(state.selected, 0);
        }
        Ok(())
    }
    #[test]
    fn runtime_entry_arguments_environment_and_cwd_reach_real_child()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let script = scratch.0.join("entry.sh");
        std::fs::write(&script, "printf '%s\\n' \"$VITRALLIS_TEST\" \"$1\"; pwd\n")?;
        let mut app = fixture();
        app.manifest.runtime = Some("/bin/sh".into());
        app.manifest.entry = script;
        app.manifest.args = vec!["literal ; $(false)".into()];
        app.manifest.cwd = Some(scratch.0.clone());
        app.manifest
            .env
            .insert("VITRALLIS_TEST".into(), "per-child".into());
        let output = command(&app)?.stdout(Stdio::piped()).output()?;
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout)?;
        assert!(text.starts_with("per-child\nliteral ; $(false)\n"));
        assert!(
            text.contains(
                scratch
                    .0
                    .file_name()
                    .ok_or("missing basename")?
                    .to_str()
                    .ok_or("UTF-8")?
            )
        );
        assert!(std::env::var_os("VITRALLIS_TEST").is_none());
        Ok(())
    }
    #[test]
    fn shutdown_kills_and_reaps_owned_child() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = fixture();
        app.manifest.args = vec!["-c".into(), "exec sleep 30".into()];
        let mut process = NativeProcess::default();
        process.start(&app)?;
        let pid = process.child.as_ref().ok_or("missing child")?.id();
        drop(process);
        let result = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()?;
        assert!(
            result.stdout.is_empty(),
            "owned child still exists after shutdown"
        );
        Ok(())
    }
    #[test]
    fn wait_failure_keeps_running_guard_and_spawn_failure_is_retryable() -> Result<(), String> {
        struct WaitFailure;
        impl Processes for WaitFailure {
            fn start(&mut self, _: &AppEntry) -> Result<(), String> {
                Ok(())
            }
            fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
                Err("injected wait error".into())
            }
        }
        let mut state = Launcher::new(vec![fixture()], 1, 1)?;
        let mut process = WaitFailure;
        state.input(Action::Activate);
        activate(&mut state, &mut process, 0);
        assert!(process.poll().is_err());
        assert_eq!(state.phase, Phase::Running);
        assert!(state.input(Action::Activate).is_none());
        Ok(())
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use crate::app::{AppManifest, AppSource};
    use std::os::unix::{ffi::OsStrExt, net::UnixDatagram};

    #[test]
    fn native_focus_and_notepad_delivery_preserve_the_owned_child()
    -> Result<(), Box<dyn std::error::Error>> {
        let broker = vitrallis_native::ipc::Broker::new()?;
        for native in vitrallis_native::APPLICATIONS {
            let mut app = AppEntry {
                id: native.id.into(),
                name: native.name.into(),
                source: AppSource::Native,
                icon: None,
                unavailable: None,
                manifest: AppManifest {
                    entry: "/bin/sh".into(),
                    args: vec!["-c".into(), "exec sleep 30".into()],
                    env: [(
                        vitrallis_native::ipc::ENV.into(),
                        broker.path.clone().into_os_string(),
                    )]
                    .into(),
                    ..AppManifest::default()
                },
            };
            let mut process = NativeProcess::default();
            process.start(&app)?;
            let pid = process.child.as_ref().ok_or("Missing owned child")?.id();
            let socket =
                UnixDatagram::bind(process.native_socket.as_ref().ok_or("No native inbox")?)?;
            socket.set_read_timeout(Some(Duration::from_secs(3)))?;
            process.focus()?;
            let mut bytes = [0; 1024];
            assert_eq!(socket.recv(&mut bytes)?, 0);
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(result) = process.poll_focus()? {
                    assert_eq!(result, FocusResult::Focused);
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("Native focus timed out".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            if native.id == "io.vitrallis.notepad" {
                let path = std::path::Path::new("/notes/a file with $ and ' characters");
                app.manifest.args = vec!["--".into(), path.into()];
                process.deliver(&app)?;
                let count = socket.recv(&mut bytes)?;
                assert_eq!(&bytes[..count], path.as_os_str().as_bytes());
            }
            assert_eq!(
                process.child.as_ref().ok_or("Lost process ownership")?.id(),
                pid
            );
            assert!(process.start(&app).is_err());
            process.child.as_mut().ok_or("Missing child")?.kill()?;
            let deadline = Instant::now() + Duration::from_secs(3);
            while process.poll()?.is_none() {
                if Instant::now() >= deadline {
                    return Err("Native child was not reaped".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(process.child.is_none());
        }
        Ok(())
    }
}
