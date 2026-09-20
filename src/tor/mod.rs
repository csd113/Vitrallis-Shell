//! Shared Tor policy and asynchronous supervision. Arti is never linked into Shell.
#[cfg(test)]
mod tests;
mod worker;
use crate::app_center::{metadata, storage};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 9150;
pub const HELPER: &str = include_str!("supervisor.py");
pub const SANDBOX: &str = include_str!("sandbox.py");

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    #[default]
    None,
    Preferred,
    Required,
}
impl Requirement {
    pub fn parse(manifest: &serde_json::Value) -> Result<Self, String> {
        let Some(network) = manifest.get("network") else {
            return Ok(Self::None);
        };
        metadata::fields(network, "tor")?;
        match network["tor"].as_str() {
            Some("none") => Ok(Self::None),
            Some("preferred") => Ok(Self::Preferred),
            Some("required") => Ok(Self::Required),
            _ => Err("network.tor must be required, preferred or none".into()),
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    #[default]
    OnDemand,
    AlwaysOn,
    Disabled,
}
impl Mode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OnDemand => "On demand",
            Self::AlwaysOn => "Always on",
            Self::Disabled => "Disabled",
        }
    }
    pub const fn key(self) -> &'static str {
        match self {
            Self::OnDemand => "on-demand",
            Self::AlwaysOn => "always-on",
            Self::Disabled => "disabled",
        }
    }
    pub const fn next(self) -> Self {
        match self {
            Self::OnDemand => Self::AlwaysOn,
            Self::AlwaysOn => Self::Disabled,
            Self::Disabled => Self::OnDemand,
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Disabled,
    #[default]
    Stopped,
    Starting,
    Bootstrapping,
    Connected,
    Stopping,
    Error,
}
impl State {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Stopped => "Stopped",
            Self::Starting => "Starting",
            Self::Bootstrapping => "Bootstrapping",
            Self::Connected => "Connected",
            Self::Stopping => "Stopping",
            Self::Error => "Error",
        }
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub mode: Mode,
    pub state: State,
    pub progress: Option<u8>,
    pub apps: usize,
    pub diagnostic: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Start,
    Stop,
    Restart,
    Mode(Mode),
    Demand(usize),
    Shutdown,
}
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub config: PathBuf,
}
impl Paths {
    pub fn current() -> Result<Self, String> {
        let loc = storage::Locations::current()?;
        Ok(Self {
            root: loc.data.join("vitrallis/tor"),
            config: loc.sources.with_file_name("tor.json"),
        })
    }
    pub fn load(&self) -> Result<Mode, String> {
        let Some(file) = storage::read(&self.config, 4096)? else {
            return Ok(Mode::OnDemand);
        };
        let value = metadata::json(&file.bytes)?;
        metadata::fields(&value, "startup")?;
        match value["startup"].as_str() {
            Some("on-demand") => Ok(Mode::OnDemand),
            Some("always-on") => Ok(Mode::AlwaysOn),
            Some("disabled") => Ok(Mode::Disabled),
            _ => Err("Invalid Tor startup configuration".into()),
        }
    }
    pub fn save(&self, mode: Mode) -> Result<(), String> {
        storage::atomic(
            &self.config,
            &storage::FileData {
                bytes: format!("{{\"startup\":\"{}\"}}\n", mode.key()).into_bytes(),
                mode: 0o600,
            },
        )
    }
}
#[derive(Debug, Default)]
pub struct Service {
    tx: Option<mpsc::SyncSender<worker::Event>>,
    snapshot: Arc<Mutex<Snapshot>>,
    handle: Option<std::thread::JoinHandle<()>>,
}
impl Service {
    pub fn initialize(&mut self) -> Result<(), String> {
        if self.tx.is_some() {
            return Ok(());
        }
        let paths = Paths::current()?;
        let (tx, rx) = mpsc::sync_channel(32);
        let output = Arc::clone(&self.snapshot);
        let events = tx.clone();
        self.handle = Some(
            std::thread::Builder::new()
                .name("tor-service".into())
                .spawn(move || {
                    worker::run(paths, &rx, &events, &output);
                })
                .map_err(|e| e.to_string())?,
        );
        self.tx = Some(tx);
        Ok(())
    }
    pub fn request(&mut self, control: Control) -> Result<(), String> {
        self.initialize()?;
        self.tx
            .as_ref()
            .ok_or("Tor worker unavailable")?
            .try_send(worker::Event::Control(control))
            .map_err(|_| "Tor service is busy; retry".into())
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().map_or_else(
            |_| Snapshot {
                state: State::Error,
                diagnostic: "Tor worker unavailable".into(),
                ..Snapshot::default()
            },
            |s| s.clone(),
        )
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(worker::Event::Control(Control::Shutdown));
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Fixed helper invocation: no shell expansion, PATH lookup or app-supplied proxy.
pub fn required_command() -> Result<std::process::Command, String> {
    if !cfg!(target_os = "linux") || !std::path::Path::new("/usr/bin/bwrap").is_file() {
        return Err(
            "Tor required: network isolation unavailable (install bubblewrap on Linux)".into(),
        );
    }
    let paths = Paths::current()?;
    let mut cmd = std::process::Command::new("/usr/bin/bwrap");
    cmd.args(sandbox_arguments(&paths.root));
    Ok(cmd)
}

/// Per-launch discovery contract; availability is advisory and never a bypass.
pub fn configure_app(app: &mut crate::app::AppEntry, snapshot: &Snapshot) {
    let env = &mut app.manifest.env;
    for (key, value) in [
        ("VITRALLIS_TOR_API", "1".to_owned()),
        (
            "VITRALLIS_TOR_AVAILABLE",
            if snapshot.state == State::Connected {
                "1"
            } else {
                "0"
            }
            .into(),
        ),
        ("VITRALLIS_TOR_SOCKS_HOST", HOST.into()),
        ("VITRALLIS_TOR_SOCKS_PORT", PORT.to_string()),
        (
            "VITRALLIS_TOR_STATE",
            snapshot.state.label().to_ascii_lowercase(),
        ),
    ] {
        env.insert(key.into(), value.into());
    }
    if let Ok(paths) = Paths::current() {
        env.insert(
            "VITRALLIS_TOR_STATUS_SOCKET".into(),
            paths.root.join("status.sock").into_os_string(),
        );
    }
    if snapshot.state == State::Connected {
        for key in [
            "ALL_PROXY",
            "all_proxy",
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
        ] {
            env.insert(key.into(), format!("socks5h://{HOST}:{PORT}").into());
        }
        for key in ["NO_PROXY", "no_proxy"] {
            env.insert(key.into(), "".into());
        }
    }
}

fn sandbox_arguments(root: &std::path::Path) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = [
        "--die-with-parent",
        "--unshare-user",
        "--unshare-pid",
        "--unshare-net",
        "--cap-drop",
        "ALL",
        "--bind",
        "/",
        "/",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--",
        "/usr/bin/python3",
        "-I",
        "-c",
        SANDBOX,
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push(root.join("proxy.sock").into_os_string());
    args.push(HELPER.into());
    args
}

/// Exported launchers enforce isolation too, including launches from desktop aliases.
pub fn wrap_launcher(
    bytes: Vec<u8>,
    tor: Requirement,
    root: &std::path::Path,
) -> Result<Vec<u8>, String> {
    if tor != Requirement::Required {
        return Ok(bytes);
    }
    let script = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut args = sandbox_arguments(root);
    args.extend(["/bin/sh".into(), "-c".into(), script.into()]);
    let mut script = "#!/bin/sh\nexec /usr/bin/bwrap".to_owned();
    for arg in args {
        let value = arg.to_str().ok_or("Tor launcher paths require UTF-8")?;
        script.push_str(" '");
        script.push_str(&value.replace('\'', "'\\''"));
        script.push('\'');
    }
    script.push('\n');
    Ok(script.into_bytes())
}
