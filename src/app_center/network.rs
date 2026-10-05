//! GitHub-only HTTPS requests, bounded by bytes and a total transfer deadline.
use super::{
    metadata::{self, Files, Package},
    sources::Repository,
};
use std::{
    io::Read,
    process::{Command, Stdio},
};
pub trait Fetch {
    fn fetch(&self, url: &str, limit: usize) -> Result<Vec<u8>, String>;
    fn fetch_progress(
        &self,
        url: &str,
        limit: usize,
        progress: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<Vec<u8>, String> {
        progress(0)?;
        let bytes = self.fetch(url, limit)?;
        progress(bytes.len())?;
        Ok(bytes)
    }
}
pub struct Curl;
impl Fetch for Curl {
    fn fetch(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        self.fetch_progress(url, limit, &mut |_| Ok(()))
    }
    fn fetch_progress(
        &self,
        url: &str,
        limit: usize,
        progress: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<Vec<u8>, String> {
        progress(0)?;
        if !(url.starts_with("https://api.github.com/repos/")
            || url.starts_with("https://raw.githubusercontent.com/"))
            || url.chars().any(char::is_control)
        {
            return Err("Download host/HTTPS policy rejected URL".into());
        }
        let mut child = Command::new("/usr/bin/curl")
            .args([
                "-q",
                "--fail",
                "--silent",
                "--location",
                "--max-redirs",
                "0",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                "10",
                "--max-time",
                "30",
                "--max-filesize",
                &limit.max(1).to_string(),
                "--user-agent",
                "Vitrallis-App-Center",
                "--url",
                url,
            ])
            .env_remove("CURL_CA_BUNDLE")
            .env_remove("SSL_CERT_FILE")
            .env_remove("SSL_CERT_DIR")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let result = (|| {
            let out = child.stdout.take().ok_or("Missing download stream")?;
            read_download(out, limit, progress)
        })();
        if result.is_err()
            && let Err(error) = child.kill()
            && error.kind() != std::io::ErrorKind::InvalidInput
        {
            eprintln!(
                "level=error event=download_kill pid={} message={error:?}",
                child.id()
            );
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        let bytes = result?;
        if !status.success() {
            // curl's exit code 28 means the transfer deadline was reached.
            if status.code() == Some(28_i32) {
                return Err(format!(
                    "Download timed out; check connection and retry\ncurl: {status}"
                ));
            }
            return Err(format!(
                "GitHub request failed ({status}); check connection/rate limit"
            ));
        }
        Ok(bytes)
    }
}
/// Report cumulative bytes only after they have actually been read from the pipe.
pub(super) fn read_download(
    mut reader: impl Read,
    limit: usize,
    progress: &mut dyn FnMut(usize) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 16 * 1024];
    loop {
        progress(bytes.len())?;
        let remaining = limit
            .saturating_sub(bytes.len())
            .saturating_add(1)
            .min(chunk.len());
        let part = chunk
            .get_mut(..remaining)
            .ok_or("Invalid download buffer length")?;
        let count = match reader.read(part) {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result.map_err(|e| e.to_string())?,
        };
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(chunk.get(..count).ok_or("Invalid download read count")?);
        progress(bytes.len())?;
        if bytes.len() > limit {
            return Err("Download exceeds byte limit".into());
        }
    }
    Ok(bytes)
}
fn api(fetch: &impl Fetch, path: &str) -> Result<serde_json::Value, String> {
    metadata::json(&fetch.fetch(
        &format!("https://api.github.com/repos/{path}"),
        metadata::CATALOG_LIMIT,
    )?)
}
fn encode(s: &str) -> String {
    let mut out = String::new();
    for c in s.bytes() {
        if c.is_ascii_alphanumeric() || b"-_.~".contains(&c) {
            out.push(char::from(c));
        } else {
            out.push('%');
            for nibble in [c >> 4_i32, c & 15] {
                out.push(char::from(if nibble < 10 {
                    b'0'.saturating_add(nibble)
                } else {
                    b'A'.saturating_add(nibble.saturating_sub(10))
                }));
            }
        }
    }
    out
}
pub fn catalog_document(fetch: &impl Fetch, repo: &Repository) -> Result<Vec<u8>, String> {
    let info = api(fetch, repo.as_str())?;
    let branch = metadata::text(metadata::field(&info, "default_branch")?, 255)?;
    let commit = api(
        fetch,
        &format!("{}/commits/{}", repo.as_str(), encode(branch)),
    )?;
    let sha = metadata::text(metadata::field(&commit, "sha")?, 40)?;
    metadata::hex(sha, 40)?;
    let url = format!(
        "https://raw.githubusercontent.com/{}/{sha}/apps.json",
        repo.as_str()
    );
    fetch.fetch(&url, metadata::CATALOG_LIMIT)
}
#[cfg(test)]
pub fn catalog(fetch: &impl Fetch, repo: &Repository) -> Result<Vec<Package>, String> {
    metadata::catalog(repo, &catalog_document(fetch, repo)?)
}
pub struct Bundle {
    pub files: Files,
    pub modes: std::collections::BTreeMap<String, u32>,
}
#[cfg(test)]
pub fn bundle(
    fetch: &impl Fetch,
    p: &Package,
    progress: impl FnMut(String) -> Result<(), String>,
) -> Result<Files, String> {
    download(fetch, p, progress).map(|bundle| bundle.files)
}
pub fn download(
    fetch: &impl Fetch,
    p: &Package,
    mut progress: impl FnMut(String) -> Result<(), String>,
) -> Result<Bundle, String> {
    progress(format!("Checking source inventory: {}", p.name))?;
    // Resolve the exact directory tree without relying on a possibly truncated recursive repository tree.
    let mut tree = p.commit.clone();
    for component in p.directory.split('/') {
        progress(format!("Checking source inventory: {}", p.name))?;
        let v = api(
            fetch,
            &format!("{}/git/trees/{tree}", p.repository.as_str()),
        )?;
        let entries = tree_entries(&v)?;
        let matches: Vec<_> = entries.iter().filter(|e| e["path"] == component).collect();
        let entry = matches
            .first()
            .filter(|_| matches.len() == 1)
            .ok_or("Source directory missing/ambiguous")?;
        if entry["mode"] != "040000" || entry["type"] != "tree" {
            return Err("Source path is not a Git directory".into());
        }
        tree = metadata::text(&entry["sha"], 40)?.into();
        metadata::hex(&tree, 40)?;
    }
    let v = api(
        fetch,
        &format!("{}/git/trees/{tree}?recursive=1", p.repository.as_str()),
    )?;
    let modes = check_inventory(&v, p)?;
    download_files(fetch, p, modes, progress)
}
fn check_inventory(
    value: &serde_json::Value,
    package: &Package,
) -> Result<std::collections::BTreeMap<String, u32>, String> {
    let mut inventory = std::collections::BTreeMap::new();
    let mut modes = std::collections::BTreeMap::new();
    for entry in tree_entries(value)? {
        let name = metadata::text(&entry["path"], 240)?;
        metadata::path(name)?;
        if entry["type"] == "tree" && entry["mode"] == "040000" {
            continue;
        }
        if entry["type"] != "blob" || !matches!(entry["mode"].as_str(), Some("100644" | "100755")) {
            return Err("Git symlink/submodule/special file rejected".into());
        }
        let size = entry["size"].as_u64().ok_or("Missing Git size")?;
        if inventory.insert(name, size).is_some() {
            return Err("Duplicate Git path".into());
        }
        if !name.starts_with("tests/") {
            let previous_mode = modes.insert(
                name.to_owned(),
                if entry["mode"] == "100755" {
                    0o755
                } else {
                    0o644
                },
            );
            if previous_mode.is_some() {
                return Err("Duplicate Git mode path".into());
            }
        }
    }
    // Device packages always exclude app-local development tests.
    metadata::check_paths(inventory.keys().copied())?;
    inventory.retain(|name, _| !name.starts_with("tests/"));
    if inventory.len() != package.files.len()
        || package
            .files
            .iter()
            .any(|r| inventory.get(r.path.as_str()).copied() != u64::try_from(r.size).ok())
    {
        return Err(
            "Catalog must enumerate the pinned package; only app-local tests/ may be omitted"
                .into(),
        );
    }
    Ok(modes)
}
fn download_files(
    fetch: &impl Fetch,
    p: &Package,
    modes: std::collections::BTreeMap<String, u32>,
    mut progress: impl FnMut(String) -> Result<(), String>,
) -> Result<Bundle, String> {
    let mut files = Files::new();
    let total = p
        .files
        .iter()
        .try_fold(0_usize, |size, file| size.checked_add(file.size))
        .ok_or("Package download size overflow")?;
    if total > metadata::BUNDLE_LIMIT {
        return Err("Package download exceeds the bundle byte limit".into());
    }
    let mut downloaded = 0_usize;
    for row in &p.files {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            p.repository.as_str(),
            p.commit,
            p.directory,
            row.path
        );
        let bytes = fetch
            .fetch_progress(&url, row.size, &mut |received| {
                let cumulative = downloaded
                    .checked_add(received)
                    .ok_or("Download progress overflow")?;
                let percent = cumulative
                    .checked_mul(100)
                    .ok_or("Download percentage overflow")?
                    .checked_div(total)
                    .unwrap_or(100);
                progress(format!(
                    "Downloading: {cumulative} / {total} bytes ({percent}%)\n{}",
                    p.name
                ))
            })
            .map_err(|e| format!("{e}\nDownload failed for {}", row.path))?;
        if bytes.len() != row.size || super::storage::sha(&bytes) != row.sha256 {
            return Err(format!(
                "Package invalid: SHA-256/size mismatch: {}",
                row.path
            ));
        }
        downloaded = downloaded
            .checked_add(bytes.len())
            .ok_or("Download length overflow")?;
        if files.insert(row.path.clone(), bytes).is_some() {
            return Err("Duplicate package download path".into());
        }
    }
    progress(format!("Verifying {}", p.name))?;
    metadata::validate_bundle(p, &files)?;
    Ok(Bundle { files, modes })
}
fn tree_entries(v: &serde_json::Value) -> Result<&Vec<serde_json::Value>, String> {
    if v["truncated"].as_bool() != Some(false) {
        return Err("Truncated/unverified Git tree".into());
    }
    v["tree"]
        .as_array()
        .ok_or_else(|| "Invalid Git tree".into())
}
