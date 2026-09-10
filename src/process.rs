use crate::app::App;
use std::{
    io,
    process::{Child, Command, ExitStatus, Stdio},
};

pub fn command(app: &App) -> Result<Command, String> {
    app.validate()?;
    let mut command = Command::new(&app.executable);
    command
        .args(&app.args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(cwd) = &app.cwd {
        command.current_dir(cwd);
    }
    Ok(command)
}

pub trait Processes {
    fn start(&mut self, app: &App) -> Result<(), String>;
    fn poll(&mut self) -> Result<Option<ExitStatus>, String>;
}

#[derive(Debug, Default)]
pub struct NativeProcess {
    child: Option<Child>,
}
impl Processes for NativeProcess {
    fn start(&mut self, app: &App) -> Result<(), String> {
        if self.child.is_some() {
            return Err("a child is already running".into());
        }
        let child = command(app)?
            .spawn()
            .map_err(|e| format!("{}: {e}", app.id))?;
        eprintln!(
            "level=info event=app_started app={} pid={}",
            app.id,
            child.id()
        );
        self.child = Some(child);
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        let Some(child) = &mut self.child else {
            return Ok(None);
        };
        let status = child
            .try_wait()
            .map_err(|e| format!("child wait failed: {e}"))?;
        if status.is_some() {
            self.child = None;
        }
        Ok(status)
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // On shell shutdown own only this direct child, never unrelated processes.
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {}
                Err(error) => eprintln!("level=error event=child_wait message={error:?}"),
            }
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
        state.finished("LAUNCH FAILED: invalid app index".into());
        return;
    };
    match processes.start(app) {
        Ok(()) => state.started(),
        Err(error) => {
            eprintln!("level=error event=launch_failed message={error:?}");
            state.finished(format!("LAUNCH FAILED: {error}"));
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
    fn app() -> App {
        App {
            id: "test".into(),
            name: "Test".into(),
            icon: None,
            executable: PathBuf::from("/bin/sh"),
            args: vec![],
            cwd: Some(std::env::temp_dir()),
        }
    }
    #[test]
    fn command_preserves_arguments_and_working_directory() -> Result<(), String> {
        let mut app = app();
        app.args = vec!["literal ; $(echo unsafe)".into(), "two words".into()];
        let cmd = command(&app)?;
        assert_eq!(cmd.get_program(), "/bin/sh");
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), app.args);
        assert_eq!(cmd.get_current_dir(), app.cwd.as_deref());
        Ok(())
    }
    #[test]
    fn real_child_is_reaped_and_launcher_recovers() -> Result<(), String> {
        let mut app = app();
        // Fixed test-only script; no external input is interpolated.
        app.args = vec!["-c".into(), "exit 7".into()];
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
        app.executable = "/vitrallis-missing-test/application".into();
        let mut state = crate::launcher::Launcher::new(vec![app], 1, 1)?;
        let mut process = NativeProcess::default();
        state.input(crate::input::Action::Activate);
        activate(&mut state, &mut process, 0);
        assert_eq!(state.phase, crate::launcher::Phase::Ready);
        assert!(state.status.contains("LAUNCH FAILED"));
        assert!(process.child.is_none());
        assert_eq!(state.input(crate::input::Action::Activate), Some(0));
        Ok(())
    }
}
