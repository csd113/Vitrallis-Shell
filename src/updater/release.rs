//! GitHub metadata policy; contains no installation paths or app catalogue access.
use semver::Version;
use serde_json::Value;

pub(super) const API: &str = "https://api.github.com/repos/csd113/Vitrallis-Shell/releases";
const DOWNLOADS: &str = "https://github.com/csd113/Vitrallis-Shell/releases/download";
pub(super) const MAX_BINARY: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Asset {
    pub(super) url: String,
    pub(super) size: u64,
    pub(super) digest: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Release {
    pub version: Version,
    pub(super) name: String,
    pub(super) binary: Asset,
    pub(super) checksum: Option<Asset>,
}

impl Release {
    pub fn download_size_mb(&self) -> String {
        super::megabytes(self.binary.size)
    }
}

pub(super) fn latest<'a>(
    releases: &'a [Value],
    current: &Version,
) -> Result<(&'a Value, Version), String> {
    // Prerelease builds follow published previews through to a stable release.
    // Stable installations keep their existing stable-only update policy.
    let previews = !current.pre.is_empty();
    let mut latest: Option<(&Value, Version)> = None;
    for release in releases {
        let draft = flag(release, "draft")?;
        let prerelease = flag(release, "prerelease")?;
        if draft || (prerelease && !previews) {
            continue;
        }
        let tag = field(release, "tag_name")?;
        if tag.len() > 128 {
            return Err("Release version is too long".into());
        }
        let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
            .map_err(|_| "Release has an invalid semantic version")?;
        if !version.pre.is_empty() && !previews {
            continue;
        }
        if latest
            .as_ref()
            .is_none_or(|(_, old)| version.cmp_precedence(old).is_gt())
        {
            latest = Some((release, version));
        }
    }
    latest.ok_or_else(|| {
        if previews {
            "No shell release is published yet"
        } else {
            "No stable shell release is published yet"
        }
        .into()
    })
}

pub(super) fn select(release: &Value, version: Version, name: String) -> Result<Release, String> {
    let tag = field(release, "tag_name")?;
    let assets = release["assets"]
        .as_array()
        .ok_or("Release assets are missing")?;
    let binary = asset(assets, tag, &name, MAX_BINARY)?
        .ok_or("No shell build is available for this platform")?;
    let checksum = asset(assets, tag, &format!("{name}.sha256"), 1024)?;
    if binary.digest.is_none() && checksum.is_none() {
        return Err("Release has no SHA-256 verification; install refused".into());
    }
    Ok(Release {
        version,
        name,
        binary,
        checksum,
    })
}

fn asset(assets: &[Value], tag: &str, name: &str, limit: u64) -> Result<Option<Asset>, String> {
    let mut matches = assets
        .iter()
        .filter(|asset| asset["name"].as_str() == Some(name));
    let Some(value) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() || field(value, "state")? != "uploaded" {
        return Err("Release contains duplicate or incomplete assets".into());
    }
    let url = field(value, "browser_download_url")?;
    if url != format!("{DOWNLOADS}/{tag}/{name}") {
        return Err("Release artifact is not from the official repository".into());
    }
    let size = value["size"]
        .as_u64()
        .filter(|size| *size > 0 && *size <= limit)
        .ok_or("Release artifact has an invalid size")?;
    let digest = if value["digest"].is_null() {
        None
    } else {
        Some(parse_digest(
            field(value, "digest")?
                .strip_prefix("sha256:")
                .ok_or("Release digest is not SHA-256")?,
        )?)
    };
    Ok(Some(Asset {
        url: url.into(),
        size,
        digest,
    }))
}

pub(super) fn parse_digest(value: &str) -> Result<String, String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid SHA-256 checksum".into());
    }
    Ok(value.to_ascii_lowercase())
}

pub(super) fn checksum(bytes: &[u8], name: &str) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Checksum is not UTF-8")?;
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() != 2 || fields[1].strip_prefix('*').unwrap_or(fields[1]) != name {
        return Err("Checksum does not identify the shell artifact".into());
    }
    parse_digest(fields[0])
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("Release field {name} is invalid"))
}
fn flag(value: &Value, name: &str) -> Result<bool, String> {
    value[name]
        .as_bool()
        .ok_or_else(|| format!("Release field {name} is invalid"))
}
