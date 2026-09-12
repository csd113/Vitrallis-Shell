//! Remote snapshots and optional presentation files have their own invalidation.
//! Reopening, local scans and mutations never fetch repository metadata.
use super::{
    Checked, Sources, metadata, network, refresh_local, source_error,
    sources::Repository,
    storage::{self, FileData, Locations},
};
use std::path::PathBuf;

fn snapshot(loc: &Locations, origin: &Repository) -> PathBuf {
    loc.state
        .join("catalogs")
        .join(format!("{}.json", storage::sha(origin.as_str().as_bytes())))
}

pub(super) fn load(loc: &Locations, sources: &Sources) -> Vec<Checked> {
    let mut rows = Vec::new();
    for origin in &sources.catalogs {
        match storage::read(&snapshot(loc, origin), metadata::CATALOG_LIMIT) {
            Ok(Some(file)) => {
                match parse(loc, sources, origin, &file.bytes, None::<&network::Curl>) {
                    Ok(entries) => rows.extend(entries),
                    Err(error) => rows.push(source_error(
                        origin,
                        &format!("Cached repository invalid: {error}"),
                    )),
                }
            }
            Ok(None) => (),
            Err(error) => rows.push(source_error(origin, &error)),
        }
    }
    rows
}

pub(super) fn refresh(
    loc: &Locations,
    sources: &Sources,
    fetch: &impl network::Fetch,
    previous: &mut Vec<Checked>,
    mut progress: impl FnMut(String),
) -> Vec<Checked> {
    let mut rows = Vec::new();
    for origin in &sources.catalogs {
        progress(format!("Refreshing {}", origin.as_str()));
        let result = (|| {
            let bytes = network::catalog_document(fetch, origin)?;
            let entries = parse(loc, sources, origin, &bytes, Some(fetch))?;
            storage::atomic(&snapshot(loc, origin), &FileData { bytes, mode: 0o600 })?;
            Ok::<_, String>(entries)
        })();
        match result {
            Ok(entries) => rows.extend(entries),
            Err(error) => {
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
        }
    }
    rows
}

fn parse(
    loc: &Locations,
    sources: &Sources,
    origin: &Repository,
    bytes: &[u8],
    fetch: Option<&impl network::Fetch>,
) -> Result<Vec<Checked>, String> {
    let entries = metadata::catalog_entries(origin, bytes)?;
    let mut rows = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        match entry {
            Ok(mut package) => {
                // Source approvals apply to presentation downloads as well as installation.
                let approved = sources.trusted(origin, &package.repository);
                presentation(loc, &mut package, fetch.filter(|_| approved));
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
    Ok(rows)
}

fn presentation(loc: &Locations, p: &mut metadata::Package, fetch: Option<&impl network::Fetch>) {
    for (name, limit) in [("CHANGELOG.md", 64 * 1024), ("icon.png", 256 * 1024)] {
        let Some(file) = p.files.iter().find(|f| f.path == name && f.size <= limit) else {
            continue;
        };
        let cached = loc.state.join("presentation").join(&file.sha256);
        let result = (|| {
            if let Some(data) = storage::read(&cached, limit)?
                && data.bytes.len() == file.size
                && storage::sha(&data.bytes) == file.sha256
            {
                return Ok(data.bytes);
            }
            let fetch = fetch.ok_or("Presentation not cached")?;
            let url = format!(
                "https://raw.githubusercontent.com/{}/{}/{}/{name}",
                p.repository.as_str(),
                p.commit,
                p.directory
            );
            let bytes = fetch.fetch(&url, file.size)?;
            if bytes.len() != file.size || storage::sha(&bytes) != file.sha256 {
                return Err("Presentation checksum mismatch".into());
            }
            storage::atomic(
                &cached,
                &FileData {
                    bytes: bytes.clone(),
                    mode: 0o600,
                },
            )?;
            Ok::<_, String>(bytes)
        })();
        match result {
            Ok(bytes) if name == "CHANGELOG.md" => {
                p.changelog = changelog(&bytes).map(Into::into);
            }
            Ok(bytes) => p.icon = icon(&bytes).ok().map(std::sync::Arc::new),
            Err(error) => eprintln!(
                "level=info event=app_center_presentation app={:?} file={name:?} error={error:?}",
                p.id
            ),
        }
    }
}
fn icon(bytes: &[u8]) -> Result<Vec<u8>, String> {
    metadata::validate_icon(bytes)?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut data = vec![0; reader.output_buffer_size()];
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
