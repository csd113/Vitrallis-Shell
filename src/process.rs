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
    if m.tor == crate::tor::Requirement::Required && app.source == crate::app::AppSource::AppCenter
    {
        // The exported launcher owns its namespace, but missing prerequisites
        // must be diagnosed before spawning it rather than only on stderr.
        crate::tor::required_command()?;
    }
    let program = m.runtime.as_ref().unwrap_or(&m.entry);
    let mut command = if m.tor == crate::tor::Requirement::Required
        && app.source != crate::app::AppSource::AppCenter
    {
        let mut command = crate::tor::required_command()?;
        command.arg(program);
        command
    } else {
        Command::new(program)
    };
    if m.runtime.is_some() {
        command.arg(&m.entry);
    }
    command.envs(&m.env);
    command.env("VITRALLIS_APP_ID", &app.id);
    command.env(
        "VITRALLIS_DOCUMENTS_DIR",
        vitrallis_native::paths::documents(&vitrallis_native::home(), &app.id)
            .map_err(|e| e.to_string())?,
    );
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
    fn cancel_focus(&mut self) {}
    fn discover_window(&mut self) -> Result<bool, String> {
        Ok(false)
    }
    fn close_if_safe(&mut self) -> Result<(), String> {
        Ok(())
    }
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
    /// Completes a start requested by [`Processes::start`]. `None` means the
    /// request is still in progress; the caller keeps rendering and polling.
    fn poll_launch(&mut self) -> Option<Result<String, String>> {
        None
    }
    /// Whether a start request is currently in flight.
    fn launching(&self) -> Option<String> {
        None
    }
    /// Authoritative lifecycle state, used for the Shell's running indicators.
    fn state(&self, _id: &str) -> AppState {
        AppState::Stopped
    }
    /// Stops one owned application. Returns whether it was owned.
    fn terminate(&mut self, _id: &str) -> bool {
        false
    }
}

/// Explicit application lifecycle. The Shell derives every running indicator
/// from this state instead of from loosely coupled booleans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// No process is owned by the Shell.
    Stopped,
    /// A start request is in flight; no process is owned yet.
    Launching,
    /// The application is the currently focused task.
    RunningForeground,
    /// The application is alive while the Shell owns the foreground.
    RunningBackground,
    /// The last start attempt failed and no process is owned.
    Failed,
}

impl AppState {
    #[must_use]
    pub const fn is_running(self) -> bool {
        matches!(self, Self::RunningForeground | Self::RunningBackground)
    }
}

