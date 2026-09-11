//! Shell-only self-update orchestration. No app discovery, configuration or manifests.
mod release;
mod transport;

use crate::platform::update::Target;
use release::Release;
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom},
    sync::mpsc::{self, Receiver, TryRecvError},
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
    Installing,
    Installed {
        version: Version,
        durable: bool,
    },
    Failed(String),
}
impl State {
    pub const fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Installing)
    }
    pub fn detail(&self) -> String {
        match self {
            Self::Idle => "Check the official stable shell releases".into(),
            Self::Checking => "Checking GitHub for shell updates...".into(),
            Self::Current => "Vitrallis is up to date.".into(),
            Self::Available(release) => format!("New version available: {}", release.version),
            Self::Installing => "Downloading and verifying shell update...".into(),
            Self::Installed {
                version,
                durable: true,
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
}
impl Updater {
    pub fn check(&mut self) {
        if self.state.busy() || matches!(self.state, State::Installed { .. }) {
            return;
        }
        self.start(State::Checking, || {
            check(&Curl, VERSION, Target::current)
                .unwrap_or_else(|error| State::Failed(format!("Update check failed: {error}")))
        });
    }
    pub fn install(&mut self) {
        let State::Available(release) = &self.state else {
            return;
        };
        let release = release.clone();
        self.start(State::Installing, move || {
            install(&Curl, &release).map_or_else(
                |error| State::Failed(format!("Update failed: {error}")),
                |durable| State::Installed {
                    version: release.version,
                    durable,
                },
            )
        });
    }
    fn start(&mut self, state: State, work: impl FnOnce() -> State + Send + 'static) {
        let (send, receive) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("shell-update".into())
            .spawn(move || {
                let _ = send.send(work());
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
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                State::Failed("Shell update worker stopped; check again".into())
            }
        };
        if let State::Failed(error) = &self.state {
            eprintln!("level=warning event=shell_update_failed message={error:?}");
        }
        self.result = None;
        true
    }
}

fn check(
    transport: &impl Transport,
    current: &str,
    target: impl FnOnce() -> Result<Target, String>,
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
            let (latest, version) = release::latest(&releases)?;
            if !version.cmp_precedence(&current).is_gt() {
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
fn install(transport: &impl Transport, release: &Release) -> Result<bool, String> {
    let target = Target::current()?;
    if target.artifact() != release.name {
        return Err("Update platform changed; check again".into());
    }
    let installation = crate::platform::update::Installation::current()?;
    let mut file = installation.payload()?;
    download(transport, release, &mut file)?;
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut header = [0; 64];
    file.read_exact(&mut header)
        .map_err(|_| "Incomplete executable header")?;
    target.verify_header(&header)?;
    installation.ready(file, &release.version)?;
    installation.commit()
}
#[cfg(not(unix))]
fn install(_transport: &impl Transport, _release: &Release) -> Result<bool, String> {
    Err("Shell installation is unsupported on this operating system".into())
}

fn download(
    transport: &impl Transport,
    release: &Release,
    file: &mut std::fs::File,
) -> Result<(), String> {
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
    transport.fetch(&release.binary.url, release.binary.size, file)?;
    if file.metadata().map_err(|e| e.to_string())?.len() != release.binary.size {
        return Err("Shell download is incomplete".into());
    }
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
    if hex_digest(&hash.finalize()) != expected {
        return Err("Shell SHA-256 verification failed; install refused".into());
    }
    Ok(())
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

#[cfg(test)]
pub mod tests;
