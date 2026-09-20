//! Storage ownership comes from installed App Manager manifests and receipts.
use super::{discovery, install, metadata, storage::Locations};
use crate::storage::scan::{Scanner, Size};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Debug, Clone)]
pub struct AppUsage {
    pub id: String,
    pub name: String,
    pub icon: Option<Vec<u8>>,
    pub parts: [Size; 4],
    pub total: Size,
}
pub const PARTS: [&str; 4] = [
    "Application files",
    "App runtime",
    "App cache",
    "Data / managed files",
];

pub fn installed(
    loc: &Locations,
    scanner: &mut Scanner<'_>,
) -> Result<(Vec<AppUsage>, Option<String>), String> {
    let mut manifests = discovery::manifests(loc, || !scanner.stopped())?;
    manifests.sort_by(|a, b| {
        a.as_ref()
            .ok()
            .map(|(path, _)| path)
            .cmp(&b.as_ref().ok().map(|(path, _)| path))
    });
    let mut apps = Vec::new();
    let mut issue = None;
    for manifest in manifests {
        let (root, manifest) = match manifest {
            Ok(app) => app,
            Err(error) => {
                issue.get_or_insert(error);
                continue;
            }
        };
        let id = metadata::text(&manifest["id"], 128)?.to_owned();
        let name = metadata::text(&manifest["name"], 1000)?.to_owned();
        let receipt = if scanner.stopped() {
            Err("Scan cancelled or limit reached".into())
        } else {
            install::receipt(&root).and_then(|receipt| {
                if receipt.as_ref().is_some_and(|value| value["id"] != id) {
                    Err("Install receipt identity differs".into())
                } else {
                    Ok(receipt)
                }
            })
        };
        let mut parts = std::array::from_fn(|_| Size::default());
        let mut owned: BTreeSet<String> = match receipt {
            Ok(Some(ref receipt)) => receipt["files"]
                .as_object()
                .map(|files| files.keys().cloned().collect())
                .unwrap_or_default(),
            Ok(None) => {
                parts[0].fail("Install receipt missing; file classification unavailable");
                BTreeSet::new()
            }
            Err(ref error) => {
                parts[0].fail(error);
                BTreeSet::new()
            }
        };
        let issue = scanner.walk(&root, false, &mut |path, bytes| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            let index = category(relative, &owned);
            if let Some(name) = relative.to_str() {
                owned.remove(name);
            }
            parts[index].bytes = parts[index].bytes.saturating_add(bytes);
        });
        if !owned.is_empty() {
            parts[0].fail("Some installed files are missing or unreadable");
        }
        if let Some(issue) = issue {
            for part in &mut parts {
                part.fail(&issue);
            }
        }
        for path in managed_locations(loc, &id, receipt.as_ref().ok().and_then(Option::as_ref)) {
            parts[3].add(&scanner.measure(&path, true));
        }
        if !scanner.stopped() {
            desktop_files(loc, &id, scanner, &mut parts[3]);
        }
        let total = aggregate(parts.iter());
        let icon = if scanner.stopped() {
            None
        } else {
            super::storage::read(&root.join("icon.png"), 256 * 1024)
                .ok()
                .flatten()
                .and_then(|file| super::cache::icon(&file.bytes).ok())
        };
        apps.push(AppUsage {
            id,
            name,
            icon,
            parts,
            total,
        });
    }
    sort(&mut apps);
    Ok((apps, issue))
}

fn category(path: &std::path::Path, owned: &BTreeSet<String>) -> usize {
    if path
        .components()
        .any(|c| c.as_os_str() == ".vitrallis-bytecode" || c.as_os_str() == "__pycache__")
    {
        2
    } else if path.starts_with("runtime") || path.starts_with(".venv") {
        1
    } else if path.to_str().is_some_and(|p| owned.contains(p)) || path.as_os_str().is_empty() {
        0
    } else {
        3
    }
}

fn managed_locations(
    loc: &Locations,
    id: &str,
    receipt: Option<&serde_json::Value>,
) -> Vec<PathBuf> {
    let mut paths = vec![loc.state.join("launchers").join(id)];
    if let Some(receipt) = receipt
        && let Some(origin) = receipt["origin"].as_str()
        && metadata::identity(id).is_ok()
    {
        paths.push(
            loc.state
                .join("transactions")
                .join(super::storage::sha(format!("{origin}:{id}").as_bytes())),
        );
    }
    paths
}

fn desktop_files(loc: &Locations, id: &str, scanner: &mut Scanner<'_>, size: &mut Size) {
    let launcher = loc.state.join("launchers").join(id);
    let expected = match install::desktop_quote(&launcher) {
        Ok(value) => format!("Exec={value}"),
        Err(error) => {
            size.fail(error);
            return;
        }
    };
    for parent in [loc.data.join("applications"), loc.home.join("Desktop")] {
        let path = parent.join(format!("{id}.desktop"));
        match super::storage::read(&path, metadata::FILE_LIMIT) {
            Ok(Some(file))
                if std::str::from_utf8(&file.bytes)
                    .is_ok_and(|text| text.lines().any(|line| line == expected)) =>
            {
                size.add(&scanner.measure(&path, false));
            }
            Ok(_) => (),
            Err(error) => size.fail(error),
        }
    }
}

