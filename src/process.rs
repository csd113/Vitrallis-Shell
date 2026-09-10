use crate::app::AppEntry;
use std::{
    io,
    process::{Child, Command, ExitStatus, Stdio},
};

pub fn command(app: &AppEntry) -> Result<Command, String> {
    app.validate()?;
    let m = &app.manifest;
    if let Some(parent) = m.entry.parent() {
        if parent
            .join(".installation-pending")
            .try_exists()
            .map_err(|error| format!("cannot check installation state: {error}"))?
        {
            return Err("installation incomplete; use Store to repair this app".into());
        }
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
    fn poll(&mut self) -> Result<Option<ExitStatus>, String>;
    fn focus(&mut self) -> Result<(), String> {
        Err("application focus is unavailable".into())
    }
}

/// Each application keeps its own existing process owner. Window focus never
/// transfers process ownership, and dropping the set cleans every owned child.
#[derive(Debug, Default)]
pub struct ProcessSet<P = NativeProcess> {
    members: Vec<(String, P)>,
    active: Option<String>,
    pub exited_active: bool,
}
impl<P> ProcessSet<P> {
    pub fn has_children(&self) -> bool {
        !self.members.is_empty()
    }
    pub fn running_ids(&self) -> Vec<String> {
        self.members.iter().map(|(id, _)| id.clone()).collect()
    }
}
impl<P: Processes + Default> Processes for ProcessSet<P> {
    fn start(&mut self, app: &AppEntry) -> Result<(), String> {
        if let Some((_, process)) = self.members.iter_mut().find(|(id, _)| id == &app.id) {
            process.focus()?;
        } else {
            let mut process = P::default();
            process.start(app)?;
            self.members.push((app.id.clone(), process));
        }
        self.active = Some(app.id.clone());
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        for index in 0..self.members.len() {
            if let Some(status) = self.members[index].1.poll()? {
                let (id, _) = self.members.remove(index);
                self.exited_active = self.active.as_ref() == Some(&id);
                if self.exited_active {
                    self.active = None;
                }
                return Ok(Some(status));
            }
        }
        Ok(None)
    }
    fn focus(&mut self) -> Result<(), String> {
        self.members
            .iter_mut()
            .find(|(id, _)| Some(id) == self.active.as_ref())
            .ok_or("no active app")?
            .1
            .focus()
    }
}

#[derive(Debug, Default)]
pub struct NativeProcess {
    child: Option<Child>,
    window_hint: Option<crate::platform::AppWindow>,
}
impl Processes for NativeProcess {
    fn focus(&mut self) -> Result<(), String> {
        let child = self.child.as_ref().ok_or("no running app")?;
        let result = crate::platform::focus_application(child.id(), self.window_hint);
        eprintln!(
            "level=info event=app_resume pid={} result={result:?}",
            child.id()
        );
        result
    }
    fn start(&mut self, app: &AppEntry) -> Result<(), String> {
        if self.child.is_some() {
            return Err("a child is already running".into());
        }
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
            if let Err(error) = child.kill() {
                if error.kind() != io::ErrorKind::InvalidInput {
                    eprintln!("level=error event=child_kill message={error:?}");
                }
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
fn cleanup_group(pid: u32) {
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
