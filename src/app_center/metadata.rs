//! Strict catalog and manifest contracts. Metadata is data, never executable code.
use super::sources::Repository;
use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
pub const CATALOG_LIMIT: usize = 8 * 1024 * 1024;
pub const FILE_LIMIT: usize = 2 * 1024 * 1024;
pub const BUNDLE_LIMIT: usize = 16 * 1024 * 1024;
pub type Files = BTreeMap<String, Vec<u8>>;

struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueVisitor)
    }
}
struct UniqueVisitor;
impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = Unique;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JSON without duplicate keys")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
        let mut out = Map::new();
        while let Some((key, Unique(value))) = map.next_entry::<String, Unique>()? {
            if out.insert(key, value).is_some() {
                return Err(de::Error::custom("duplicate JSON key"));
            }
        }
        Ok(Unique(Value::Object(out)))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
        let mut out = Vec::new();
        while let Some(Unique(value)) = seq.next_element()? {
            out.push(value);
        }
        Ok(Unique(Value::Array(out)))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
        Ok(Unique(v.into()))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
        Ok(Unique(Value::Null))
    }
}
pub fn json(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > CATALOG_LIMIT {
        return Err("JSON exceeds 8 MiB".into());
    }
    serde_json::from_slice::<Unique>(bytes)
        .map(|v| v.0)
        .map_err(|e| e.to_string())
}
pub fn fields(value: &Value, keys: &str) -> Result<(), String> {
    let map = value.as_object().ok_or("expected object")?;
    if map.len() != keys.split_whitespace().count()
        || keys.split_whitespace().any(|k| !map.contains_key(k))
    {
        return Err(format!("missing or unknown fields; expected {keys}"));
    }
    Ok(())
}
pub fn text(value: &Value, max: usize) -> Result<&str, String> {
    let s = value.as_str().ok_or("expected text")?;
    if s.is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
        return Err("invalid text".into());
    }
    Ok(s)
}
pub fn path(s: &str) -> Result<(), String> {
    if s.len() > 240
        || s.split('/').any(|p| {
            p.is_empty()
                || p == "."
                || p == ".."
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        })
    {
        return Err(format!("unsafe package path: {s}"));
    }
    Ok(())
}
pub fn identity(s: &str) -> Result<(), String> {
    if s.len() > 128
        || !s.contains('.')
        || s.split('.').any(|p| {
            !p.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
    {
        return Err("invalid app ID".into());
    }
    Ok(())
}
/// Stable numeric catalog versions allow up to 32 characters, including components
/// larger than u64. Compare component length before digits, never lexicographic labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version([String; 3]);
impl Version {
    pub fn zero() -> Self {
        Self(["0".into(), "0".into(), "0".into()])
    }
}
impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0[0], self.0[1], self.0[2])
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .iter()
            .zip(&other.0)
            .map(|(a, b)| a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
            .find(|c| *c != std::cmp::Ordering::Equal)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
pub fn version(s: &str) -> Result<Version, String> {
    let parts: Vec<_> = s.split('.').collect();
    if s.len() > 32
        || parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || !p.bytes().all(|b| b.is_ascii_digit())
                || p.len() > 1 && p.starts_with('0')
        })
    {
        return Err("Expected stable MAJOR.MINOR.PATCH".into());
    }
    Ok(Version([parts[0].into(), parts[1].into(), parts[2].into()]))
}
pub fn hex(s: &str, len: usize) -> Result<(), String> {
    if s.len() != len
        || !s
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("invalid pinned checksum/revision".into());
    }
    Ok(())
}
pub fn check_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut files = BTreeSet::new();
    let mut spellings = BTreeMap::new();
    for name in paths {
        path(name)?;
        if !files.insert(name.to_ascii_lowercase()) {
            return Err("duplicate/colliding file path".into());
        }
        let mut prefix = String::new();
        for part in name.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if spellings
                .insert(prefix.to_ascii_lowercase(), prefix.clone())
                .is_some_and(|old| old != prefix)
            {
                return Err("case-colliding directory".into());
            }
        }
    }
    for name in &files {
        for (i, _) in name.match_indices('/') {
            if files.contains(&name[..i]) {
                return Err("file/directory collision".into());
            }
        }
    }
    Ok(())
}
#[derive(Debug, Clone)]
pub struct FileRow {
    pub path: String,
    pub size: usize,
    pub sha256: String,
}
#[derive(Debug, Clone)]
pub struct Package {
    pub origin: Repository,
    pub id: String,
    pub name: String,
    pub version: Version,
    pub entry: String,
    pub permissions: Value,
    pub installable: bool,
    pub notes: String,
    pub repository: Repository,
    pub commit: String,
    pub directory: String,
    pub files: Vec<FileRow>,
}
impl Package {
    pub fn key(&self) -> String {
        format!("{}:{}", self.origin.as_str(), self.id)
    }
}
pub fn catalog(origin: &Repository, bytes: &[u8]) -> Result<Vec<Package>, String> {
    let doc = json(bytes)?;
    fields(&doc, "schema_version apps")?;
    if doc["schema_version"].as_u64() != Some(1) {
        return Err("unsupported catalog schema".into());
    }
    let apps = doc["apps"]
        .as_array()
        .filter(|a| a.len() <= 1000)
        .ok_or("invalid app list")?;
    let mut ids = BTreeSet::new();
    apps.iter()
        .map(|v| {
            let p = parse_package(origin, v)?;
            if !ids.insert(p.id.clone()) {
                return Err("Duplicate app ID".into());
            }
            Ok(p)
        })
        .collect()
}
fn parse_package(origin: &Repository, v: &Value) -> Result<Package, String> {
    fields(
        v,
        "id name version description runtime entry permissions installable compatibility_notes source files",
    )?;
    let id = text(&v["id"], 128)?.to_owned();
    identity(&id)?;
    let name = text(&v["name"], 1000)?.to_owned();
    text(&v["description"], 1000)?;
    let notes = text(&v["compatibility_notes"], 1000)?.to_owned();
    let version = version(text(&v["version"], 32)?)?;
    if v["runtime"] != "python" {
        return Err("Unsupported runtime".into());
    }
    let entry = text(&v["entry"], 240)?.to_owned();
    path(&entry)?;
    permissions(&v["permissions"])?;
    let installable = v["installable"]
        .as_bool()
        .ok_or("Invalid installable flag")?;
    let (repository, commit, directory) = parse_source(&v["source"])?;
    let files = parse_inventory(&v["files"])?;
    if !files.iter().any(|f| f.path == entry) {
        return Err("Entry missing from inventory".into());
    }
    Ok(Package {
        origin: origin.clone(),
        id,
        name,
        version,
        entry,
        permissions: v["permissions"].clone(),
        installable,
        notes,
        repository,
        commit,
        directory,
        files,
    })
}
fn parse_source(source: &Value) -> Result<(Repository, String, String), String> {
    fields(source, "repository commit path")?;
    let name = text(&source["repository"], 140)?;
    if name != name.trim() || name.contains("://") || name.ends_with('/') {
        return Err("Catalog repository must be owner/repo".into());
    }
    let repository = Repository::parse(name)?;
    let commit = text(&source["commit"], 40)?.to_owned();
    hex(&commit, 40)?;
    let directory = text(&source["path"], 240)?.to_owned();
    path(&directory)?;
    let (base, slug) = directory
        .split_once('/')
        .ok_or("Invalid source directory")?;
    let canonical = base == "apps"
        && slug.split('-').all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if !canonical {
        return Err("Invalid source directory".into());
    }
    Ok((repository, commit, directory))
}
fn parse_inventory(v: &Value) -> Result<Vec<FileRow>, String> {
    let rows = v
        .as_array()
        .filter(|r| !r.is_empty() && r.len() <= 256)
        .ok_or("Invalid inventory")?;
    let files = rows
        .iter()
        .map(|row| {
            fields(row, "path size sha256")?;
            let path = text(&row["path"], 240)?.to_owned();
            let size = row["size"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n <= FILE_LIMIT)
                .ok_or("Invalid file size")?;
            let sha256 = text(&row["sha256"], 64)?.to_owned();
            hex(&sha256, 64)?;
            Ok(FileRow { path, size, sha256 })
        })
        .collect::<Result<Vec<_>, String>>()?;
    check_paths(files.iter().map(|f| f.path.as_str()))?;
    if files.iter().any(|f| f.path.starts_with("tests/")) {
        return Err("App-local tests must be excluded from device packages".into());
    }
    if files.windows(2).any(|rows| rows[0].path >= rows[1].path) {
        return Err("Inventory must be sorted by ASCII path".into());
    }
    if files.iter().map(|f| f.size).sum::<usize>() > BUNDLE_LIMIT {
        return Err("Bundle exceeds 16 MiB".into());
    }
    Ok(files)
}
fn permissions(value: &Value) -> Result<(), String> {
    fields(value, "network audio storage")?;
    if ["network", "audio", "storage"]
        .iter()
        .any(|k| !value[k].is_boolean())
    {
        return Err("invalid permissions".into());
    }
    Ok(())
}
pub fn manifest(bytes: &[u8]) -> Result<Value, String> {
    let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let parsed: toml::Value = toml::from_str(s).map_err(|e| e.to_string())?;
    let v = serde_json::to_value(parsed).map_err(|e| e.to_string())?;
    fields(
        &v,
        "manifest_version name id version runtime entry permissions",
    )?;
    if v["manifest_version"].as_u64() != Some(1) || v["runtime"] != "python" {
        return Err("unsupported manifest/runtime".into());
    }
    text(&v["name"], 1000)?;
    identity(text(&v["id"], 128)?)?;
    version(text(&v["version"], 32)?)?;
    path(text(&v["entry"], 240)?)?;
    permissions(&v["permissions"])?;
    if std::path::Path::new(text(&v["entry"], 240)?).extension() != Some(std::ffi::OsStr::new("py"))
    {
        return Err("entry must be Python".into());
    }
    Ok(v)
}
pub fn validate_bundle(p: &Package, files: &Files) -> Result<(), String> {
    if files.len() != p.files.len() {
        return Err("incomplete inventory".into());
    }
    if p.files.iter().any(|f| f.path.starts_with("tests/")) {
        return Err("App-local tests must be excluded from device packages".into());
    }
    for row in &p.files {
        let bytes = files.get(&row.path).ok_or("missing file")?;
        if bytes.len() != row.size || super::storage::sha(bytes) != row.sha256 {
            return Err(format!("size/SHA-256 mismatch: {}", row.path));
        }
    }
    for name in [
        "app.toml",
        "icon.png",
        "main.py",
        "requirements.txt",
        "README.md",
    ] {
        if !files.contains_key(name) {
            return Err(format!("missing {name}"));
        }
    }
    // Development tests are required in the source repository, but current
    // catalog v1 device packages omit them. Assets remain part of the payload.
    if !files.keys().any(|p| p.starts_with("assets/")) {
        return Err("missing populated assets/".into());
    }
    let v = manifest(&files["app.toml"])?;
    if v["id"] != p.id
        || v["name"] != p.name
        || v["version"] != p.version.to_string()
        || v["entry"] != p.entry
        || v["permissions"] != p.permissions
    {
        return Err("catalog/manifest disagreement".into());
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&files["icon.png"]));
    decoder.set_limits(png::Limits {
        bytes: 4 * 1024 * 1024,
    });
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    if !(1..=512).contains(&info.width) || !(1..=512).contains(&info.height) || info.interlaced {
        return Err("invalid icon dimensions/interlacing".into());
    }
    let mut pixels = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    reader.finish().map_err(|e| e.to_string())?;

    Ok(())
}
