//! Remote snapshots and optional presentation files have their own invalidation.
//! Reopening, local scans and mutations never fetch repository metadata.
use super::{
    Checked, Sources, metadata, network, refresh_local, source_error,
    sources::Repository,
    storage::{self, FileData, Locations},
};
use std::{collections::BTreeMap, path::PathBuf};

fn snapshot(loc: &Locations, origin: &Repository) -> PathBuf {
    loc.state
        .join("catalogs")
        .join(format!("{}.json", storage::sha(origin.as_str().as_bytes())))
}

pub(super) fn load(loc: &Locations, sources: &Sources) -> Vec<Checked> {
    let mut rows = Vec::new();
    for origin in &sources.catalogs {
        match storage::read(&snapshot(loc, origin), metadata::CATALOG_LIMIT) {
            Ok(Some(file)) => match metadata::catalog_entries(origin, &file.bytes) {
                Ok(entries) => rows.extend(parse(loc, sources, origin, entries, &BTreeMap::new())),
                Err(error) => rows.push(source_error(
                    origin,
                    &format!("Cached repository invalid: {error}"),
                )),
            },
            Ok(None) => (),
            Err(error) => rows.push(source_error(origin, &error)),
        }
    }
    rows
}

pub(super) struct Fetched {
    origin: Repository,
    document: Result<Document, String>,
}
struct Document {
    snapshot: Vec<u8>,
    entries: Vec<Result<metadata::Package, String>>,
    presentations: BTreeMap<PathBuf, FileData>,
}

/// Total raw CHANGELOG/icon bytes one unlocked refresh may stage in memory
/// before the writer phase. A full 1000-entry catalogue of maximum-size icons
/// would otherwise hold hundreds of megabytes on a device with ~512 MiB of
/// RAM; skipped presentation files are retried on the next refresh.
pub(super) const PRESENTATION_BUDGET: usize = 16 * 1024 * 1024;

/// Network-only phase of a refresh: no storage lock and no durable writes.
/// Presentation bytes are staged for `store_documents`; per-origin request
/// order and the on-disk cache predicate are identical to the old refresh.
pub(super) fn fetch_documents(
    loc: &Locations,
    sources: &Sources,
    fetch: &impl network::Fetch,
    mut progress: impl FnMut(String),
) -> Vec<Fetched> {
    let mut fetched = Vec::with_capacity(sources.catalogs.len());
    let mut budget = PRESENTATION_BUDGET;
    for origin in &sources.catalogs {
        progress(format!("Refreshing {}", origin.as_str()));
        fetched.push(Fetched {
            origin: origin.clone(),
            document: document(loc, sources, origin, fetch, &mut budget),
        });
    }
    fetched
}

fn document(
    loc: &Locations,
    sources: &Sources,
    origin: &Repository,
    fetch: &impl network::Fetch,
    budget: &mut usize,
) -> Result<Document, String> {
    let snapshot = network::catalog_document(fetch, origin)?;
    let entries = metadata::catalog_entries(origin, &snapshot)?;
    let mut presentations = BTreeMap::new();
    for entry in &entries {
        let Ok(package) = entry else {
            continue;
        };
        // Source approvals apply to presentation downloads as well as installation.
        stage(
            loc,
            package,
            sources
                .trusted(origin, &package.repository)
                .then_some(fetch),
            &mut presentations,
            budget,
        );
    }
    Ok(Document {
        snapshot,
        entries,
        presentations,
    })
}

pub(super) fn stage(
    loc: &Locations,
    p: &metadata::Package,
    fetch: Option<&impl network::Fetch>,
    presentations: &mut BTreeMap<PathBuf, FileData>,
    budget: &mut usize,
) {
    for (name, limit) in [("CHANGELOG.md", 64 * 1024), ("icon.png", 256 * 1024)] {
        let Some(file) = p.files.iter().find(|f| f.path == name && f.size <= limit) else {
            continue;
        };
        let cached = loc.state.join("presentation").join(&file.sha256);
        match storage::read(&cached, limit) {
            Ok(Some(data))
                if data.bytes.len() == file.size && storage::sha(&data.bytes) == file.sha256 =>
            {
                continue;
            }
            Ok(_) => (),
            Err(error) => {
                presentation_error(p, name, &error);
                continue;
            }
        }
        if file.size > *budget {
            presentation_error(p, name, "Presentation refresh budget exhausted");
            continue;
        }
        let Some(fetch) = fetch else {
            presentation_error(p, name, "Presentation not cached");
            continue;
        };
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{name}",
            p.repository.as_str(),
            p.commit,
            p.directory
        );
        let result = fetch.fetch(&url, file.size).and_then(|bytes| {
            if bytes.len() == file.size && storage::sha(&bytes) == file.sha256 {
                Ok(bytes)
            } else {
                Err("Presentation checksum mismatch".into())
            }
        });
        match result {
            Ok(bytes) => {
                *budget = budget.saturating_sub(bytes.len());
                presentations.insert(cached, FileData { bytes, mode: 0o600 });
            }
            Err(error) => presentation_error(p, name, &error),
        }
    }
}

fn presentation_error(p: &metadata::Package, name: &str, error: &str) {
    eprintln!(
        "level=info event=app_center_presentation app={:?} file={name:?} error={error:?}",
        p.id
    );
}

