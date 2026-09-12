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
}
pub struct Curl;
impl Fetch for Curl {
    fn fetch(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
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
            let mut bytes = Vec::new();
            out.take(u64::try_from(limit).map_err(|e| e.to_string())? + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > limit {
                return Err("Download exceeds byte limit".into());
            }
            Ok(bytes)
        })();
        if result.is_err() {
            let _ = child.kill();
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!(
                "GitHub request failed ({status}); check connection/rate limit"
            ));
        }
        result
    }
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
            use std::fmt::Write;
            let _ = write!(out, "%{c:02X}");
        }
    }
    out
}
pub fn catalog(fetch: &impl Fetch, repo: &Repository) -> Result<Vec<Package>, String> {
    let info = api(fetch, repo.as_str())?;
    let branch = metadata::text(&info["default_branch"], 255)?;
    let commit = api(
        fetch,
        &format!("{}/commits/{}", repo.as_str(), encode(branch)),
    )?;
    let sha = metadata::text(&commit["sha"], 40)?;
    metadata::hex(sha, 40)?;
    let url = format!(
        "https://raw.githubusercontent.com/{}/{sha}/apps.json",
        repo.as_str()
    );
    metadata::catalog(repo, &fetch.fetch(&url, metadata::CATALOG_LIMIT)?)
}
pub fn bundle(
    fetch: &impl Fetch,
    p: &Package,
    mut progress: impl FnMut(String),
) -> Result<Files, String> {
    // Resolve the exact directory tree without relying on a possibly truncated recursive repository tree.
    let mut tree = p.commit.clone();
    for component in p.directory.split('/') {
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
    let mut inventory = std::collections::BTreeMap::new();
    for entry in tree_entries(&v)? {
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
    }
    if inventory.len() != p.files.len()
        || p.files
            .iter()
            .any(|r| inventory.get(r.path.as_str()).copied() != u64::try_from(r.size).ok())
    {
        return Err("Catalog must enumerate the complete pinned directory".into());
    }
    let mut files = Files::new();
    for (index, row) in p.files.iter().enumerate() {
        progress(format!(
            "{}: verifying {}/{} {}",
            p.name,
            index + 1,
            p.files.len(),
            row.path
        ));
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            p.repository.as_str(),
            p.commit,
            p.directory,
            row.path
        );
        let bytes = fetch.fetch(&url, row.size)?;
        if bytes.len() != row.size || super::storage::sha(&bytes) != row.sha256 {
            return Err(format!("SHA-256/size mismatch: {}", row.path));
        }
        files.insert(row.path.clone(), bytes);
    }
    metadata::validate_bundle(p, &files)?;
    Ok(files)
}
fn tree_entries(v: &serde_json::Value) -> Result<&Vec<serde_json::Value>, String> {
    if v["truncated"].as_bool() != Some(false) {
        return Err("Truncated/unverified Git tree".into());
    }
    v["tree"]
        .as_array()
        .ok_or_else(|| "Invalid Git tree".into())
}