pub fn aggregate<'a>(sizes: impl Iterator<Item = &'a Size>) -> Size {
    sizes.fold(Size::default(), |mut total, size| {
        total.add(size);
        total
    })
}
pub fn sort(apps: &mut [AppUsage]) {
    apps.sort_by(|a, b| {
        b.total
            .bytes
            .cmp(&a.total.bytes)
            .then_with(|| a.total.incomplete.cmp(&b.total.incomplete))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::atomic::AtomicBool};

    fn installed_fixture(loc: &Locations, id: &str, rust: bool) -> Result<(), String> {
        let root = loc.data.join("vitrallis/apps").join(id);
        let runtime = if rust {
            "runtime = \"rust\"\n[binaries]\nx86_64-unknown-linux-gnu = \"bin/app\"\n"
        } else {
            "runtime = \"python\"\nentry = \"main.py\"\n"
        };
        let manifest = format!(
            "manifest_version = 1\nname = \"{id}\"\nid = \"{id}\"\nversion = \"1.0.0\"\n{runtime}[permissions]\nnetwork = false\naudio = false\nstorage = true\n"
        );
        let files = [
            ("app.toml", manifest.as_bytes()),
            (
                if rust { "bin/app" } else { "main.py" },
                b"application".as_slice(),
            ),
            ("runtime/venv/lib/package", b"private runtime".as_slice()),
            (".vitrallis-bytecode/cache", b"bytecode".as_slice()),
            ("config/preferences", b"user preferences".as_slice()),
            (
                "icon.png",
                include_bytes!("../../assets/system/apps.png").as_slice(),
            ),
        ];
        for (name, bytes) in files {
            super::super::storage::atomic(
                &root.join(name),
                &super::super::storage::FileData {
                    bytes: bytes.to_vec(),
                    mode: 0o644,
                },
            )?;
        }
        let receipt = serde_json::json!({ "id": id, "version": "1.0.0", "origin": "owner/apps", "repository": "owner/apps", "commit": "a".repeat(40), "files": { "app.toml": "b".repeat(64), if rust { "bin/app" } else { "main.py" }: "c".repeat(64) } });
        fs::write(root.join(".vitrallis-receipt.json"), receipt.to_string())
            .map_err(|e| e.to_string())?;
        let launcher = loc.state.join("launchers").join(id);
        super::super::storage::atomic(
            &launcher,
            &super::super::storage::FileData {
                bytes: vec![1; 8192],
                mode: 0o755,
            },
        )?;
        Ok(())
    }
    #[test]
    fn python_and_rust_share_manifest_inventory_and_multiple_owned_locations() -> Result<(), String>
    {
        let (_scratch, loc) = super::super::tests::locations()?;
        for (id, rust) in [("io.test.python", false), ("io.test.rust", true)] {
            installed_fixture(&loc, id, rust)?;
        }
        let cancel = AtomicBool::new(false);
        let mut scanner = Scanner::new(&cancel);
        let (apps, issue) = installed(&loc, &mut scanner)?;
        assert!(issue.is_none());
        assert_eq!(apps.len(), 2);
        for app in &apps {
            assert!(!app.total.incomplete, "{:?}", app.total.issue);
            assert_eq!(app.icon.as_ref().map(Vec::len), Some(32 * 32 * 4));
            assert_eq!(
                app.total.bytes,
                app.parts.iter().map(|part| part.bytes).sum::<u64>()
            );
            assert!(app.parts.iter().all(|part| part.bytes > 0));
            let mut independent = Scanner::new(&cancel);
            let root_size =
                independent.measure(&loc.data.join("vitrallis/apps").join(&app.id), false);
            let launcher_size =
                independent.measure(&loc.state.join("launchers").join(&app.id), false);
            assert_eq!(app.total.bytes, root_size.bytes + launcher_size.bytes);
        }
        // Ownership roots remain claimed when measuring wider manager categories.
        assert_eq!(
            scanner
                .measure(&loc.state.join("launchers/io.test.python"), false)
                .bytes,
            0
        );
        Ok(())
    }
    #[test]
    fn empty_installations_corrupt_manifests_and_missing_receipts_are_explicit()
    -> Result<(), String> {
        let (_scratch, loc) = super::super::tests::locations()?;
        let cancel = AtomicBool::new(false);
        let (apps, issue) = installed(&loc, &mut Scanner::new(&cancel))?;
        assert!(apps.is_empty() && issue.is_none());
        installed_fixture(&loc, "io.test.python", false)?;
        let root = loc.data.join("vitrallis/apps/io.test.python");
        fs::remove_file(root.join(".vitrallis-receipt.json")).map_err(|e| e.to_string())?;
        fs::remove_file(root.join("main.py")).map_err(|e| e.to_string())?;
        let (apps, _) = installed(&loc, &mut Scanner::new(&cancel))?;
        assert_eq!(apps.len(), 1);
        assert!(apps[0].total.incomplete);
        fs::write(root.join("app.toml"), "bad manifest").map_err(|e| e.to_string())?;
        let (apps, issue) = installed(&loc, &mut Scanner::new(&cancel))?;
        assert!(apps.is_empty() && issue.is_some());
        Ok(())
    }
}