/// Writer phase of a refresh: runs under the storage lock and performs every
/// durable write (snapshot, staged presentation files and rows).
pub(super) fn store_documents(
    loc: &Locations,
    sources: &Sources,
    fetched: Vec<Fetched>,
    previous: &mut Vec<Checked>,
) -> Vec<Checked> {
    let mut rows = Vec::new();
    for Fetched { origin, document } in fetched {
        match document {
            Ok(document) => {
                let result = storage::atomic(
                    &snapshot(loc, &origin),
                    &FileData {
                        bytes: document.snapshot,
                        mode: 0o600,
                    },
                );
                match result {
                    Ok(()) => rows.extend(parse(
                        loc,
                        sources,
                        &origin,
                        document.entries,
                        &document.presentations,
                    )),
                    Err(error) => retain(loc, sources, &origin, &error, previous, &mut rows),
                }
            }
            Err(error) => retain(loc, sources, &origin, &error, previous, &mut rows),
        }
    }
    rows
}

fn retain(
    loc: &Locations,
    sources: &Sources,
    origin: &Repository,
    error: &str,
    previous: &mut Vec<Checked>,
    rows: &mut Vec<Checked>,
) {
    eprintln!(
        "level=warn event=app_center_repository source={:?} error={error:?}",
        origin.as_str()
    );
    // Move the known-good origin's metadata, retaining its pinned inventory.
    let mut retained = Vec::new();
    for mut row in std::mem::take(previous) {
        if row.package.origin == *origin && !row.package.entry.is_empty() {
            refresh_local(loc, sources, &mut row);
            rows.push(row);
        } else {
            retained.push(row);
        }
    }
    *previous = retained;
    rows.push(source_error(
        origin,
        &format!("Repository unavailable; cached apps kept. {error}"),
    ));
}

fn parse(
    loc: &Locations,
    sources: &Sources,
    origin: &Repository,
    entries: Vec<Result<metadata::Package, String>>,
    presentations: &BTreeMap<PathBuf, FileData>,
) -> Vec<Checked> {
    let mut rows = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        match entry {
            Ok(mut package) => {
                presentation(loc, &mut package, presentations);
                let mut row = Checked {
                    package,
                    installed: String::new(),
                    status: String::new(),
                    ready: false,
                };
                refresh_local(loc, sources, &mut row);
                rows.push(row);
            }
            Err(error) => {
                let mut row = source_error(origin, &error);
                row.package.id = format!("io.vitrallis.invalid{index}");
                row.package.name = format!("Invalid app {}", index + 1);
                rows.push(row);
            }
        }
    }
    rows
}

fn presentation(
    loc: &Locations,
    p: &mut metadata::Package,
    presentations: &BTreeMap<PathBuf, FileData>,
) {
    for (name, limit) in [("CHANGELOG.md", 64 * 1024), ("icon.png", 256 * 1024)] {
        let Some(file) = p.files.iter().find(|f| f.path == name && f.size <= limit) else {
            continue;
        };
        let cached = loc.state.join("presentation").join(&file.sha256);
        let bytes = if let Some(data) = presentations.get(&cached) {
            if let Err(error) = storage::atomic(&cached, data) {
                presentation_error(p, name, &error);
                continue;
            }
            data.bytes.clone()
        } else {
            match storage::read(&cached, limit) {
                Ok(Some(data))
                    if data.bytes.len() == file.size
                        && storage::sha(&data.bytes) == file.sha256 =>
                {
                    data.bytes
                }
                _ => continue,
            }
        };
        match name {
            "CHANGELOG.md" => p.changelog = changelog(&bytes).map(Into::into),
            _ => p.icon = icon(&bytes).ok().map(std::sync::Arc::new),
        }
    }
}

pub(super) fn icon(bytes: &[u8]) -> Result<Vec<u8>, String> {
    metadata::validate_icon(bytes)?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or("PNG output buffer exceeds addressable memory")?;
    let mut data = vec![0; size];
    let frame = reader.next_frame(&mut data).map_err(|e| e.to_string())?;
    let width = usize::try_from(frame.width).map_err(|e| e.to_string())?;
    let height = usize::try_from(frame.height).map_err(|e| e.to_string())?;
    let channels = frame.color_type.samples();
    let mut pixels = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32 {
        for x in 0..32 {
            let offset = ((y * height / 32) * width + x * width / 32) * channels;
            let px = &data[offset..offset + channels];
            match frame.color_type {
                png::ColorType::Rgb => pixels.extend_from_slice(&[px[0], px[1], px[2], 255]),
                png::ColorType::Rgba => pixels.extend_from_slice(px),
                png::ColorType::Grayscale => pixels.extend_from_slice(&[px[0], px[0], px[0], 255]),
                png::ColorType::GrayscaleAlpha => {
                    pixels.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
                }
                png::ColorType::Indexed => return Err("Unexpanded icon".into()),
            }
        }
    }
    Ok(pixels)
}

pub(super) fn changelog(bytes: &[u8]) -> Option<String> {
    if bytes.len() > 64 * 1024 {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    if text.trim().is_empty()
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return None;
    }
    Some(text.replace("\r\n", "\n").replace('\t', "    "))
}