/// Each application keeps its own existing process owner. Window focus never
/// transfers process ownership, and dropping the set cleans every owned child.
#[derive(Debug, Default)]
pub struct ProcessSet<P = NativeProcess> {
    members: Vec<(String, P)>,
    waiting_windows: std::collections::BTreeSet<String>,
    background_since: std::collections::BTreeMap<String, (Instant, bool)>,
    discovery: std::collections::BTreeMap<String, DiscoveryBudget>,
    pub tor: crate::tor::Service,
    tor_apps: std::collections::BTreeSet<String>,
    tor_pending: Option<(AppEntry, Instant)>,
    active: Option<String>,
    pub exited_active: bool,
    resume: Option<Resume>,
    pending: Option<PendingStart<P>>,
    /// Outcome of a start that completed without a worker (resume/instant start).
    completed: Option<Result<String, String>>,
    failures: std::collections::BTreeMap<String, String>,
}
/// A start request being prepared on its own worker thread.
#[derive(Debug)]
struct PendingStart<P> {
    app: AppEntry,
    receiver: mpsc::Receiver<Result<P, String>>,
    deadline: Instant,
}
/// Advisory auto-close is given this long to let the app save and exit before
/// the Shell stops it. Default policy disables both steps entirely.
const BACKGROUND_GRACE: Duration = Duration::from_secs(30);
/// A start request that has not produced a child in this long is abandoned.
const LAUNCH_DEADLINE: Duration = Duration::from_secs(30);
/// Window discovery is a bounded best-effort probe. A member whose window
/// never appears must not keep spawning the window-manager query tool for the
/// life of the process, so probes slow down and then stop.
const DISCOVERY_BACKOFF: [Duration; 4] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(1),
];
/// How long after launch focus retries may run.
const DISCOVERY_WINDOW: Duration = Duration::from_secs(30);
/// Bounded tracking after the focus window ends, so a window that appears late
/// is noticed once without leaving a dead gap or probing forever.
const DISCOVERY_GRACE: Duration = Duration::from_secs(5);
/// Delay before the next retry: 250 ms, 500 ms, then 1 s for the rest.
fn retry_interval(attempts: u32) -> Duration {
    let index = usize::try_from(attempts).unwrap_or(usize::MAX);
    DISCOVERY_BACKOFF[index.min(DISCOVERY_BACKOFF.len() - 1)]
}
#[derive(Debug)]
struct Resume {
    app: AppEntry,
    deadline: Instant,
    next_attempt: Instant,
    /// Completed focus attempts; selects the escalating retry interval.
    attempts: u32,
    relaunch_on_exit: bool,
    last_error: Option<String>,
}
/// Per-member window-discovery budget; dropped with the member or when the
/// user leaves the launch view.
#[derive(Debug)]
struct DiscoveryBudget {
    /// Earliest time the next probe may run.
    next: Instant,
    /// Probes stop entirely after this instant.
    deadline: Instant,
    attempts: u32,
    given_up: bool,
    last_error: Option<String>,
}
impl<P> ProcessSet<P> {
    pub const fn has_children(&self) -> bool {
        !self.members.is_empty() || self.tor_pending.is_some() || self.pending.is_some()
    }
    pub const fn waiting_for_tor(&self) -> bool {
        self.tor_pending.is_some()
    }
    #[cfg(test)]
    pub fn running_ids(&self) -> Vec<String> {
        self.members.iter().map(|(id, _)| id.clone()).collect()
    }
}
impl<P: Processes + Default + Send + 'static> ProcessSet<P> {
    /// Stops one owned application and forgets its background bookkeeping.
    /// Dropping the owner kills its process group and reaps the direct child.
    pub(crate) fn stop_owned(&mut self, id: &str) -> bool {
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
        self.background_since.remove(id);
        self.waiting_windows.remove(id);
        self.discovery.remove(id);
        self.members.remove(index);
        true
    }
    /// Leaving the launch view cancels focus retries, never process/window tracking.
    pub fn stop_focus_retry(&mut self) {
        self.resume = None;
        for (_, process) in &mut self.members {
            process.cancel_focus();
        }
    }
    pub fn returned_home(&mut self) {
        self.stop_focus_retry();
        self.active = None;
        // Leaving the launch view also ends best-effort window discovery:
        // process ownership continues, but a window that has not appeared yet
        // is no longer probed, so a missing window cannot keep spawning the
        // window-manager query tool while the user is elsewhere in the Shell.
        self.waiting_windows.clear();
        self.discovery.clear();
    }
    #[cfg(test)]
    fn background_requested(&self, id: &str) -> bool {
        self.background_since
            .get(id)
            .is_some_and(|(_, requested)| *requested)
    }
    /// Background lifetime policy. Becoming the foreground Shell again never
    /// stops an application by itself: only an opted-in timeout expires, and the
    /// app is first asked to close safely before the Shell stops it. Returns the
    /// IDs stopped by policy so the caller can refresh its indicators.
    pub fn background_policy(
        &mut self,
        policy: &crate::preferences::Policy,
        now: Instant,
    ) -> Vec<String> {
        self.background_since
            .retain(|id, _| self.members.iter().any(|(member, _)| member == id));
        self.waiting_windows
            .retain(|id| self.members.iter().any(|(member, _)| member == id));
        self.discovery
            .retain(|id, _| self.members.iter().any(|(member, _)| member == id));
        let mut expired: Vec<String> = Vec::new();
        let mut stopped: Vec<String> = Vec::new();
        for (id, process) in &mut self.members {
            if self.active.as_ref() == Some(id) {
                self.background_since.remove(id);
                continue;
            }
            let Some(timeout) = policy.timeout(id) else {
                // No configured lifetime: keep the app running indefinitely.
                self.background_since.remove(id);
                continue;
            };
            let (since, requested) = self
                .background_since
                .entry(id.clone())
                .or_insert((now, false));
            let elapsed = now.saturating_duration_since(*since);
            if !*requested && elapsed >= timeout {
                // Advisory close first; a member that ignores it is stopped
                // after the grace period, which bounds how long an unanswered
                // request can keep an opted-in background app's resources.
                if let Err(error) = process.close_if_safe() {
                    eprintln!("level=warn event=auto_close_deferred app={id:?} error={error:?}");
                }
                *requested = true;
            }
            if elapsed >= timeout.saturating_add(BACKGROUND_GRACE) {
                expired.push(id.clone());
            }
        }
        for id in expired {
            eprintln!("level=info event=background_lifetime_expired app={id:?}");
            if self.stop_owned(&id) {
                stopped.push(id);
            }
        }
        stopped
    }
    /// Authoritative lifecycle state for one application.
    pub fn state(&self, id: &str) -> AppState {
        if self.launching().as_deref() == Some(id) {
            return AppState::Launching;
        }
        if self.members.iter().any(|(member, _)| member == id) {
            return if self.active.as_deref() == Some(id) {
                AppState::RunningForeground
            } else {
                AppState::RunningBackground
            };
        }
        if self.failures.contains_key(id) {
            return AppState::Failed;
        }
        AppState::Stopped
    }
    fn record_failure(&mut self, id: &str, error: &str) {
        if self.failures.len() >= 64 && !self.failures.contains_key(id) {
            self.failures.clear();
        }
        self.failures.insert(id.into(), error.into());
    }

    fn poll_tor_launch(&mut self) -> Result<Option<FocusResult>, String> {
        use crate::tor::{Requirement, State};
        let snapshot = self.tor.snapshot();
        let Some((_, deadline)) = &self.tor_pending else {
            return Ok(None);
        };
        let ready = snapshot.state == State::Connected;
        let failed =
            matches!(snapshot.state, State::Error | State::Disabled) || Instant::now() >= *deadline;
        if !ready && !failed {
            return Ok(None);
        }
        let Some((mut app, _)) = self.tor_pending.take() else {
            return Ok(None);
        };
        if !ready && app.manifest.tor == Requirement::Required {
            self.tor_apps.remove(&app.id);
            self.tor
                .request(crate::tor::Control::Demand(self.tor_apps.len()))?;
            return Err(format!(
                "Tor required: {}. {}",
                snapshot.state.label(),
                snapshot.diagnostic
            ));
        }
        crate::tor::configure_app(&mut app, &snapshot);
        // The spawn itself still happens off the UI thread; `poll_launch`
        // reports completion and `poll_focus` tracks the new window.
        self.start_ready(&app)?;
        self.sync_tor_apps();
        Ok(None)
    }
    pub fn sync_tor_apps(&mut self) {
        let previous = self.tor_apps.len();
        self.tor_apps.retain(|id| {
            self.members.iter().any(|(member, _)| member == id)
                || self
                    .tor_pending
                    .as_ref()
                    .is_some_and(|(app, _)| &app.id == id)
        });
        if self.tor_apps.len() != previous {
            let _ = self
                .tor
                .request(crate::tor::Control::Demand(self.tor_apps.len()));
        }
    }
    /// Adopts a started process: records ownership and starts focus tracking.
    fn adopt(&mut self, app: &AppEntry, mut process: P) -> Result<(), String> {
        if process.waits_for_window() {
            let now = Instant::now();
            self.track_window(&app.id, now);
            process.focus()?;
            self.resume = Some(Resume {
                app: app.clone(),
                deadline: now + DISCOVERY_WINDOW,
                next_attempt: now + DISCOVERY_BACKOFF[0],
                attempts: 0,
                relaunch_on_exit: false,
                last_error: None,
            });
        }
        self.members.push((app.id.clone(), process));
        self.active = Some(app.id.clone());
        Ok(())
    }
    /// Records or refreshes bounded window tracking for one member.
    fn track_window(&mut self, id: &str, now: Instant) {
        self.waiting_windows.insert(id.into());
        self.discovery.insert(
            id.into(),
            DiscoveryBudget {
                next: now + DISCOVERY_BACKOFF[0],
                deadline: now + DISCOVERY_WINDOW + DISCOVERY_GRACE,
                attempts: 0,
                given_up: false,
                last_error: None,
            },
        );
    }
    /// Hands process creation to a worker thread. `command` inspects the
    /// filesystem and `spawn` copies the Shell's address space; neither may
    /// stall rendering on the `PocketCHIP`.
    fn request_start(app: &AppEntry) -> Result<PendingStart<P>, String> {
        let request = app.clone();
        let (send, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("app-launch".into())
            .spawn(move || {
                let mut process = P::default();
                let result = process.start(&request).map(|()| process);
                let _ = send.send(result);
            })
            .map_err(|error| format!("launch worker: {error}"))?;
        Ok(PendingStart {
            app: app.clone(),
            receiver,
            deadline: Instant::now() + LAUNCH_DEADLINE,
        })
    }
    fn start_ready(&mut self, app: &AppEntry) -> Result<(), String> {
        if let Some(index) = self.members.iter().position(|(id, _)| id == &app.id) {
            // A window can close before the next scheduled child poll. Reap it
            // now so activation does not try to resume an already exited app.
            if self.members[index].1.poll()?.is_none() {
                self.members[index].1.deliver(app)?;
                self.members[index].1.focus()?;
                self.active = Some(app.id.clone());
                self.background_since.remove(&app.id);
                self.resume = Some(Resume {
                    app: app.clone(),
                    deadline: Instant::now() + DISCOVERY_WINDOW,
                    next_attempt: Instant::now() + DISCOVERY_BACKOFF[0],
                    attempts: 0,
                    relaunch_on_exit: true,
                    last_error: None,
                });
                // Re-activation gives a still-missing window a fresh bounded
                // tracking window instead of an already-expired budget.
                if self.waiting_windows.contains(&app.id) {
                    self.track_window(&app.id, Instant::now());
                }
                // A resume is complete as soon as it is requested; report it
                // through the same path as a worker-completed start.
                self.completed = Some(Ok(app.id.clone()));
                return Ok(());
            }
            self.members.remove(index);
            // A stopped Tor app must re-enter the readiness gate before relaunch.
            if app.manifest.tor != crate::tor::Requirement::None {
                return self.start(app);
            }
        }
        self.resume = None;
        self.pending = Some(Self::request_start(app)?);
        Ok(())
    }
}
impl<P: Processes + Default + Send + 'static> Processes for ProcessSet<P> {
    fn start(&mut self, app: &AppEntry) -> Result<(), String> {
        // A second request for the app already starting is not an error: the
        // user asked for the same thing twice and one launch satisfies it.
        if let Some(pending) = &self.pending {
            if pending.app.id == app.id {
                return Ok(());
            }
            return Err(format!("{} is still starting", pending.app.name));
        }
        if app.manifest.tor == crate::tor::Requirement::None
            || self.members.iter().any(|(id, _)| id == &app.id)
        {
            return self.start_ready(app);
        }
        if self.tor_pending.is_some() {
            return Err("Another application is waiting for Tor".into());
        }
        app.validate()?;
        self.tor_apps.insert(app.id.clone());
        if let Err(error) = self
            .tor
            .request(crate::tor::Control::Demand(self.tor_apps.len()))
        {
            self.tor_apps.remove(&app.id);
            return Err(error);
        }
        self.tor_pending = Some((app.clone(), Instant::now() + Duration::from_secs(190)));
        Ok(())
    }
    fn poll_launch(&mut self) -> Option<Result<String, String>> {
        if let Some(outcome) = self.completed.take() {
            if let Ok(id) = &outcome {
                self.failures.remove(id);
            }
            return Some(outcome);
        }
        let outcome = {
            let pending = self.pending.as_ref()?;
            match pending.receiver.try_recv() {
                Ok(Ok(process)) => Ok(process),
                Ok(Err(error)) => Err(error),
                Err(mpsc::TryRecvError::Empty) if Instant::now() < pending.deadline => return None,
                Err(mpsc::TryRecvError::Empty) => {
                    Err(format!("{} did not start in time", pending.app.name))
                }
                Err(mpsc::TryRecvError::Disconnected) => Err("launch worker stopped".into()),
            }
        };
        let pending = self.pending.take()?;
        let app = pending.app;
        match outcome {
            Ok(process) => match self.adopt(&app, process) {
                Ok(()) => {
                    self.failures.remove(&app.id);
                    eprintln!(
                        "level=info event=launch_completed app={} running={}",
                        app.id,
                        self.members.len()
                    );
                    Some(Ok(app.id))
                }
                Err(error) => {
                    self.record_failure(&app.id, &error);
                    Some(Err(error))
                }
            },
            Err(error) => {
                let error = format!("{}: {error}", app.name);
                eprintln!("level=error event=launch_failed message={error:?}");
                self.record_failure(&app.id, &error);
                Some(Err(error))
            }
        }
    }
    fn launching(&self) -> Option<String> {
        self.pending.as_ref().map(|pending| pending.app.id.clone())
    }
    fn state(&self, id: &str) -> AppState {
        Self::state(self, id)
    }
    fn terminate(&mut self, id: &str) -> bool {
        self.stop_owned(id)
    }
    fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        let mut failure = None;
        let now = Instant::now();
        for (id, process) in &mut self.members {
            if !self.waiting_windows.contains(id)
                || self.resume.as_ref().is_some_and(|r| &r.app.id == id)
            {
                continue;
            }
            let Some(budget) = self.discovery.get_mut(id) else {
                continue;
            };
            if now >= budget.deadline {
                if !budget.given_up {
                    budget.given_up = true;
                    eprintln!(
                        "level=warn event=window_discovery_given_up app={id:?} attempts={} window_secs={} last_error={:?}",
                        budget.attempts,
                        (DISCOVERY_WINDOW + DISCOVERY_GRACE).as_secs(),
                        budget.last_error
                    );
                }
                continue;
            }
            if now < budget.next {
                continue;
            }
            let interval = retry_interval(budget.attempts);
            budget.attempts = budget.attempts.saturating_add(1);
            budget.next = now + interval;
            match process.discover_window() {
                Ok(true) => {
                    self.waiting_windows.remove(id);
                    self.discovery.remove(id);
                }
                Ok(false) => {
                    if let Some(budget) = self.discovery.get_mut(id) {
                        budget.last_error = None;
                    }
                }
                Err(error) => {
                    if let Some(budget) = self.discovery.get_mut(id) {
                        budget.last_error = Some(error);
                    }
                }
            }
        }
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
        if self.tor_pending.is_some() {
            return self.poll_tor_launch();
        }
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
                self.waiting_windows.remove(&self.members[index].0);
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
                        // Escalating retries keep a slow window from spawning a
                        // focus worker every 250 ms until the deadline.
                        resume.attempts = resume.attempts.saturating_add(1);
                        resume.next_attempt = Instant::now() + retry_interval(resume.attempts);
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
    discovery_result: Option<mpsc::Receiver<Result<FocusResult, String>>>,
    focus_cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    native_socket: Option<std::path::PathBuf>,
}
impl Processes for NativeProcess {
    fn cancel_focus(&mut self) {
        self.focus_cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        self.focus_result = None;
    }
    fn close_if_safe(&mut self) -> Result<(), String> {
        if let Some(path) = &self.native_socket {
            vitrallis_native::ipc::close_if_safe(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn discover_window(&mut self) -> Result<bool, String> {
        if let Some(path) = &self.native_socket {
            return Ok(path.exists());
        }
        if let Some(result) = &self.discovery_result {
            match result.try_recv() {
                Ok(result) => {
                    self.discovery_result = None;
                    return result.map(|r| r == FocusResult::Focused);
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(false),
                Err(mpsc::TryRecvError::Disconnected) => self.discovery_result = None,
            }
        }
        if let Some(child) = &self.child {
            let pid = child.id();
            self.discovery_result = Some(request_focus(move || {
                crate::platform::application_window(pid, false)
            })?);
        }
        Ok(false)
    }

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
        self.focus_cancelled = std::sync::Arc::default();
        let cancelled = std::sync::Arc::clone(&self.focus_cancelled);
        self.focus_result = Some(request_focus(move || {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Ok(FocusResult::Missing);
            }
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
/// The start itself is asynchronous; the caller keeps the Shell responsive until
/// `Processes::poll_launch` reports the outcome.
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
    let name = app.name.clone();
    match processes.start(app) {
        Ok(()) => state.launching(&name),
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
    /// Starts an app and waits for its worker-owned child, so tests exercise the
    /// same ownership path as the Shell without racing the launch thread.
    pub(super) fn start_blocking<P: Processes + Default + Send + 'static>(
        processes: &mut ProcessSet<P>,
        app: &AppEntry,
    ) -> Result<(), String> {
        // Discard outcomes from earlier requests so this call observes its own.
        while processes.poll_launch().is_some() {}
        processes.start(app)?;
        settle(processes)
    }
    /// Waits for the in-flight start request, if there is one.
    pub(super) fn settle<P: Processes + Default + Send + 'static>(
        processes: &mut ProcessSet<P>,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if processes.launching().is_none() && processes.completed.is_none() {
                return Ok(());
            }
            match processes.poll_launch() {
                Some(Ok(_)) => return Ok(()),
                Some(Err(error)) => return Err(error),
                None if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                None => return Err("launch timed out".into()),
            }
        }
    }
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
        start_blocking(&mut processes, &first)?;
        start_blocking(&mut processes, &second)?;
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
            attempts: 0,
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
    fn cancelled_focus_cannot_satisfy_a_later_activation() -> Result<(), String> {
        let (send, receive) = mpsc::channel();
        send.send(Ok(FocusResult::Focused))
            .map_err(|e| e.to_string())?;
        let mut process = NativeProcess {
            focus_result: Some(receive),
            child: None,
            window_hint: None,
            native_socket: None,
            discovery_result: None,
            focus_cancelled: std::sync::Arc::default(),
        };
        process.cancel_focus();
        assert_eq!(process.poll_focus()?, None);
        assert!(
            process
                .focus_cancelled
                .load(std::sync::atomic::Ordering::Acquire)
        );
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
            discovery_result: None,
            focus_cancelled: std::sync::Arc::default(),
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
        settle(&mut processes)?;
        state.returned_home();
        state.input(crate::input::Action::SelectAndActivate(1));
        activate(&mut state, &mut processes, 1);
        settle(&mut processes)?;
        assert_eq!(processes.running_ids(), ["test", "second"]);
        start_blocking(&mut processes, &state.apps[0])?;
        assert_eq!(processes.members[0].1.starts, 1);
        assert_eq!(processes.members[0].1.focuses, 1);
        processes.poll_focus()?;
        processes.members[0].1.exited = true;
        // Activation arriving before the scheduled exit poll starts a fresh
        // process rather than dispatching focus to a dead window.
        start_blocking(&mut processes, &state.apps[0])?;
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
        close_requests: usize,
        focus_error: Option<String>,
        exited: bool,
        window_ready: bool,
        starts: usize,
        focuses: usize,
        discoveries: usize,
    }
    impl Processes for ClosingWindow {
        fn close_if_safe(&mut self) -> Result<(), String> {
            self.close_requests += 1;
            Ok(())
        }
        fn discover_window(&mut self) -> Result<bool, String> {
            self.discoveries += 1;
            Ok(self.window_ready)
        }
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
    fn delayed_window_is_discovered_after_leaving_without_relaunch_or_focus() -> Result<(), String>
    {
        #[derive(Default)]
        struct Delayed(ClosingWindow);
        impl Processes for Delayed {
            fn start(&mut self, app: &AppEntry) -> Result<(), String> {
                self.0.start(app)
            }
            fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
                self.0.poll()
            }
            fn focus(&mut self) -> Result<(), String> {
                self.0.focus()
            }
            fn poll_focus(&mut self) -> Result<Option<FocusResult>, String> {
                self.0.poll_focus()
            }
            fn waits_for_window(&self) -> bool {
                true
            }
            fn discover_window(&mut self) -> Result<bool, String> {
                self.0.discover_window()
            }
        }
        for navigated_away in [false, true] {
            let mut processes = ProcessSet::<Delayed>::default();
            start_blocking(&mut processes, &app())?;
            if navigated_away {
                processes.returned_home();
            } else {
                processes.resume.as_mut().ok_or("missing launch")?.deadline = Instant::now();
                assert_eq!(processes.poll_focus()?, Some(FocusResult::Missing));
            }
            processes.members[0].1.0.window_ready = true;
            if navigated_away {
                // Leaving the launch view ends discovery: ownership continues,
                // but a window that appears later is no longer probed for, so
                // no window-manager query can be spawned while the user is
                // elsewhere in the Shell.
                assert!(processes.discovery.is_empty());
                assert!(processes.waiting_windows.is_empty());
                assert_eq!(processes.poll()?, None);
                assert_eq!(processes.members[0].1.0.discoveries, 0);
            } else {
                // The expired resume no longer owns discovery; the scheduled
                // probe still notices the window without relaunching or
                // focusing it.
                let budget = processes
                    .discovery
                    .get_mut("test")
                    .ok_or("missing discovery")?;
                budget.next = Instant::now();
                assert_eq!(processes.poll()?, None);
                assert!(processes.waiting_windows.is_empty());
                assert!(!processes.discovery.contains_key("test"));
                assert_eq!(processes.members[0].1.0.discoveries, 1);
            }
            assert_eq!(processes.members[0].1.0.starts, 1);
            assert_eq!(processes.members[0].1.0.focuses, 1); // Only the original launch request.
            assert_eq!(processes.poll_focus()?, None);
        }
        Ok(())
    }
    #[test]
    fn window_discovery_backs_off_gives_up_and_stops_after_returning_home() -> Result<(), String> {
        #[derive(Default)]
        struct Counting {
            discoveries: usize,
        }
        impl Processes for Counting {
            fn start(&mut self, _: &AppEntry) -> Result<(), String> {
                Ok(())
            }
            fn focus(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
                Ok(None)
            }
            fn waits_for_window(&self) -> bool {
                true
            }
            fn discover_window(&mut self) -> Result<bool, String> {
                self.discoveries += 1;
                Ok(false)
            }
        }
        let mut processes = ProcessSet::<Counting>::default();
        start_blocking(&mut processes, &app())?;
        // Simulate the focus deadline expiring; the member stays owned.
        processes.resume = None;
        // The first probe is scheduled after the launch delay, not immediately.
        assert_eq!(processes.poll()?, None);
        assert_eq!(processes.members[0].1.discoveries, 0);
        let budget = processes
            .discovery
            .get_mut("test")
            .ok_or("missing discovery")?;
        assert!(budget.next > Instant::now());
        budget.next = Instant::now();
        assert_eq!(processes.poll()?, None);
        assert_eq!(processes.members[0].1.discoveries, 1);
        let budget = processes.discovery.get("test").ok_or("missing discovery")?;
        assert_eq!(budget.attempts, 1);
        assert!(
            budget.next > Instant::now(),
            "the next probe must be delayed"
        );
        // The bounded window expiring stops probing for good.
        processes
            .discovery
            .get_mut("test")
            .ok_or("missing discovery")?
            .deadline = Instant::now();
        assert_eq!(processes.poll()?, None);
        assert_eq!(processes.members[0].1.discoveries, 1);
        let budget = processes.discovery.get("test").ok_or("missing discovery")?;
        assert!(budget.given_up);
        assert_eq!(budget.attempts, 1);
        // Leaving the launch view drops the budget and the pending window mark.
        processes.returned_home();
        assert!(processes.discovery.is_empty());
        assert!(processes.waiting_windows.is_empty());
        assert_eq!(processes.poll()?, None);
        assert_eq!(processes.members[0].1.discoveries, 1);
        Ok(())
    }
    #[test]
    fn re_activation_refreshes_the_window_tracking_budget() -> Result<(), String> {
        #[derive(Default)]
        struct Quiet;
        impl Processes for Quiet {
            fn start(&mut self, _: &AppEntry) -> Result<(), String> {
                Ok(())
            }
            fn focus(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
                Ok(None)
            }
            fn waits_for_window(&self) -> bool {
                true
            }
        }
        let mut processes = ProcessSet::<Quiet>::default();
        start_blocking(&mut processes, &app())?;
        // Simulate a tracking budget that already gave up.
        let budget = processes
            .discovery
            .get_mut("test")
            .ok_or("missing discovery")?;
        budget.deadline = Instant::now();
        budget.given_up = true;
        assert!(processes.waiting_windows.contains("test"));
        // Re-activating a still-waiting member starts a fresh bounded window.
        processes.start_ready(&app())?;
        let budget = processes.discovery.get("test").ok_or("missing discovery")?;
        assert!(!budget.given_up);
        assert_eq!(budget.attempts, 0);
        assert!(budget.deadline > Instant::now());
        assert!(budget.next > Instant::now());
        Ok(())
    }
    #[test]
    fn background_lifetime_is_advisory_then_bounded_and_essential_apps_are_exempt()
    -> Result<(), String> {
        let mut processes = ProcessSet::<ClosingWindow>::default();
        start_blocking(&mut processes, &app())?;
        processes.returned_home();
        let now = Instant::now();
        let mut policy = crate::preferences::Policy {
            background_seconds: 60,
            ..Default::default()
        };
        // Becoming the foreground Shell again never terminates anything.
        assert!(processes.background_policy(&policy, now).is_empty());
        assert_eq!(processes.members[0].1.close_requests, 0);
        processes.background_policy(&policy, now + Duration::from_secs(59));
        assert!(!processes.background_requested("test"));
        assert_eq!(processes.members[0].1.close_requests, 0);
        // The configured lifetime expires: ask the app to close safely first.
        processes.background_policy(&policy, now + Duration::from_secs(60));
        assert!(processes.background_requested("test"));
        assert_eq!(processes.members[0].1.close_requests, 1);
        assert_eq!(processes.members.len(), 1);
        // An app that ignores the request is stopped only after the grace period,
        // so a save-to-disk has time to finish.
        let stopped = processes.background_policy(&policy, now + Duration::from_secs(60 + 29));
        assert!(stopped.is_empty());
        assert_eq!(processes.members.len(), 1);
        let stopped =
            processes.background_policy(&policy, now + Duration::from_secs(60) + BACKGROUND_GRACE);
        assert_eq!(stopped, ["test"]);
        assert!(!processes.has_children());
        // An essential app keeps its background bookkeeping cleared.
        start_blocking(&mut processes, &app())?;
        processes.returned_home();
        policy.essential.insert("test".into());
        processes.background_policy(&policy, now + Duration::from_secs(120));
        assert!(processes.background_since.is_empty());
        assert_eq!(processes.members[0].1.close_requests, 0);
        assert_eq!(processes.members.len(), 1);
        Ok(())
    }
    #[test]
    fn disabled_lifetime_and_foreground_returns_never_stop_an_app() -> Result<(), String> {
        let mut processes = ProcessSet::<ClosingWindow>::default();
        start_blocking(&mut processes, &app())?;
        processes.returned_home();
        let now = Instant::now();
        let policy = crate::preferences::Policy::default();
        for seconds in [0_u64, 600, 86_400] {
            assert!(
                processes
                    .background_policy(&policy, now + Duration::from_secs(seconds))
                    .is_empty()
            );
        }
        assert_eq!(processes.members[0].1.close_requests, 0);
        assert_eq!(processes.members.len(), 1);
        // Explicit termination still works promptly.
        assert!(processes.terminate("test"));
        assert!(!processes.has_children());
        Ok(())
    }
    #[test]
    fn reopen_waits_for_closing_process_then_spawns_exactly_once() -> Result<(), String> {
        let mut processes = ProcessSet::<ClosingWindow>::default();
        start_blocking(&mut processes, &app())?;
        start_blocking(&mut processes, &app())?;
        assert_eq!(processes.poll_focus()?, None); // Window gone; process still exits asynchronously.
        assert_eq!(processes.members.len(), 1);
        assert_eq!(processes.members[0].1.starts, 1);
        processes.members[0].1.exited = true;
        assert_eq!(processes.poll()?, None); // Preserve the pending activation.
        assert_eq!(processes.poll_focus()?, Some(FocusResult::Focused));
        settle(&mut processes)?; // The replacement child is created by the worker.
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
        start_blocking(&mut processes, &app())?;
        start_blocking(&mut processes, &app())?;
        assert_eq!(processes.poll_focus()?, None);
        processes.members[0].1.window_ready = true;
        assert_eq!(processes.poll_focus()?, Some(FocusResult::Focused));
        assert_eq!(processes.members[0].1.starts, 1);
        processes.members[0].1.window_ready = false;
        start_blocking(&mut processes, &app())?;
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
        start_blocking(&mut processes, &app())?;
        start_blocking(&mut processes, &app())?;
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
        start_blocking(&mut processes, &app())?;
        start_blocking(&mut processes, &app())?;
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
        assert_eq!(state.phase, crate::launcher::Phase::Launching);
        assert!(process.start(&state.apps[0]).is_err());
        state.launched("Test");
        assert_eq!(state.phase, crate::launcher::Phase::Running);
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
        let mut process = ProcessSet::<NativeProcess>::default();
        state.input(crate::input::Action::Activate);
        activate(&mut state, &mut process, 0);
        // The spawn failure arrives from the launch worker, never inline.
        assert_eq!(state.phase, crate::launcher::Phase::Launching);
        let deadline = Instant::now() + Duration::from_secs(5);
        let error = loop {
            match process.poll_launch() {
                Some(Ok(_)) => return Err("missing entry must not start".into()),
                Some(Err(error)) => break error,
                None if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                None => return Err("launch failure timed out".into()),
            }
        };
        state.failed(error);
        assert_eq!(state.phase, crate::launcher::Phase::Ready);
        assert!(state.status.contains("LAUNCH FAILED"));
        assert!(!process.has_children());
        assert_eq!(process.state("test"), AppState::Failed);
        assert_eq!(state.input(crate::input::Action::Activate), None); // Dismiss the error.
        assert_eq!(state.input(crate::input::Action::Activate), Some(0));
        assert_eq!(state.phase, crate::launcher::Phase::Launching);
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
            ..ProcessSet::default()
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
    #[test]
    fn forty_launch_exit_cycles_reap_and_recover_without_accumulating_children()
    -> Result<(), String> {
        let mut state = Launcher::new(vec![fixture()], 1, 1)?;
        let mut processes = ProcessSet::<NativeProcess>::default();
        for _ in 0..40 {
            assert_eq!(state.input(Action::Activate), Some(0));
            activate(&mut state, &mut processes, 0);
            assert_eq!(state.phase, Phase::Launching);
            // A repeated activation while starting never spawns a second child.
            assert!(state.input(Action::Activate).is_none());
            assert!(processes.start(&state.apps[0]).is_ok());
            super::tests::start_blocking(&mut processes, &state.apps[0])?;
            assert_eq!(processes.state("cycles"), AppState::RunningForeground);
            assert_eq!(processes.running_ids(), ["cycles"]);
            state.launched("Cycles");
            assert_eq!(state.phase, Phase::Running);
            let deadline = Instant::now() + Duration::from_secs(5);
            let status = loop {
                if let Some(status) = processes.poll()? {
                    break status;
                }
                if Instant::now() >= deadline {
                    return Err("child did not exit".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            assert!(status.success());
            assert!(processes.poll()?.is_none());
            assert_eq!(processes.state("cycles"), AppState::Stopped);
            assert!(!processes.has_children());
            state.finished("closed".into());
            assert_eq!(state.phase, Phase::Ready);
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
        assert_eq!(state.phase, Phase::Launching);
        assert!(process.poll().is_err());
        // The single in-flight start is never duplicated while it is pending.
        assert!(state.input(Action::Activate).is_none());
        assert_eq!(state.input(Action::SelectAndActivate(0)), None);
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
