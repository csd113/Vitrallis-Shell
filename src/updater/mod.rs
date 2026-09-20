//! Shell-only self-update orchestration. No app discovery, configuration or manifests.
pub mod bundle;
mod release;
mod transport;

use crate::platform::update::{Relaunch, Target};
use release::Release;
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
};
use transport::{Curl, Transport};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Checking,
    Current,
    Available(Release),
    Downloading {
        received: u64,
        total: u64,
    },
    Installing,
    Installed {
        version: Version,
        durable: bool,
        relaunch: Relaunch,
    },
    Failed(String),
}
impl State {
    pub const fn busy(&self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Downloading { .. } | Self::Installing
        )
    }
    pub fn detail(&self) -> String {
        match self {
            Self::Idle => "Check the official shell releases".into(),
            Self::Checking => "Checking GitHub for shell updates...".into(),
            Self::Current => "Vitrallis is up to date.".into(),
            Self::Available(release) if release.version.to_string() == VERSION => format!(
                "Complete this release: {}\nDownload size: {} MB",
                release.version,
                megabytes(release.binary.size)
            ),
            Self::Available(release) => format!(
                "New version available: {}\nDownload size: {} MB",
                release.version,
                megabytes(release.binary.size)
            ),
            Self::Downloading { received, total } => format!(
                "Downloading shell update: {}%\n{} / {} MB",
                received
                    .saturating_mul(100)
                    .checked_div(*total)
                    .unwrap_or(0)
                    .min(100),
                megabytes(*received),
                megabytes(*total)
            ),
            Self::Installing => "Verifying and installing shell update...".into(),
            Self::Installed {
                version,
                durable: true,
                ..
            } => format!("Shell updated to {version}. Relaunch required."),
            Self::Installed { durable: false, .. } => {
                "Shell replaced; disk sync failed. Backup retained.".into()
            }
            Self::Failed(error) => error.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Updater {
    pub state: State,
    result: Option<Receiver<State>>,
    relaunch_requested: bool,
    relaunch_error: Option<String>,
    progress: Arc<Mutex<Option<State>>>,
}
impl Updater {
    pub fn detail(&self) -> String {
        let mut detail = self
            .relaunch_error
            .clone()
            .unwrap_or_else(|| self.state.detail());
        if let Some(notice) = crate::platform::linux_handheld::gpu_setup_notice() {
            detail.push('\n');
            detail.push_str(notice);
        }
        detail
    }
    pub const fn request_relaunch(&mut self) {
        if matches!(self.state, State::Installed { .. }) {
            self.relaunch_requested = true;
        }
    }
    pub fn relaunch_if_requested(&mut self, blocked: bool) -> bool {
        self.relaunch_with(blocked, Relaunch::execute)
    }
    fn relaunch_with(
        &mut self,
        blocked: bool,
        execute: impl FnOnce(&Relaunch) -> Result<(), String>,
    ) -> bool {
        if !std::mem::take(&mut self.relaunch_requested) {
            return false;
        }
        let State::Installed { relaunch, .. } = &self.state else {
            return false;
        };
        let result = if blocked {
            Err("Close running apps and wait for operations to finish, then select Relaunch Shell again".into())
        } else {
            execute(relaunch)
        };
        self.relaunch_error = result.err();
        true
    }
    pub fn check(&mut self) {
        if self.state.busy() || matches!(self.state, State::Installed { .. }) {
            return;
        }
        self.start(State::Checking, |_| {
            #[cfg(unix)]
            let incomplete = crate::platform::update::Installation::current()
                .is_ok_and(|installation| installation.needs_completion());
            #[cfg(not(unix))]
            let incomplete = false;
            check_inventory(&Curl, VERSION, Target::current, incomplete)
                .unwrap_or_else(|error| State::Failed(format!("Update check failed: {error}")))
        });
    }
    pub fn install(&mut self) {
        let State::Available(release) = &self.state else {
            return;
        };
        let release = release.clone();
        self.start(
            State::Downloading {
                received: 0,
                total: release.binary.size,
            },
            move |progress| {
                install(&Curl, &release, &mut |state| {
                    // Replace the previous sample so the UI always reads the latest byte count.
                    if let Ok(mut latest) = progress.lock() {
                        *latest = Some(state);
                    }
                })
                .map_or_else(
                    |error| State::Failed(format!("Update failed: {error}")),
                    |(durable, relaunch)| State::Installed {
                        version: release.version,
                        durable,
                        relaunch,
                    },
                )
            },
        );
    }
    fn start(
        &mut self,
        state: State,
        work: impl FnOnce(&Mutex<Option<State>>) -> State + Send + 'static,
    ) {
        let (send, receive) = mpsc::sync_channel(1);
        self.progress = Arc::default();
        let progress = Arc::clone(&self.progress);
        match thread::Builder::new()
            .name("shell-update".into())
            .spawn(move || {
                let result = work(&progress);
                let _ = send.send(result);
            }) {
            Ok(_) => {
                self.state = state;
                self.result = Some(receive);
            }
            Err(error) => {
                self.state = State::Failed(format!("Cannot start shell updater: {error}"));
            }
        }
    }
    pub fn poll(&mut self) -> bool {
        let Some(receiver) = &self.result else {
            return false;
        };
        self.state = match receiver.try_recv() {
            Ok(state) => state,
            Err(TryRecvError::Empty) => {
                let Ok(mut latest) = self.progress.lock() else {
                    return false;
                };
                let Some(state) = latest.take() else {
                    return false;
                };
                state
            }
            Err(TryRecvError::Disconnected) => {
                State::Failed("Shell update worker stopped; check again".into())
            }
        };
        if let State::Failed(error) = &self.state {
            eprintln!("level=warning event=shell_update_failed message={error:?}");
        }
        if !self.state.busy() {
            self.result = None;
        }
        true
    }
}

#[cfg(test)]
fn check(
    transport: &impl Transport,
    current: &str,
    target: impl FnOnce() -> Result<Target, String>,
) -> Result<State, String> {
    check_inventory(transport, current, target, false)
}

fn check_inventory(
    transport: &impl Transport,
    current: &str,
    target: impl FnOnce() -> Result<Target, String>,
    incomplete: bool,
) -> Result<State, String> {
    let current = Version::parse(current).map_err(|_| "Installed build has an invalid version")?;
    let mut releases = Vec::new();
    // GitHub's /latest can designate a lower maintenance release. Inspect all
    // pages, with an explicit cap; never claim current after a partial listing.
    for page in 1..=10 {
        let mut bytes = Vec::new();
        transport.fetch(
            &format!("{}?per_page=100&page={page}", release::API),
            2 * 1024 * 1024,
            &mut bytes,
        )?;
        let values: Vec<Value> = serde_json::from_slice(&bytes)
            .map_err(|_| "GitHub returned malformed release metadata")?;
        let complete = values.len() < 100;
        releases.extend(values);
        if complete {
            let (latest, version) = release::latest(&releases, &current)?;
            if version.cmp_precedence(&current).is_lt()
                || (version.cmp_precedence(&current).is_eq()
                    && !(incomplete && current.to_string() == "0.1.0-beta4"))
            {
                return Ok(State::Current);
            }
            let available = version.to_string();
            return target()
                .and_then(|target| release::select(latest, version, target.artifact()))
                .map(State::Available)
                .map_err(|error| format!("Version {available} available; {error}"));
        }
    }
    Err("Too many release pages; cannot determine the newest version safely".into())
}

#[cfg(unix)]
fn install(
    transport: &impl Transport,
    release: &Release,
    progress: &mut dyn FnMut(State),
) -> Result<(bool, Relaunch), String> {
    let target = Target::current()?;
    if target.artifact() != release.name {
        return Err("Update platform changed; check again".into());
    }
    let mut installation = crate::platform::update::Installation::current()?;
    let mut file = installation.payload()?;
    let sha256 = download(transport, release, &mut file, progress)?;
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    installation.ready(file, &release.version, target, sha256)?;
    let durable = installation.commit()?;
    Ok((durable, installation.relaunch_target()?))
}
#[cfg(not(unix))]
fn install(
    _transport: &impl Transport,
    _release: &Release,
    _progress: &mut dyn FnMut(State),
) -> Result<(bool, Relaunch), String> {
    Err("Shell installation is unsupported on this operating system".into())
}

fn download(
    transport: &impl Transport,
    release: &Release,
    file: &mut std::fs::File,
    progress: &mut dyn FnMut(State),
) -> Result<[u8; 32], String> {
    let mut expected = release.binary.digest.clone();
    if let Some(asset) = &release.checksum {
        let mut bytes = Vec::new();
        transport.fetch(&asset.url, asset.size, &mut bytes)?;
        verify_bytes(&bytes, asset)?;
        let checksum = release::checksum(&bytes, &release.name)?;
        if expected.as_ref().is_some_and(|digest| *digest != checksum) {
            return Err("Release checksums disagree; install refused".into());
        }
        expected = Some(checksum);
    }
    let expected = expected.ok_or("No checksum available; install refused")?;
    transport.fetch(
        &release.binary.url,
        release.binary.size,
        &mut DownloadWriter {
            output: file,
            received: 0,
            total: release.binary.size,
            progress,
        },
    )?;
    if file.metadata().map_err(|e| e.to_string())?.len() != release.binary.size {
        return Err("Shell download is incomplete".into());
    }
    progress(State::Installing);
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 16384];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|e| format!("Verify download: {e}"))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let digest: [u8; 32] = hash.finalize().into();
    if hex_digest(&digest) != expected {
        return Err("Shell SHA-256 verification failed; install refused".into());
    }
    Ok(digest)
}
fn verify_bytes(bytes: &[u8], asset: &release::Asset) -> Result<(), String> {
    if bytes.len() as u64 != asset.size {
        return Err("Checksum download is incomplete".into());
    }
    if asset
        .digest
        .as_ref()
        .is_some_and(|digest| *digest != hex_digest(&Sha256::digest(bytes)))
    {
        return Err("Checksum file verification failed".into());
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|byte| {
            [
                char::from(HEX[usize::from(byte >> 4)]),
                char::from(HEX[usize::from(byte & 15)]),
            ]
        })
        .collect()
}

// Decimal megabytes, rounded to two places without floating-point conversions.
fn megabytes(bytes: u64) -> String {
    let hundredths = bytes.saturating_add(5_000) / 10_000;
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

struct DownloadWriter<'a> {
    output: &'a mut dyn Write,
    received: u64,
    total: u64,
    progress: &'a mut dyn FnMut(State),
}
impl Write for DownloadWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.output.write(bytes)?;
        self.received += count as u64;
        if count != 0 {
            (self.progress)(State::Downloading {
                received: self.received,
                total: self.total,
            });
        }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

#[cfg(test)]
pub mod tests;
