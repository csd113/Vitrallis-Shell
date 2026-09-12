//! Persistent supplemental GitHub catalogs; the built-in origin cannot be removed.
use super::{metadata, storage};
use std::{collections::BTreeSet, path::Path};
pub const DEFAULT: &str = "csd113/vitrallis-apps";
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Repository(String);
impl Repository {
    pub fn parse(input: &str) -> Result<Self, String> {
        let s = input.trim();
        let s = s
            .strip_prefix("https://github.com/")
            .unwrap_or(s)
            .trim_end_matches('/');
        let s = s.strip_suffix(".git").unwrap_or(s);
        let (owner, repo) = s
            .split_once('/')
            .ok_or("Use owner/repo or https://github.com/owner/repo")?;
        if owner.is_empty()
            || owner.len() > 39
            || owner.starts_with('-')
            || owner.ends_with('-')
            || owner.contains("--")
            || !owner
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || repo.is_empty()
            || repo.len() > 100
            || matches!(repo, "." | "..")
            || !repo
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        {
            return Err("Invalid GitHub repository".into());
        }
        Ok(Self(format!("{owner}/{repo}").to_ascii_lowercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    pub catalogs: Vec<Repository>,
    pub approvals: BTreeSet<(Repository, Repository)>,
}
impl Default for Sources {
    fn default() -> Self {
        Self {
            catalogs: vec![Repository(DEFAULT.into())],
            approvals: BTreeSet::new(),
        }
    }
}
impl Sources {
    pub fn batch(input: &str) -> Result<Vec<Repository>, String> {
        if input.len() > 8192 {
            return Err("Repository entry exceeds 8 KiB".into());
        }
        let mut repos = BTreeSet::new();
        for token in input
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|s| !s.is_empty())
        {
            repos.insert(Repository::parse(token)?);
        }
        if repos.is_empty() {
            return Err("Enter at least one repository".into());
        }
        Ok(repos.into_iter().collect())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let Some(file) = storage::read(path, 65536)? else {
            return Ok(Self::default());
        };
        let v = metadata::json(&file.bytes)?;
        metadata::fields(&v, "version catalogs approvals")?;
        if v["version"].as_u64() != Some(1) {
            return Err("Unsupported source settings".into());
        }
        let mut result = Self::default();
        let catalogs = v["catalogs"]
            .as_array()
            .filter(|a| a.len() <= 32)
            .ok_or("Invalid sources")?;
        for v in catalogs {
            let repo = Repository::parse(metadata::text(v, 160)?)?;
            if !result.catalogs.contains(&repo) {
                result.catalogs.push(repo);
            }
        }
        let approvals = v["approvals"]
            .as_array()
            .filter(|a| a.len() <= 128)
            .ok_or("Invalid approvals")?;
        for v in approvals {
            let pair = v
                .as_array()
                .filter(|a| a.len() == 2)
                .ok_or("Invalid source approval")?;
            let origin = Repository::parse(metadata::text(&pair[0], 160)?)?;
            let source = Repository::parse(metadata::text(&pair[1], 160)?)?;
            if result.catalogs.contains(&origin) {
                result.approvals.insert((origin, source));
            }
        }
        Ok(result)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.catalogs.len() > 32 || self.approvals.len() > 128 {
            return Err("Source settings limit exceeded".into());
        }
        let v = serde_json::json!({"version":1, "catalogs": self.catalogs.iter().map(Repository::as_str).collect::<Vec<_>>(), "approvals":self.approvals.iter().map(|(a,b)| [a.as_str(),b.as_str()]).collect::<Vec<_>>()});
        let bytes = serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?;
        storage::atomic(path, &storage::FileData { bytes, mode: 0o600 })
    }
    pub fn trusted(&self, origin: &Repository, source: &Repository) -> bool {
        origin == source || self.approvals.contains(&(origin.clone(), source.clone()))
    }
    pub fn remove(&mut self, index: usize) {
        if index > 0 && index < self.catalogs.len() {
            let old = self.catalogs.remove(index);
            self.approvals.retain(|(origin, _)| origin != &old);
        }
    }
    pub fn edit(&mut self, index: Option<usize>, text: &str) -> Result<(), String> {
        let repos = Self::batch(text)?;
        let mut next = self.clone();
        if let Some(index) = index {
            if index == 0 {
                return Err("The default catalog is always included".into());
            }
            next.remove(index);
        }
        for repo in repos {
            if !next.catalogs.contains(&repo) {
                next.catalogs.push(repo);
            }
        }
        if next.catalogs.len() > 32 {
            return Err("At most 32 catalogs".into());
        }
        *self = next;
        Ok(())
    }
}
